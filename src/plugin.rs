//! Per-site plugins: compiled-in hooks, configured per site under `plugins:` and run in the
//! order written.
//!
//! A plugin can answer a request itself (`request`), change what is sent upstream
//! (`upstream_request`) and change every response of the site, proxied or generated locally
//! (`response`). Hooks are synchronous and run on the request task, so they must not block.
//! A plugin that has to wait (a password hash, an auth service) says so from `gate`; its
//! `check` is then awaited before the chain goes on, so the written order always holds.

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, header};
use pingora::Result;
use pingora::http::{RequestHeader, ResponseHeader};

use crate::config::{HeaderOps, HeadersConfig, IpFilterConfig, PluginConfig, UpstreamRef};
use crate::crowdsec;
use crate::limits;
use crate::realip::Cidr;
use crate::replace::{BodyReplace, BodyReplacePlugin};
use crate::route::Route;
use crate::rules::{Action, Headers, RequestView, Rule, compile_headers};
use crate::upstream::Pool;

/// A response produced without contacting the upstream.
pub struct Local {
    pub status: u16,
    pub headers: Headers,
    pub body: Bytes,
    /// What refused the client, for the access log (`crowdsec`, `appsec`, `mtls`, a plugin).
    pub blocked_by: Option<&'static str>,
}

impl Local {
    pub fn status(status: u16) -> Local {
        Local {
            status,
            headers: Vec::new(),
            body: Bytes::new(),
            blocked_by: None,
        }
    }

    pub fn with_body(status: u16, headers: Headers, body: Bytes) -> Local {
        Local {
            status,
            headers,
            body,
            blocked_by: None,
        }
    }

    pub fn redirect(status: u16, location: &[u8]) -> Local {
        let mut l = Local::status(status);
        if let Ok(v) = HeaderValue::from_bytes(location) {
            l.headers.push((header::LOCATION, v));
        } else {
            l.status = 400;
        }
        l
    }
}

/// What a plugin's synchronous step decided.
pub enum Gate {
    Pass,
    Answer(Local),
    /// Await the plugin's `check` before going on.
    Check,
    /// Proxy along this route; the chain goes on.
    Route(Arc<Route>),
    /// Rewrite the response body; the chain goes on.
    Replace(Arc<BodyReplace>),
}

/// Finds a pool by name, or builds an inline one under the name given.
pub type UpstreamFor<'a> = dyn FnMut(&UpstreamRef, String) -> Result<Arc<Pool>, String> + 'a;

/// What the plugins that let a request through leave for the rest of it.
#[derive(Default)]
pub struct Effects {
    /// In-flight counts (`concurrency`), released when the request ends.
    pub held: Vec<limits::Held>,
    /// The first `proxy` rule that matched.
    pub route: Option<Arc<Route>>,
    /// Every `body_replace` that takes the request, in chain order.
    pub replace: Vec<Arc<BodyReplace>>,
}

/// Where the chain stopped.
pub enum Step {
    /// Every plugin let the request through.
    Pass,
    Answer(Local),
    /// The plugin at this index waits in `check`; the chain goes on after it.
    Check(usize),
}

/// What `check` sees of the request.
pub struct CheckRequest<'a> {
    pub req: &'a RequestHeader,
    /// The client address after `real_ip` processing.
    pub client: Option<IpAddr>,
    /// The scheme the client used, as `X-Forwarded-Proto` tells upstreams.
    pub scheme: &'static str,
}

/// Headers to set (`Some`) or remove (`None`) on the request sent upstream.
pub type UpstreamEdits = Vec<(HeaderName, Option<HeaderValue>)>;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;

    /// Before proxying. Returning a response ends the request; later plugins do not run.
    fn request(&self, _req: &RequestView<'_>) -> Option<Local> {
        None
    }

    /// `request` for plugins that keep something for as long as the request runs (pushed
    /// to `held`, dropped with the request).
    fn admit(&self, req: &RequestView<'_>, _held: &mut Vec<limits::Held>) -> Option<Local> {
        self.request(req)
    }

    /// `admit` for plugins that may have to wait: `Gate::Check` hands the request to `check`.
    fn gate(&self, req: &RequestView<'_>, held: &mut Vec<limits::Held>) -> Gate {
        match self.admit(req, held) {
            Some(local) => Gate::Answer(local),
            None => Gate::Pass,
        }
    }

    /// The waiting part, after `gate` returned `Check`. `Err` answers the request; `Ok`
    /// changes the request sent upstream.
    async fn check(&self, _req: &CheckRequest<'_>) -> Result<UpstreamEdits, Local> {
        Ok(Vec::new())
    }

    /// The request about to be sent upstream.
    fn upstream_request(&self, _up: &mut RequestHeader) -> Result<()> {
        Ok(())
    }

    /// Every response of the site, including local ones and errors.
    fn response(&self, _resp: &mut ResponseHeader) -> Result<()> {
        Ok(())
    }

    /// Whether `request` reads `RequestView::path`, which is then normalized beforehand.
    fn needs_path(&self) -> bool {
        false
    }

    /// Whether requests go to CrowdSec AppSec once the synchronous hooks let them pass.
    fn appsec(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct Chain {
    plugins: Vec<Box<dyn Plugin>>,
    needs_path: bool,
    appsec: bool,
    /// Pools `proxy` rules send requests to.
    pools: Vec<Arc<Pool>>,
    /// A `proxy` rule without an upstream of its own: the one of the site.
    site_pool: bool,
}

impl Chain {
    /// `site`: the site's id, for metrics and for limits' state across reloads. `crowdsec`:
    /// the top-level `crowdsec:` section, which the plugin of that name needs. `upstream`
    /// finds a named pool, or builds an inline one under the name given.
    pub fn build(
        cfgs: &[PluginConfig],
        site: &str,
        crowdsec: Option<&Arc<crowdsec::Plan>>,
        watched: &mut Vec<std::path::PathBuf>,
        upstream: &mut UpstreamFor<'_>,
    ) -> Result<Chain, String> {
        let mut plugins: Vec<Box<dyn Plugin>> = Vec::with_capacity(cfgs.len());
        let mut pools = Vec::new();
        let mut site_pool = false;
        for (i, c) in cfgs.iter().enumerate() {
            let p: Box<dyn Plugin> = match c {
                PluginConfig::Rules(rules) => {
                    let mut resolve = |u: &UpstreamRef, j: usize| {
                        upstream(u, format!("site:{site}:rule{}.{}", i + 1, j + 1))
                    };
                    let rules: Vec<Rule> = rules
                        .iter()
                        .enumerate()
                        .map(|(j, r)| Rule::compile(j, r, watched, &mut resolve))
                        .collect::<Result<_, _>>()
                        .map_err(|e| format!("plugin #{} (rules): {e}", i + 1))?;
                    for r in &rules {
                        if let Action::Proxy(route) = &r.action {
                            match &route.pool {
                                Some(pool) => pools.push(pool.clone()),
                                None => site_pool = true,
                            }
                        }
                    }
                    Box::new(RulesPlugin(rules))
                }
                PluginConfig::BlockExploits(c) => Box::new(
                    crate::exploits::BlockExploits::new(c, site)
                        .map_err(|e| format!("plugin #{} (block_exploits): {e}", i + 1))?,
                ),
                PluginConfig::BodyReplace(c) => Box::new(
                    BodyReplacePlugin::new(c)
                        .map_err(|e| format!("plugin #{} (body_replace): {e}", i + 1))?,
                ),
                PluginConfig::Headers(h) => Box::new(
                    HeadersPlugin::new(h)
                        .map_err(|e| format!("plugin #{} (headers): {e}", i + 1))?,
                ),
                PluginConfig::IpFilter(f) => Box::new(
                    IpFilter::new(f).map_err(|e| format!("plugin #{} (ip_filter): {e}", i + 1))?,
                ),
                PluginConfig::CrowdSec(c) => Box::new(
                    crowdsec::CrowdSecPlugin::new(crowdsec, c.appsec)
                        .map_err(|e| format!("plugin #{} (crowdsec): {e}", i + 1))?,
                ),
                PluginConfig::RateLimit(c) => Box::new(
                    limits::RateLimit::new(c, site, i)
                        .map_err(|e| format!("plugin #{} (rate_limit): {e}", i + 1))?,
                ),
                PluginConfig::Concurrency(c) => Box::new(
                    limits::Concurrency::new(c, site, i)
                        .map_err(|e| format!("plugin #{} (concurrency): {e}", i + 1))?,
                ),
                PluginConfig::BasicAuth(c) => Box::new(
                    crate::auth::BasicAuth::new(c, site, watched)
                        .map_err(|e| format!("plugin #{} (basic_auth): {e}", i + 1))?,
                ),
                PluginConfig::ForwardAuth(c) => Box::new(
                    crate::auth::ForwardAuth::new(c, site)
                        .map_err(|e| format!("plugin #{} (forward_auth): {e}", i + 1))?,
                ),
            };
            plugins.push(p);
        }
        Ok(Chain {
            needs_path: plugins.iter().any(|p| p.needs_path()),
            appsec: plugins.iter().any(|p| p.appsec()),
            plugins,
            pools,
            site_pool,
        })
    }

    /// The pools `proxy` rules use.
    pub fn pools(&self) -> &[Arc<Pool>] {
        &self.pools
    }

    /// Whether a `proxy` rule sends requests to the site's own upstream.
    pub fn uses_site_pool(&self) -> bool {
        self.site_pool
    }

    pub fn needs_path(&self) -> bool {
        self.needs_path
    }

    pub fn appsec(&self) -> bool {
        self.appsec
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Run the synchronous steps from plugin `from` on. What plugins leave for the rest of
    /// the request (in-flight counts, a route, body rewrites) goes to `fx`.
    pub fn run(&self, req: &RequestView<'_>, fx: &mut Effects, from: usize) -> Step {
        for (i, p) in self.plugins.iter().enumerate().skip(from) {
            match p.gate(req, &mut fx.held) {
                Gate::Pass => {}
                Gate::Answer(local) => return Step::Answer(local),
                Gate::Check => return Step::Check(i),
                Gate::Route(route) => {
                    fx.route.get_or_insert(route);
                }
                Gate::Replace(r) => fx.replace.push(r),
            }
        }
        Step::Pass
    }

    /// The waiting part of plugin `index`, after `run` stopped at it.
    pub async fn check(
        &self,
        index: usize,
        req: &CheckRequest<'_>,
    ) -> Result<UpstreamEdits, Local> {
        self.plugins[index].check(req).await
    }

    /// `run` for chains without waiting plugins.
    #[cfg(test)]
    pub fn request(&self, req: &RequestView<'_>, held: &mut Vec<limits::Held>) -> Option<Local> {
        let mut fx = Effects {
            held: std::mem::take(held),
            ..Effects::default()
        };
        let step = self.run(req, &mut fx, 0);
        *held = fx.held;
        match step {
            Step::Pass => None,
            Step::Answer(local) => Some(local),
            Step::Check(i) => panic!("plugin #{} waits", i + 1),
        }
    }

    pub fn upstream_request(&self, up: &mut RequestHeader) -> Result<()> {
        self.plugins.iter().try_for_each(|p| p.upstream_request(up))
    }

    pub fn response(&self, resp: &mut ResponseHeader) -> Result<()> {
        self.plugins.iter().try_for_each(|p| p.response(resp))
    }
}

/// For chains built without any pool (tests).
#[cfg(test)]
pub fn no_upstreams(u: &UpstreamRef, _name: String) -> Result<Arc<Pool>, String> {
    Err(format!("no pools here: {u:?}"))
}

/// `rules`: the first matching rule answers the request, or routes it.
struct RulesPlugin(Vec<Rule>);

impl Plugin for RulesPlugin {
    fn name(&self) -> &'static str {
        "rules"
    }

    fn needs_path(&self) -> bool {
        true
    }

    fn gate(&self, req: &RequestView<'_>, _held: &mut Vec<limits::Held>) -> Gate {
        let Some(rule) = self.0.iter().find(|r| r.is_match(req)) else {
            return Gate::Pass;
        };
        log::debug!("{}: {} matched", req.host, rule.name);
        Gate::Answer(match &rule.action {
            Action::Proxy(route) => return Gate::Route(route.clone()),
            Action::Deny(status) => Local::status(*status),
            Action::Redirect { status, to } => {
                Local::redirect(*status, &to.render(req.https, req.host, req.path, req.uri))
            }
            Action::Respond {
                status,
                headers,
                body,
            } => Local::with_body(*status, headers.clone(), body.clone()),
        })
    }
}

struct Ops {
    set: Headers,
    remove: Vec<HeaderName>,
}

impl Ops {
    fn compile(o: &HeaderOps) -> Result<Ops, String> {
        Ok(Ops {
            set: compile_headers(&o.set)?,
            remove: o
                .remove
                .iter()
                .map(|n| {
                    HeaderName::try_from(n.as_str())
                        .map_err(|_| format!("invalid header name {n:?}"))
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

/// `headers`: set or remove request and response headers.
struct HeadersPlugin {
    request: Ops,
    response: Ops,
}

impl HeadersPlugin {
    fn new(c: &HeadersConfig) -> Result<HeadersPlugin, String> {
        Ok(HeadersPlugin {
            request: Ops::compile(&c.request)?,
            response: Ops::compile(&c.response)?,
        })
    }
}

impl Plugin for HeadersPlugin {
    fn name(&self) -> &'static str {
        "headers"
    }

    fn upstream_request(&self, up: &mut RequestHeader) -> Result<()> {
        for n in &self.request.remove {
            up.remove_header(n);
        }
        for (n, v) in &self.request.set {
            up.insert_header(n.clone(), v.clone())?;
        }
        Ok(())
    }

    fn response(&self, resp: &mut ResponseHeader) -> Result<()> {
        for n in &self.response.remove {
            resp.remove_header(n);
        }
        for (n, v) in &self.response.set {
            resp.insert_header(n.clone(), v.clone())?;
        }
        Ok(())
    }
}

/// `ip_filter`: refuse clients by address (the real client address, see `real_ip`).
struct IpFilter {
    allow: Vec<Cidr>,
    deny: Vec<Cidr>,
    status: u16,
}

impl IpFilter {
    fn new(c: &IpFilterConfig) -> Result<IpFilter, String> {
        let parse = |v: &[String]| {
            v.iter()
                .map(|s| Cidr::parse(s))
                .collect::<Result<Vec<_>, _>>()
        };
        if !(400..=599).contains(&c.status) {
            return Err(format!("status {} must be 4xx or 5xx", c.status));
        }
        Ok(IpFilter {
            allow: parse(&c.allow)?,
            deny: parse(&c.deny)?,
            status: c.status,
        })
    }
}

impl Plugin for IpFilter {
    fn name(&self) -> &'static str {
        "ip_filter"
    }

    fn request(&self, req: &RequestView<'_>) -> Option<Local> {
        let allowed = match req.client {
            Some(ip) => {
                !self.deny.iter().any(|c| c.contains(ip))
                    && (self.allow.is_empty() || self.allow.iter().any(|c| c.contains(ip)))
            }
            None => self.allow.is_empty(),
        };
        (!allowed).then(|| Local::status(self.status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;
    use http::Method;

    fn chain(yaml: &str) -> Chain {
        let cfg = parse(&format!(
            "sites:\n  - id: t\n    hosts: [t]\n    upstream: http://127.0.0.1:1\n    plugins:\n{yaml}"
        ))
        .unwrap();
        Chain::build(
            &cfg.sites[0].plugins,
            "t",
            None,
            &mut Vec::new(),
            &mut no_upstreams,
        )
        .unwrap()
    }

    fn view<'a>(client: &str, path: &'a [u8], headers: &'a http::HeaderMap) -> RequestView<'a> {
        RequestView {
            method: &Method::GET,
            https: true,
            host: "t.example",
            client: client.parse().ok(),
            path,
            uri: path,
            query: b"",
            headers,
        }
    }

    #[test]
    fn ip_filter() {
        let c =
            chain("      - ip_filter: { allow: [10.0.0.0/8], deny: [10.9.0.0/16], status: 404 }\n");
        let h = http::HeaderMap::new();
        assert!(
            c.request(&view("10.1.2.3", b"/", &h), &mut Vec::new())
                .is_none()
        );
        assert_eq!(
            c.request(&view("10.9.0.1", b"/", &h), &mut Vec::new())
                .unwrap()
                .status,
            404
        );
        assert_eq!(
            c.request(&view("192.0.2.1", b"/", &h), &mut Vec::new())
                .unwrap()
                .status,
            404
        );
        assert_eq!(
            c.request(&view("", b"/", &h), &mut Vec::new())
                .unwrap()
                .status,
            404
        );
        // IPv4-mapped IPv6 from a dual-stack socket
        assert!(
            c.request(&view("::ffff:10.1.2.3", b"/", &h), &mut Vec::new())
                .is_none()
        );
        assert!(!c.needs_path());
    }

    #[test]
    fn order_and_headers() {
        let c = chain(
            "      - headers:\n          request: { set: { X-Env: prod }, remove: [X-Debug] }\n          response: { set: { X-Frame-Options: DENY }, remove: [Server] }\n      - rules:\n          - match: { path: { prefix: /admin } }\n            action: { deny: 403 }\n",
        );
        assert!(c.needs_path());
        assert_eq!(c.names(), ["headers", "rules"]);
        let h = http::HeaderMap::new();
        assert_eq!(
            c.request(&view("1.2.3.4", b"/admin/x", &h), &mut Vec::new())
                .unwrap()
                .status,
            403
        );
        assert!(
            c.request(&view("1.2.3.4", b"/ok", &h), &mut Vec::new())
                .is_none()
        );

        let mut up = RequestHeader::build("GET", b"/", None).unwrap();
        up.insert_header("x-debug", "1").unwrap();
        c.upstream_request(&mut up).unwrap();
        assert!(up.headers.get("x-debug").is_none());
        assert_eq!(up.headers["x-env"], "prod");

        let mut resp = ResponseHeader::build(200, None).unwrap();
        resp.insert_header("server", "app").unwrap();
        c.response(&mut resp).unwrap();
        assert!(resp.headers.get("server").is_none());
        assert_eq!(resp.headers["x-frame-options"], "DENY");
    }
}
