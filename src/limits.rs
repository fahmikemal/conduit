//! `rate_limit` and `concurrency`: per-client limits on top of pingora-limits' count-min
//! sketches, so memory stays fixed (64 KiB and 32 KiB per plugin) however many clients come.
//! A sketch can only overestimate: an unlucky client may be counted with another one, never
//! less than it sent.
//!
//! Counts outlive reloads: a plugin whose site, position and settings are unchanged takes over
//! its predecessor's state, so a reload neither hands out a fresh allowance nor forgets the
//! requests still in flight.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::net::IpAddr;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::Duration;

use http::{HeaderName, HeaderValue, header};
use pingora_limits::estimator::Estimator;
use pingora_limits::rate::Rate;
use prometheus::IntCounter;

use crate::config::{ConcurrencyConfig, RateLimitConfig};
use crate::metrics;
use crate::plugin::{Local, Plugin};
use crate::rules::{Matcher, RequestView};

type Hasher = foldhash::fast::RandomState;

/// What requests are counted by.
#[derive(Debug)]
enum Key {
    ClientIp,
    Host,
    Header(HeaderName),
}

impl Key {
    fn parse(s: &str) -> Result<Key, String> {
        match s.trim() {
            "client_ip" => Ok(Key::ClientIp),
            "host" => Ok(Key::Host),
            other => match other.strip_prefix("header:") {
                Some(name) => HeaderName::try_from(name.trim())
                    .map(Key::Header)
                    .map_err(|_| format!("invalid header name in key {s:?}")),
                None => Err(format!(
                    "key {s:?}: expected client_ip, host or header:<name>"
                )),
            },
        }
    }

    fn of(&self, req: &RequestView<'_>, h: &Hasher) -> u64 {
        match self {
            Key::ClientIp => h.hash_one(client(req.client)),
            Key::Host => h.hash_one(("host", req.host)),
            Key::Header(name) => match req.headers.get(name) {
                Some(v) => h.hash_one(("header", v.as_bytes())),
                None => h.hash_one(client(req.client)),
            },
        }
    }
}

/// A client address as counted: IPv4-mapped IPv6 as IPv4, IPv6 by its /64, which one
/// subscriber usually holds whole.
fn client(ip: Option<IpAddr>) -> Option<IpAddr> {
    match ip?.to_canonical() {
        IpAddr::V6(v6) => Some(IpAddr::V6((u128::from(v6) & (u128::MAX << 64)).into())),
        v4 => Some(v4),
    }
}

fn check_status(status: u16) -> Result<(), String> {
    if (400..=599).contains(&status) {
        Ok(())
    } else {
        Err(format!("status {status} must be 4xx or 5xx"))
    }
}

/// State kept across reloads, by site, position in the chain and settings.
static CARRIED: LazyLock<Mutex<HashMap<String, Weak<dyn std::any::Any + Send + Sync>>>> =
    LazyLock::new(Default::default);

fn carried<T: Send + Sync + 'static>(id: String, new: impl FnOnce() -> T) -> Arc<T> {
    let mut map = CARRIED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.retain(|_, w| w.strong_count() > 0);
    if let Some(old) = map.get(&id).and_then(Weak::upgrade)
        && let Ok(old) = old.downcast::<T>()
    {
        return old;
    }
    let state = Arc::new(new());
    let any: Arc<dyn std::any::Any + Send + Sync> = state.clone();
    map.insert(id, Arc::downgrade(&any));
    state
}

struct Window {
    rate: Rate,
    hasher: Hasher,
    /// The same key always holds the same lock across the read-and-observe decision.
    /// Independent keys can proceed on other shards; the fixed-size sketch stays bounded.
    admission: [Mutex<()>; 64],
}

/// `rate_limit`: at most `requests` per `per`, over a sliding window (the previous window's
/// count, weighted by how much of it still overlaps, plus the current one's).
pub struct RateLimit {
    window: Arc<Window>,
    limit: f64,
    key: Key,
    matcher: Option<Matcher>,
    status: u16,
    retry_after: HeaderValue,
    refused: IntCounter,
}

impl RateLimit {
    pub fn new(c: &RateLimitConfig, site: &str, index: usize) -> Result<RateLimit, String> {
        if c.requests == 0 {
            return Err("requests must be at least 1".into());
        }
        let per = c.per.0;
        if per < Duration::from_millis(100) {
            return Err("per must be at least 100ms".into());
        }
        check_status(c.status)?;
        let window = carried(format!("rate\n{site}\n{index}\n{c:?}"), || Window {
            rate: Rate::new(per),
            hasher: Hasher::default(),
            admission: std::array::from_fn(|_| Mutex::new(())),
        });
        Ok(RateLimit {
            window,
            limit: f64::from(c.requests),
            key: Key::parse(&c.key)?,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            status: c.status,
            retry_after: HeaderValue::from(per.as_secs_f64().ceil().max(1.0) as u64),
            refused: metrics::LIMIT_REFUSED.with_label_values(&[site, "rate_limit"]),
        })
    }
}

impl Plugin for RateLimit {
    fn name(&self) -> &'static str {
        "rate_limit"
    }

    fn needs_path(&self) -> bool {
        self.matcher.is_some()
    }

    fn request(&self, req: &RequestView<'_>) -> Option<Local> {
        if self.matcher.as_ref().is_some_and(|m| !m.is_match(req)) {
            return None;
        }
        let w = &self.window;
        let key = self.key.of(req, &w.hasher);
        let admitted = {
            let _guard = w.admission[key as usize % w.admission.len()]
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let used = w.rate.rate_with(&key, |c| {
                c.prev_samples as f64 * (1.0 - c.current_interval_fraction) + c.curr_samples as f64
            });
            if used + 1.0 <= self.limit {
                w.rate.observe(&key, 1);
                true
            } else {
                false
            }
        };
        if !admitted {
            self.refused.inc();
            let mut l = Local::status(self.status);
            l.headers
                .push((header::RETRY_AFTER, self.retry_after.clone()));
            l.blocked_by = Some("rate_limit");
            return Some(l);
        }
        None
    }
}

struct InFlight {
    counts: Estimator,
    hasher: Hasher,
}

/// A request counted by `concurrency`; it stops counting when dropped with the request.
pub struct Held {
    state: Arc<InFlight>,
    key: u64,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.state.counts.decr(self.key, 1);
    }
}

/// `concurrency`: at most `max` requests of one client being processed at once, from the
/// request header until the response is sent.
pub struct Concurrency {
    state: Arc<InFlight>,
    max: isize,
    key: Key,
    matcher: Option<Matcher>,
    status: u16,
    refused: IntCounter,
}

impl Concurrency {
    pub fn new(c: &ConcurrencyConfig, site: &str, index: usize) -> Result<Concurrency, String> {
        if c.max == 0 {
            return Err("max must be at least 1".into());
        }
        check_status(c.status)?;
        let state = carried(format!("concurrency\n{site}\n{index}\n{c:?}"), || {
            InFlight {
                counts: Estimator::new(4, 1024),
                hasher: Hasher::default(),
            }
        });
        Ok(Concurrency {
            state,
            max: c.max as isize,
            key: Key::parse(&c.key)?,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            status: c.status,
            refused: metrics::LIMIT_REFUSED.with_label_values(&[site, "concurrency"]),
        })
    }
}

impl Plugin for Concurrency {
    fn name(&self) -> &'static str {
        "concurrency"
    }

    fn needs_path(&self) -> bool {
        self.matcher.is_some()
    }

    fn admit(&self, req: &RequestView<'_>, held: &mut Vec<Held>) -> Option<Local> {
        if self.matcher.as_ref().is_some_and(|m| !m.is_match(req)) {
            return None;
        }
        let key = self.key.of(req, &self.state.hasher);
        if self.state.counts.incr(key, 1) > self.max {
            self.state.counts.decr(key, 1);
            self.refused.inc();
            let mut l = Local::status(self.status);
            l.blocked_by = Some("concurrency");
            return Some(l);
        }
        held.push(Held {
            state: self.state.clone(),
            key,
        });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;
    use crate::plugin::Chain;
    use http::Method;

    fn chain(site: &str, yaml: &str) -> Chain {
        let cfg = parse(&format!(
            "sites:\n  - id: {site}\n    hosts: [t]\n    upstream: http://127.0.0.1:1\n    plugins:\n{yaml}"
        ))
        .unwrap();
        Chain::build(
            &cfg.sites[0].plugins,
            site,
            None,
            &mut Vec::new(),
            &mut crate::plugin::no_upstreams,
        )
        .unwrap()
    }

    fn view<'a>(
        client: &str,
        method: &'a Method,
        path: &'a [u8],
        headers: &'a http::HeaderMap,
    ) -> RequestView<'a> {
        RequestView {
            method,
            https: true,
            host: "t",
            client: client.parse().ok(),
            path,
            uri: path,
            query: b"",
            headers,
        }
    }

    #[test]
    fn rate_limit_per_client_and_match() {
        let c = chain(
            "rl",
            "      - rate_limit: { requests: 3, per: 1m, match: { method: [POST] } }\n",
        );
        let h = http::HeaderMap::new();
        let mut held = Vec::new();
        let (post, get) = (Method::POST, Method::GET);
        for _ in 0..3 {
            assert!(
                c.request(&view("192.0.2.1", &post, b"/login", &h), &mut held)
                    .is_none()
            );
        }
        let refused = c
            .request(&view("192.0.2.1", &post, b"/login", &h), &mut held)
            .unwrap();
        assert_eq!(refused.status, 429);
        assert_eq!(refused.blocked_by, Some("rate_limit"));
        assert_eq!(
            refused.headers[0],
            (header::RETRY_AFTER, HeaderValue::from(60u64))
        );
        // Refused requests do not use up the allowance; others and unmatched requests pass.
        assert!(
            c.request(&view("192.0.2.1", &post, b"/login", &h), &mut held)
                .is_some()
        );
        assert!(
            c.request(&view("192.0.2.1", &get, b"/login", &h), &mut held)
                .is_none()
        );
        assert!(
            c.request(&view("192.0.2.2", &post, b"/login", &h), &mut held)
                .is_none()
        );
        // One IPv6 /64 is one client.
        for _ in 0..3 {
            assert!(
                c.request(&view("2001:db8:1:2::1", &post, b"/", &h), &mut held)
                    .is_none()
            );
        }
        assert!(
            c.request(&view("2001:db8:1:2::ffff", &post, b"/", &h), &mut held)
                .is_some()
        );
        assert!(
            c.request(&view("2001:db8:1:3::1", &post, b"/", &h), &mut held)
                .is_none()
        );
        assert!(held.is_empty());

        // A reload with the same settings keeps the counts; other settings start over.
        let again = chain(
            "rl",
            "      - rate_limit: { requests: 3, per: 1m, match: { method: [POST] } }\n",
        );
        assert!(
            again
                .request(&view("192.0.2.1", &post, b"/", &h), &mut held)
                .is_some()
        );
        let changed = chain(
            "rl",
            "      - rate_limit: { requests: 4, per: 1m, match: { method: [POST] } }\n",
        );
        assert!(
            changed
                .request(&view("192.0.2.1", &post, b"/", &h), &mut held)
                .is_none()
        );
        drop(c);
    }

    #[test]
    fn rate_limit_by_header() {
        let c = chain(
            "rlh",
            "      - rate_limit: { requests: 1, per: 10s, key: \"header:X-Api-Key\" }\n",
        );
        let get = Method::GET;
        let mut a = http::HeaderMap::new();
        a.insert("x-api-key", HeaderValue::from_static("a"));
        let mut b = http::HeaderMap::new();
        b.insert("x-api-key", HeaderValue::from_static("b"));
        let mut held = Vec::new();
        assert!(
            c.request(&view("192.0.2.1", &get, b"/", &a), &mut held)
                .is_none()
        );
        // Same key from elsewhere: refused. Another key from the same address: allowed.
        assert!(
            c.request(&view("192.0.2.9", &get, b"/", &a), &mut held)
                .is_some()
        );
        assert!(
            c.request(&view("192.0.2.1", &get, b"/", &b), &mut held)
                .is_none()
        );
    }

    #[test]
    fn concurrency_holds_until_dropped() {
        let c = chain("cc", "      - concurrency: { max: 2, status: 503 }\n");
        let h = http::HeaderMap::new();
        let get = Method::GET;
        let mut first = Vec::new();
        let mut second = Vec::new();
        assert!(
            c.request(&view("192.0.2.1", &get, b"/", &h), &mut first)
                .is_none()
        );
        assert!(
            c.request(&view("192.0.2.1", &get, b"/", &h), &mut second)
                .is_none()
        );
        let mut third = Vec::new();
        let refused = c
            .request(&view("192.0.2.1", &get, b"/", &h), &mut third)
            .unwrap();
        assert_eq!(
            (refused.status, refused.blocked_by),
            (503, Some("concurrency"))
        );
        assert!(third.is_empty());
        assert!(
            c.request(&view("192.0.2.2", &get, b"/", &h), &mut third)
                .is_none()
        );
        // A reload keeps counting the requests in flight.
        let again = chain("cc", "      - concurrency: { max: 2, status: 503 }\n");
        let mut fourth = Vec::new();
        assert!(
            again
                .request(&view("192.0.2.1", &get, b"/", &h), &mut fourth)
                .is_some()
        );
        drop(first);
        assert!(
            again
                .request(&view("192.0.2.1", &get, b"/", &h), &mut fourth)
                .is_none()
        );
    }

    #[test]
    fn bad_settings() {
        let err = |yaml: &str| {
            let cfg = parse(&format!(
                "sites:\n  - id: x\n    hosts: [t]\n    upstream: http://127.0.0.1:1\n    plugins:\n{yaml}"
            ))
            .unwrap();
            match Chain::build(
                &cfg.sites[0].plugins,
                "x",
                None,
                &mut Vec::new(),
                &mut crate::plugin::no_upstreams,
            ) {
                Ok(_) => panic!("built"),
                Err(e) => e,
            }
        };
        assert!(err("      - rate_limit: { requests: 0 }\n").contains("at least 1"));
        assert!(err("      - rate_limit: { requests: 1, per: 10ms }\n").contains("100ms"));
        assert!(
            err("      - rate_limit: { requests: 1, key: cookie }\n")
                .contains("expected client_ip")
        );
        assert!(err("      - concurrency: { max: 1, status: 200 }\n").contains("4xx or 5xx"));
    }
}
