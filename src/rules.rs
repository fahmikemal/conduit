//! Rules for the `rules` plugin: conditions on the request and a local response for the
//! first match.

use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use http::{HeaderName, HeaderValue, Method};
use regex::bytes::Regex;

use crate::config::{ActionConfig, HeaderMatch, MatchConfig, RuleConfig, StrMatch, UpstreamRef};
use crate::route::Route;
use crate::upstream::Pool;

/// Finds or builds the pool a `proxy` action names; the index is the rule's.
pub type Resolve<'a> = dyn FnMut(&UpstreamRef, usize) -> Result<Arc<Pool>, String> + 'a;

#[derive(Debug)]
enum BytesMatch {
    Exact(Vec<u8>),
    Prefix(Vec<u8>),
    Regex(Regex),
}

impl BytesMatch {
    fn compile(m: &StrMatch, what: &str) -> Result<BytesMatch, String> {
        Ok(match m {
            StrMatch::Exact(s) => BytesMatch::Exact(s.as_bytes().to_vec()),
            StrMatch::Prefix(s) => BytesMatch::Prefix(s.as_bytes().to_vec()),
            StrMatch::Regex(r) => BytesMatch::Regex(
                Regex::new(r).map_err(|e| format!("invalid {what} regex {r:?}: {e}"))?,
            ),
        })
    }

    fn is_match(&self, v: &[u8]) -> bool {
        match self {
            BytesMatch::Exact(e) => v == e.as_slice(),
            BytesMatch::Prefix(p) => v.starts_with(p),
            BytesMatch::Regex(r) => r.is_match(v),
        }
    }
}

#[derive(Debug)]
enum HeaderCond {
    Present(bool),
    Equals(Vec<u8>),
    Regex(Regex),
}

impl HeaderCond {
    fn compile(name: &str, m: &HeaderMatch) -> Result<HeaderCond, String> {
        let set = [m.present.is_some(), m.equals.is_some(), m.regex.is_some()];
        if set.iter().filter(|s| **s).count() != 1 {
            return Err(format!(
                "header {name:?}: give exactly one of present, equals, regex"
            ));
        }
        Ok(if let Some(p) = m.present {
            HeaderCond::Present(p)
        } else if let Some(e) = &m.equals {
            HeaderCond::Equals(e.as_bytes().to_vec())
        } else {
            let r = m.regex.as_deref().unwrap_or_default();
            HeaderCond::Regex(
                Regex::new(r).map_err(|e| format!("header {name:?}: invalid regex {r:?}: {e}"))?,
            )
        })
    }

    /// `value` is all values of the header joined with ", ", empty when missing.
    fn is_match(&self, value: &[u8]) -> bool {
        match self {
            HeaderCond::Present(want) => !value.is_empty() == *want,
            HeaderCond::Equals(e) => value == e.as_slice(),
            HeaderCond::Regex(r) => r.is_match(value),
        }
    }
}

/// What plugins see of the request.
pub struct RequestView<'a> {
    pub method: &'a Method,
    pub https: bool,
    /// Lowercased, without port.
    pub host: &'a str,
    /// The client address after `real_ip` processing.
    pub client: Option<std::net::IpAddr>,
    /// Normalized path, see `pathnorm`.
    pub path: &'a [u8],
    /// Raw request target, path and query.
    pub uri: &'a [u8],
    pub query: &'a [u8],
    pub headers: &'a http::HeaderMap,
}

/// The conditions of a `match:` block.
#[derive(Debug)]
pub struct Matcher {
    methods: Vec<Method>,
    path: Option<BytesMatch>,
    uri: Option<BytesMatch>,
    query: Option<BytesMatch>,
    https: Option<bool>,
    headers: Vec<(HeaderName, HeaderCond)>,
}

impl Matcher {
    pub fn compile(m: &MatchConfig) -> Result<Matcher, String> {
        let methods = m
            .method
            .iter()
            .map(|s| {
                Method::from_bytes(s.to_ascii_uppercase().as_bytes())
                    .map_err(|_| format!("invalid method {s:?}"))
            })
            .collect::<Result<_, _>>()?;
        let https = match m.scheme.as_deref() {
            None => None,
            Some("https") => Some(true),
            Some("http") => Some(false),
            Some(s) => return Err(format!("invalid scheme {s:?}: use http or https")),
        };
        let headers = m
            .headers
            .iter()
            .map(|(name, cond)| {
                let h = HeaderName::try_from(name.as_str())
                    .map_err(|_| format!("invalid header name {name:?}"))?;
                Ok((h, HeaderCond::compile(name, cond)?))
            })
            .collect::<Result<_, String>>()?;
        Ok(Matcher {
            methods,
            path: m
                .path
                .as_ref()
                .map(|p| BytesMatch::compile(p, "path"))
                .transpose()?,
            uri: m
                .uri
                .as_ref()
                .map(|p| BytesMatch::compile(p, "uri"))
                .transpose()?,
            query: m
                .query
                .as_ref()
                .map(|p| BytesMatch::compile(p, "query"))
                .transpose()?,
            https,
            headers,
        })
    }

    pub fn is_match(&self, req: &RequestView<'_>) -> bool {
        if !self.methods.is_empty() && !self.methods.contains(req.method) {
            return false;
        }
        if self.https.is_some_and(|h| h != req.https) {
            return false;
        }
        if self.path.as_ref().is_some_and(|m| !m.is_match(req.path))
            || self.uri.as_ref().is_some_and(|m| !m.is_match(req.uri))
            || self.query.as_ref().is_some_and(|m| !m.is_match(req.query))
        {
            return false;
        }
        self.headers.iter().all(|(name, cond)| {
            let mut values = req.headers.get_all(name).iter();
            match (values.next(), values.next()) {
                (None, _) => cond.is_match(b""),
                (Some(v), None) => cond.is_match(v.as_bytes()),
                (Some(first), Some(second)) => {
                    let mut joined = first.as_bytes().to_vec();
                    for v in std::iter::once(second).chain(values) {
                        joined.extend_from_slice(b", ");
                        joined.extend_from_slice(v.as_bytes());
                    }
                    cond.is_match(&joined)
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
enum Part {
    Lit(String),
    Scheme,
    Host,
    Path,
    RequestUri,
}

/// A redirect target with `{scheme}`, `{host}`, `{path}`, `{request_uri}` placeholders.
#[derive(Debug, Clone)]
pub struct Template(Vec<Part>);

impl Template {
    pub fn parse(s: &str) -> Result<Template, String> {
        let mut parts = Vec::new();
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                parts.push(Part::Lit(rest[..open].to_string()));
            }
            let close = rest[open..]
                .find('}')
                .ok_or_else(|| format!("unclosed '{{' in {s:?}"))?
                + open;
            parts.push(match &rest[open + 1..close] {
                "scheme" => Part::Scheme,
                "host" => Part::Host,
                "path" => Part::Path,
                "request_uri" => Part::RequestUri,
                other => return Err(format!("unknown placeholder {{{other}}} in {s:?}")),
            });
            rest = &rest[close + 1..];
        }
        if !rest.is_empty() {
            parts.push(Part::Lit(rest.to_string()));
        }
        Ok(Template(parts))
    }

    pub fn render(&self, https: bool, host: &str, path: &[u8], request_uri: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + request_uri.len());
        for p in &self.0 {
            match p {
                Part::Lit(s) => out.extend_from_slice(s.as_bytes()),
                Part::Scheme => out.extend_from_slice(if https { b"https" } else { b"http" }),
                Part::Host => out.extend_from_slice(host.as_bytes()),
                Part::Path => out.extend_from_slice(path),
                Part::RequestUri => out.extend_from_slice(request_uri),
            }
        }
        out
    }
}

pub type Headers = Vec<(HeaderName, HeaderValue)>;

pub fn compile_headers(
    map: &std::collections::BTreeMap<String, String>,
) -> Result<Headers, String> {
    map.iter()
        .map(|(k, v)| {
            let name = HeaderName::try_from(k.as_str())
                .map_err(|_| format!("invalid header name {k:?}"))?;
            let value =
                HeaderValue::try_from(v.as_str()).map_err(|_| format!("invalid value for {k}"))?;
            Ok((name, value))
        })
        .collect()
}

#[derive(Debug)]
pub enum Action {
    Deny(u16),
    Redirect {
        status: u16,
        to: Template,
    },
    Respond {
        status: u16,
        headers: Headers,
        body: Bytes,
    },
    /// Proxied: the request goes on to the next plugin.
    Proxy(Arc<Route>),
}

fn status_ok(s: u16) -> Result<u16, String> {
    if (200..=599).contains(&s) {
        Ok(s)
    } else {
        Err(format!("invalid status {s}"))
    }
}

impl Action {
    fn compile(
        a: &ActionConfig,
        name: &str,
        index: usize,
        watched: &mut Vec<std::path::PathBuf>,
        resolve: &mut Resolve<'_>,
    ) -> Result<Action, String> {
        Ok(match a {
            ActionConfig::Proxy(p) => {
                let pool = p.upstream.as_ref().map(|u| resolve(u, index)).transpose()?;
                Action::Proxy(Arc::new(Route::compile(name, p, pool)?))
            }
            ActionConfig::Deny(s) => Action::Deny(status_ok(*s)?),
            ActionConfig::Redirect(r) => {
                if !(300..400).contains(&r.status) {
                    return Err(format!("redirect status must be 3xx, got {}", r.status));
                }
                Action::Redirect {
                    status: r.status,
                    to: Template::parse(&r.to)?,
                }
            }
            ActionConfig::Respond(r) => Action::Respond {
                status: status_ok(r.status)?,
                headers: compile_headers(&r.headers)?,
                body: Bytes::from(r.body.clone()),
            },
            ActionConfig::File(f) => {
                let body = std::fs::read(&f.path)
                    .map_err(|e| format!("cannot read file {}: {e}", f.path))?;
                watched.push(Path::new(&f.path).to_path_buf());
                let mut headers = compile_headers(&f.headers)?;
                headers.push((
                    http::header::CONTENT_TYPE,
                    HeaderValue::try_from(f.content_type.as_str())
                        .map_err(|_| format!("invalid content_type {:?}", f.content_type))?,
                ));
                Action::Respond {
                    status: 200,
                    headers,
                    body: Bytes::from(body),
                }
            }
        })
    }
}

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    matcher: Matcher,
    pub action: Action,
}

impl Rule {
    pub fn compile(
        index: usize,
        r: &RuleConfig,
        watched: &mut Vec<std::path::PathBuf>,
        resolve: &mut Resolve<'_>,
    ) -> Result<Rule, String> {
        let name = r
            .name
            .clone()
            .unwrap_or_else(|| format!("rule #{}", index + 1));
        let matcher = Matcher::compile(&r.matcher).map_err(|e| format!("{name}: {e}"))?;
        let action = Action::compile(&r.action, &name, index, watched, resolve)
            .map_err(|e| format!("{name}: {e}"))?;
        Ok(Rule {
            name,
            matcher,
            action,
        })
    }

    pub fn is_match(&self, req: &RequestView<'_>) -> bool {
        self.matcher.is_match(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;

    fn no_pools(u: &UpstreamRef, _: usize) -> Result<Arc<Pool>, String> {
        Err(format!("no pools here: {u:?}"))
    }

    fn site_rules(c: &crate::config::Config) -> &[crate::config::RuleConfig] {
        match &c.sites[0].plugins[0] {
            crate::config::PluginConfig::Rules(r) => r,
            other => panic!("{other:?}"),
        }
    }

    fn rules(yaml: &str) -> Vec<Rule> {
        let c = parse(yaml).unwrap();
        let mut w = vec![];
        site_rules(&c)
            .iter()
            .enumerate()
            .map(|(i, r)| Rule::compile(i, r, &mut w, &mut no_pools).unwrap())
            .collect()
    }

    fn view<'a>(
        method: &'a Method,
        path: &'a str,
        uri: &'a str,
        headers: &'a http::HeaderMap,
    ) -> RequestView<'a> {
        RequestView {
            method,
            https: true,
            host: "t",
            client: None,
            path: path.as_bytes(),
            uri: uri.as_bytes(),
            query: uri.split_once('?').map_or(b"", |(_, q)| q.as_bytes()),
            headers,
        }
    }

    const SITE: &str = "sites:\n  - id: t\n    hosts: [t]\n    upstream: http://127.0.0.1:1\n    plugins:\n    - rules:\n";

    #[test]
    fn forgejo_browser_redirect() {
        let r = rules(&format!(
            "{SITE}      - match: {{ method: [GET, HEAD], headers: {{ user-agent: {{ regex: \"(?i)mozilla\" }} }} }}\n        action: {{ redirect: {{ to: \"https://git.example.com{{request_uri}}\" }} }}\n"
        ));
        let mut h = http::HeaderMap::new();
        h.insert("user-agent", HeaderValue::from_static("Mozilla/5.0"));
        assert!(r[0].is_match(&view(&Method::GET, "/a", "/a?b=1", &h)));
        assert!(!r[0].is_match(&view(&Method::POST, "/a", "/a", &h)));
        let git = http::HeaderMap::new();
        assert!(!r[0].is_match(&view(&Method::GET, "/a", "/a", &git)));
        let Action::Redirect { to, .. } = &r[0].action else {
            panic!()
        };
        assert_eq!(
            to.render(true, "forgejo.example.com", b"/a", b"/a?b=1"),
            b"https://git.example.com/a?b=1"
        );
    }

    #[test]
    fn header_presence() {
        let r = rules(&format!(
            "{SITE}      - match: {{ headers: {{ x-middleware-subrequest: {{ present: true }} }} }}\n        action: {{ deny: 403 }}\n      - match: {{ uri: {{ exact: \"/\" }}, headers: {{ authorization: {{ present: false }} }} }}\n        action: {{ redirect: {{ to: \"/console/\" }} }}\n"
        ));
        let mut h = http::HeaderMap::new();
        assert!(!r[0].is_match(&view(&Method::GET, "/", "/", &h)));
        h.insert(
            "x-middleware-subrequest",
            HeaderValue::from_static("middleware"),
        );
        assert!(r[0].is_match(&view(&Method::GET, "/", "/", &h)));
        h.insert("x-middleware-subrequest", HeaderValue::from_static(""));
        assert!(!r[0].is_match(&view(&Method::GET, "/", "/", &h)));

        let empty = http::HeaderMap::new();
        assert!(r[1].is_match(&view(&Method::GET, "/", "/", &empty)));
        assert!(!r[1].is_match(&view(&Method::GET, "/", "/?x=1", &empty)));
        let mut auth = http::HeaderMap::new();
        auth.insert(
            "authorization",
            HeaderValue::from_static("AWS4-HMAC-SHA256 x"),
        );
        assert!(!r[1].is_match(&view(&Method::GET, "/", "/", &auth)));
    }

    #[test]
    fn path_regex_on_normalized_path() {
        let r = rules(&format!(
            "{SITE}      - match: {{ path: {{ regex: \"(?i)^/api/collections/(_superusers|pbc_1)(/|$)\" }} }}\n        action: {{ deny: 403 }}\n"
        ));
        let h = http::HeaderMap::new();
        assert!(r[0].is_match(&view(
            &Method::GET,
            "/api/collections/_superusers/records",
            "",
            &h
        )));
        assert!(r[0].is_match(&view(&Method::GET, "/API/Collections/_SUPERUSERS", "", &h)));
        assert!(!r[0].is_match(&view(&Method::GET, "/api/collections/posts", "", &h)));
    }

    #[test]
    fn bad_rules_are_rejected() {
        let c = parse(&format!(
            "{SITE}      - match: {{ headers: {{ x: {{ present: true, regex: a }} }} }}\n        action: {{ deny: 403 }}\n"
        ))
        .unwrap();
        assert!(Rule::compile(0, &site_rules(&c)[0], &mut vec![], &mut no_pools).is_err());
        assert!(Template::parse("https://x{nope}").is_err());
        assert!(Template::parse("https://x{host").is_err());
    }
}
