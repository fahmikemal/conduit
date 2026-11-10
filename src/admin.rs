//! Operator endpoints on `admin.listen`: `/metrics` (Prometheus text format), `/healthz`
//! (liveness), `/status` (JSON: configuration, pools and backends, certificates) and
//! `POST /cache/purge?site=<id>[&prefix=<path>]` (drops cached responses).
//!
//! The listener has no authentication; bind it to loopback or a management network.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use http::{Method, Response, StatusCode, header};
use pingora::apps::http_app::{HttpServer, ServeHttp};
use pingora::protocols::http::ServerSession;
use pingora::services::listening::Service;
use serde_json::json;

use crate::metrics;
use crate::proxy::Shared;
use crate::timefmt;

pub struct Admin {
    shared: Arc<Shared>,
}

pub fn service(shared: Arc<Shared>, addr: &str) -> Service<HttpServer<Admin>> {
    let mut svc = Service::new("admin".into(), HttpServer::new_app(Admin { shared }));
    // Each Pingora service gets its own runtime; scrapes need no more than one thread.
    svc.threads = Some(1);
    svc.add_tcp(addr);
    svc
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn reply(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(body)
        .expect("valid response")
}

impl Admin {
    fn status(&self) -> serde_json::Value {
        let s = &self.shared;
        let rt = s.runtime.load();
        let certs = s.certs.load();
        let now = SystemTime::now();
        let reload = s.last_reload.lock().unwrap();
        let cache = crate::cache::stats();
        let upstreams: Vec<_> = rt
            .pools
            .values()
            .map(|p| {
                let backends: Vec<_> = p
                    .status()
                    .map(|b| {
                        json!({
                            "address": b.label,
                            "weight": b.weight,
                            "healthy": b.healthy,
                            "ejected": b.ejected,
                            "in_flight": b.in_flight,
                            "http_version": b.http_version,
                        })
                    })
                    .collect();
                json!({
                    "name": &*p.name,
                    "balance": format!("{:?}", p.balance),
                    "retries": p.retries,
                    "backends": backends,
                })
            })
            .collect();
        let sites: Vec<_> = rt
            .sites
            .iter()
            .map(|site| {
                json!({
                    "id": &*site.id,
                    "upstream": site.pool.as_ref().map(|p| &*p.name),
                    "files": site.files.as_ref().map(|f| &f.root),
                    "cache": site.cache.is_some(),
                    "plugins": site.plugins.names(),
                    "mtls": site.mtls.as_ref().map(|m| if m.optional { "optional" } else { "required" }),
                })
            })
            .collect();
        let mut certificates: Vec<_> = certs
            .all()
            .map(|c| {
                let left = c.not_after_unix - unix(now) as i64;
                json!({
                    "name": c.name,
                    "names": c.dns_names,
                    "not_after": timefmt::rfc3339(c.not_after_unix.max(0) as u64),
                    "expires_in_days": left.div_euclid(86_400),
                })
            })
            .collect();
        certificates.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let time = |t: Option<i64>| t.map(|t| timefmt::rfc3339(t.max(0) as u64));
        let acme = rt.acme.as_ref().map(|plan| {
            let state = s.acme.sites.lock().unwrap();
            let sites: Vec<_> = plan
                .sites
                .iter()
                .map(|site| {
                    let st = state.get(&*site.id).cloned().unwrap_or_default();
                    json!({
                        "id": &*site.id,
                        "hosts": site.hosts,
                        "file": site.path,
                        "not_after": time(st.not_after),
                        "renew_at": time(st.renew_at),
                        "renew_window_from_ca": st.renew_window_from_ca,
                        "last_error": st.last_error,
                        "retry_at": time(st.retry_at),
                    })
                })
                .collect();
            json!({ "directory": plan.directory, "sites": sites })
        });
        let crowdsec = rt.crowdsec.as_ref().map(|plan| {
            let st = crate::crowdsec::STORE
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            json!({
                "lapi_url": plan.lapi_url,
                "decisions": crate::crowdsec::STORE.len(),
                "last_pull": st.last_success.map(|t| timefmt::rfc3339(unix(t))),
                "last_error": st.last_error,
                "failures_in_a_row": st.failures,
                "appsec": plan.appsec.is_some(),
            })
        });
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "started": timefmt::rfc3339(unix(s.started)),
            "uptime_seconds": now.duration_since(s.started).unwrap_or(Duration::ZERO).as_secs(),
            "config": {
                "path": s.config_path,
                "loaded": timefmt::rfc3339(unix(rt.loaded_at)),
                "last_reload": {
                    "at": timefmt::rfc3339(unix(reload.at)),
                    "ok": reload.ok,
                    "error": reload.error,
                },
            },
            "host_names": rt.router.len(),
            "sites": sites,
            "upstreams": upstreams,
            "certificates": certificates,
            "acme": acme,
            "crowdsec": crowdsec,
            "cache": {
                "objects": cache.objects,
                "bytes": cache.bytes,
                "evicted": cache.evicted,
                "max_size": rt.cache.max_size.0,
                "max_object_size": rt.cache.max_object_size.0,
            },
            "access_log": s.access_log.path(),
        })
    }
}

impl Admin {
    /// `site` names the site; `prefix`, when given, limits the purge to request targets
    /// (path and query, as clients send them) that start with it.
    fn purge(&self, query: &str) -> Response<Vec<u8>> {
        let (mut site, mut prefix) = (None, None);
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match (k, unescape(v)) {
                ("site", Some(v)) => site = Some(v),
                ("prefix", Some(v)) => prefix = Some(v),
                _ => return bad(&format!("unknown or malformed parameter {k:?}")),
            }
        }
        let Some(site) = site else {
            return bad("site=<id> is required");
        };
        let site = String::from_utf8_lossy(&site).into_owned();
        let rt = self.shared.runtime.load();
        if !rt.sites.iter().any(|s| *s.id == *site && s.cache.is_some()) {
            return reply(
                StatusCode::NOT_FOUND,
                "text/plain",
                format!("no site {site:?} with cache\n").into_bytes(),
            );
        }
        if prefix.as_ref().is_some_and(|p| !p.starts_with(b"/")) {
            return bad("prefix must start with /");
        }
        let purged = crate::cache::purge(&site, prefix.as_deref());
        let mut body =
            serde_json::to_vec(&json!({ "site": site, "purged": purged })).unwrap_or_default();
        body.push(b'\n');
        reply(StatusCode::OK, "application/json", body)
    }
}

fn bad(why: &str) -> Response<Vec<u8>> {
    reply(
        StatusCode::BAD_REQUEST,
        "text/plain",
        format!("{why}\n").into_bytes(),
    )
}

/// Undo the query string's percent-encoding (`+` stays a plus).
fn unescape(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b
                .get(i + 1..i + 3)
                .filter(|h| h.iter().all(u8::is_ascii_hexdigit))?;
            out.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(out)
}

#[async_trait]
impl ServeHttp for Admin {
    async fn response(&self, session: &mut ServerSession) -> Response<Vec<u8>> {
        let req = session.req_header();
        if req.method == Method::POST && req.uri.path() == "/cache/purge" {
            // The body is not read; do not try to reuse the connection after it.
            session.set_keepalive(None);
            return self.purge(session.req_header().uri.query().unwrap_or(""));
        }
        if req.method != Method::GET && req.method != Method::HEAD {
            return reply(
                StatusCode::METHOD_NOT_ALLOWED,
                "text/plain",
                b"GET only (POST for /cache/purge)\n".to_vec(),
            );
        }
        match req.uri.path() {
            "/metrics" => {
                let rt = self.shared.runtime.load();
                let certs = self.shared.certs.load();
                reply(
                    StatusCode::OK,
                    "text/plain; version=0.0.4",
                    metrics::render(&rt, &certs),
                )
            }
            "/healthz" => reply(StatusCode::OK, "text/plain", b"ok\n".to_vec()),
            "/status" => {
                let mut body = serde_json::to_vec_pretty(&self.status()).unwrap_or_default();
                body.push(b'\n');
                reply(StatusCode::OK, "application/json", body)
            }
            _ => reply(StatusCode::NOT_FOUND, "text/plain", b"not found\n".to_vec()),
        }
    }
}
