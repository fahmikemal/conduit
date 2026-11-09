//! The `proxy` action of rules: another upstream for the requests a rule matches, and the
//! path sent to it.
//!
//! Path changes work on the normalized path, the one rules match on, so a request cannot
//! dodge a `strip_prefix` with `/%61pi/` or `/x/../api/`. The result is sent with every byte
//! outside RFC 3986's path characters percent-encoded, followed by the original query
//! string. As with nginx's `rewrite` and `proxy_pass` with a path, an encoded `%2F` in the
//! client's path reaches the upstream as `/`; requests no rule rewrites go out unchanged.

use std::sync::Arc;

use regex::bytes::Regex;

use crate::config::ProxyConfig;
use crate::upstream::Pool;

#[derive(Debug)]
enum Rewrite {
    Prefix { strip: Vec<u8>, add: Vec<u8> },
    Regex { re: Regex, to: Vec<u8> },
}

/// Where a request goes once a `proxy` rule matched it.
pub struct Route {
    /// The rule's name, for the debug log.
    pub name: String,
    /// `None`: the site's own upstream.
    pub pool: Option<Arc<Pool>>,
    rewrite: Option<Rewrite>,
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("name", &self.name)
            .field("pool", &self.pool.as_ref().map(|p| &p.name))
            .field("rewrite", &self.rewrite)
            .finish()
    }
}

fn path_prefix(what: &str, p: &str) -> Result<Vec<u8>, String> {
    if !p.starts_with('/') {
        return Err(format!("{what} {p:?} must start with '/'"));
    }
    if p.contains(['?', '#']) {
        return Err(format!("{what} {p:?} is a path: no '?' or '#'"));
    }
    Ok(p.as_bytes().to_vec())
}

impl Route {
    pub fn compile(name: &str, c: &ProxyConfig, pool: Option<Arc<Pool>>) -> Result<Route, String> {
        let prefix = c.strip_prefix.is_some() || c.add_prefix.is_some();
        let rewrite = match (&c.rewrite, prefix) {
            (Some(_), true) => {
                return Err("rewrite cannot be combined with strip_prefix or add_prefix".into());
            }
            (Some(r), false) => {
                if r.to.contains(['?', '#']) {
                    return Err(format!("rewrite to {:?} is a path: no '?' or '#'", r.to));
                }
                Some(Rewrite::Regex {
                    re: Regex::new(&r.regex)
                        .map_err(|e| format!("invalid rewrite regex {:?}: {e}", r.regex))?,
                    to: r.to.as_bytes().to_vec(),
                })
            }
            (None, true) => {
                let strip = c
                    .strip_prefix
                    .as_deref()
                    .map(|p| path_prefix("strip_prefix", p))
                    .transpose()?
                    .unwrap_or_default();
                let mut add = c
                    .add_prefix
                    .as_deref()
                    .map(|p| path_prefix("add_prefix", p))
                    .transpose()?
                    .unwrap_or_default();
                // The path brings its own leading '/'.
                while add.last() == Some(&b'/') {
                    add.pop();
                }
                Some(Rewrite::Prefix { strip, add })
            }
            (None, false) => None,
        };
        Ok(Route {
            name: name.to_owned(),
            pool,
            rewrite,
        })
    }

    pub fn rewrites(&self) -> bool {
        self.rewrite.is_some()
    }

    /// The path (decoded, as matched) the upstream gets for normalized `path`.
    fn path(&self, path: &[u8]) -> Vec<u8> {
        let out = match &self.rewrite {
            None => path.to_vec(),
            Some(Rewrite::Prefix { strip, add }) => {
                let rest = path.strip_prefix(strip.as_slice()).unwrap_or(path);
                let mut out = add.clone();
                if !rest.starts_with(b"/") {
                    out.push(b'/');
                }
                out.extend_from_slice(rest);
                out
            }
            Some(Rewrite::Regex { re, to }) => re.replace(path, to.as_slice()).into_owned(),
        };
        if out.first() == Some(&b'/') {
            out
        } else {
            [b"/".as_slice(), &out].concat()
        }
    }

    /// The request target sent upstream: the rewritten `path` (normalized), encoded, and
    /// the raw `query` (without '?') when the request had one.
    pub fn target(&self, path: &[u8], query: Option<&[u8]>) -> Vec<u8> {
        let path = self.path(path);
        let mut out = Vec::with_capacity(path.len() + query.map_or(0, |q| q.len() + 1) + 8);
        encode_path(&path, &mut out);
        if let Some(q) = query {
            out.push(b'?');
            out.extend_from_slice(q);
        }
        out
    }
}

/// RFC 3986 `pchar` and '/': everything else is percent-encoded, '%' included (the path
/// is decoded).
fn encode_path(path: &[u8], out: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &b in path {
        let keep = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'/' | b'-'
                    | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if keep {
            out.push(b);
        } else {
            out.extend_from_slice(&[b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RewriteConfig;

    fn route(strip: Option<&str>, add: Option<&str>, rewrite: Option<(&str, &str)>) -> Route {
        let c = ProxyConfig {
            upstream: None,
            strip_prefix: strip.map(String::from),
            add_prefix: add.map(String::from),
            rewrite: rewrite.map(|(regex, to)| RewriteConfig {
                regex: regex.into(),
                to: to.into(),
            }),
        };
        Route::compile("t", &c, None).unwrap()
    }

    fn target(r: &Route, path: &str, query: Option<&str>) -> String {
        String::from_utf8(r.target(path.as_bytes(), query.map(str::as_bytes))).unwrap()
    }

    #[test]
    fn strip_and_add_prefix() {
        let r = route(Some("/api"), None, None);
        assert_eq!(target(&r, "/api/users", Some("a=1")), "/users?a=1");
        assert_eq!(target(&r, "/api", None), "/");
        assert_eq!(target(&r, "/api/", Some("")), "/?");
        // Not a segment boundary: still stripped, with the '/' put back.
        assert_eq!(target(&r, "/apix", None), "/x");
        assert_eq!(target(&r, "/other", None), "/other");

        let r = route(Some("/api/"), Some("/v2/"), None);
        assert_eq!(target(&r, "/api/users", None), "/v2/users");
        assert_eq!(target(&r, "/api/", None), "/v2/");
        let r = route(None, Some("/v2"), None);
        assert_eq!(target(&r, "/", None), "/v2/");
        assert_eq!(target(&r, "/x/y", None), "/v2/x/y");
    }

    #[test]
    fn regex_rewrite() {
        let r = route(None, None, Some(("^/old/(.*)$", "/new/${1}")));
        assert_eq!(target(&r, "/old/a/b", Some("q")), "/new/a/b?q");
        assert_eq!(target(&r, "/keep", None), "/keep");
        let r = route(None, None, Some(("^/u/(?P<id>[0-9]+)$", "/users/${id}")));
        assert!(r.rewrites());
        assert_eq!(target(&r, "/u/42", None), "/users/42");
        assert_eq!(target(&r, "/u/x", None), "/u/x");
        // A rewrite that drops the leading '/' gets it back.
        let r = route(None, None, Some(("^/x/", "")));
        assert_eq!(target(&r, "/x/y", None), "/y");
    }

    #[test]
    fn decoded_bytes_are_encoded_again() {
        let r = route(Some("/api"), None, None);
        assert_eq!(
            target(&r, "/api/a b/%/ü/#?", Some("x=%20")),
            "/a%20b/%25/%C3%BC/%23%3F?x=%20"
        );
        assert_eq!(target(&r, "/api/a:b@c;d=e", None), "/a:b@c;d=e");
    }

    #[test]
    fn bad_configs() {
        let c = |strip: Option<&str>, add: Option<&str>, rw: Option<(&str, &str)>| {
            Route::compile(
                "t",
                &ProxyConfig {
                    upstream: None,
                    strip_prefix: strip.map(String::from),
                    add_prefix: add.map(String::from),
                    rewrite: rw.map(|(regex, to)| RewriteConfig {
                        regex: regex.into(),
                        to: to.into(),
                    }),
                },
                None,
            )
        };
        assert!(c(Some("api"), None, None).is_err());
        assert!(c(None, Some("/v?x"), None).is_err());
        assert!(c(Some("/a"), None, Some(("x", "/y"))).is_err());
        assert!(c(None, None, Some(("(", "/y"))).is_err());
        assert!(c(None, None, Some(("x", "/y?z"))).is_err());
        assert!(!c(None, None, None).unwrap().rewrites());
    }
}
