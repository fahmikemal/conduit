//! `basic_auth` and `forward_auth`: plugins that may have to wait before a request goes on,
//! for a password hash on the blocking pool or for an auth service's answer. Their `gate`
//! settles what it can at once (no credentials, credentials verified before); `check` does
//! the waiting.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine as _;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, Method, Uri, header};
use log::warn;
use prometheus::IntCounter;
use tokio::sync::Semaphore;

use crate::config::{BasicAuthConfig, ForwardAuthConfig};
use crate::httpc;
use crate::limits::Held;
use crate::metrics;
use crate::plugin::{CheckRequest, Gate, Local, Plugin, UpstreamEdits};
use crate::rules::{Matcher, RequestView};

type Hasher = foldhash::fast::RandomState;

/// allow / deny / error counters of one plugin on one site.
struct Counters {
    allow: IntCounter,
    deny: IntCounter,
    error: IntCounter,
}

impl Counters {
    fn new(site: &str, plugin: &str) -> Counters {
        let c = |result| metrics::AUTH.with_label_values(&[site, plugin, result]);
        Counters {
            allow: c("allow"),
            deny: c("deny"),
            error: c("error"),
        }
    }
}

fn refused(mut local: Local, by: &'static str) -> Local {
    local.blocked_by = Some(by);
    local
}

/// Logs at most once a minute per call site.
fn warn_sometimes(last: &AtomicU64, msg: impl FnOnce() -> String) {
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    let now = START.elapsed().as_secs() + 60;
    let prev = last.load(Relaxed);
    if now >= prev + 60 && last.compare_exchange(prev, now, Relaxed, Relaxed).is_ok() {
        warn!("{}", msg());
    }
}

/// Verifications of known credentials kept, after which the cache starts over.
const VERIFIED_MAX: usize = 1024;

/// bcrypt runs on the blocking pool, at most one per CPU at a time: a burst of wrong
/// passwords costs CPU time, never the threads serving other requests.
static HASHING: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    Arc::new(Semaphore::new(n))
});

/// Longest wait for a turn at hashing before answering 503.
const HASHING_WAIT: Duration = Duration::from_secs(5);

/// Per-process key for the verified-credentials cache, so it never holds anything that
/// could be checked against a guess outside this process.
static CACHE_KEY: LazyLock<[u8; 32]> = LazyLock::new(|| {
    let mut k = [0; 32];
    openssl::rand::rand_bytes(&mut k).expect("random bytes");
    k
});

pub struct BasicAuth {
    users: HashMap<String, Arc<str>, Hasher>,
    /// A configured hash, verified for unknown users so that they take as long as known ones.
    decoy: Arc<str>,
    challenge: HeaderValue,
    matcher: Option<Matcher>,
    /// Keys (`cache_key`) of credentials verified since this configuration loaded.
    verified: Mutex<HashSet<[u8; 32], Hasher>>,
    counters: Counters,
}

/// `user:password` from `Authorization: Basic ...`.
pub(crate) fn credentials(headers: &http::HeaderMap) -> Option<Vec<u8>> {
    let v = headers.get(header::AUTHORIZATION)?.as_bytes();
    let (scheme, rest) = v.split_at_checked(6)?;
    if !scheme.eq_ignore_ascii_case(b"basic ") {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(rest.trim_ascii())
        .ok()?;
    decoded.contains(&b':').then_some(decoded)
}

fn cache_key(credentials: &[u8]) -> [u8; 32] {
    let mut h = openssl::sha::Sha256::new();
    h.update(&*CACHE_KEY);
    h.update(credentials);
    h.finish()
}

/// bcrypt on the blocking pool. `None`: no turn within `HASHING_WAIT`.
async fn verify(password: Vec<u8>, hash: Arc<str>) -> Option<bool> {
    let permit = tokio::time::timeout(HASHING_WAIT, HASHING.clone().acquire_owned())
        .await
        .ok()?
        .ok()?;
    tokio::task::spawn_blocking(move || {
        // Held until the hash is done, even when the request is gone by then.
        let _permit = permit;
        bcrypt::verify(&password, &hash).unwrap_or(false)
    })
    .await
    .ok()
}

impl BasicAuth {
    pub fn new(
        c: &BasicAuthConfig,
        site: &str,
        watched: &mut Vec<std::path::PathBuf>,
    ) -> Result<BasicAuth, String> {
        let mut users: HashMap<String, Arc<str>, Hasher> = HashMap::default();
        let mut add = |user: &str, hash: &str, from: &str| -> Result<(), String> {
            if user.is_empty() || user.contains(':') {
                return Err(format!("{from}: invalid user name {user:?}"));
            }
            if hash.parse::<bcrypt::HashParts>().is_err() {
                return Err(format!(
                    "{from}: user {user:?}: not a bcrypt hash; make one with \
                     `htpasswd -nB {user}` or `caddy hash-password`"
                ));
            }
            if users.insert(user.to_string(), Arc::from(hash)).is_some() {
                return Err(format!("{from}: user {user:?} is listed twice"));
            }
            Ok(())
        };
        if let Some(path) = &c.htpasswd {
            watched.push(path.into());
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("htpasswd {path}: {e}"))?;
            for (n, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let from = format!("htpasswd {path} line {}", n + 1);
                let (user, hash) = line
                    .split_once(':')
                    .ok_or_else(|| format!("{from}: expected user:hash"))?;
                add(user, hash, &from)?;
            }
        }
        for (user, hash) in &c.users {
            add(user, hash, "users")?;
        }
        let Some(decoy) = users.values().next().cloned() else {
            return Err("no users: set users or htpasswd".into());
        };
        if c.realm
            .chars()
            .any(|ch| ch == '"' || ch == '\\' || ch.is_control())
        {
            return Err(format!(
                "realm {:?}: no quotes, backslashes or control characters",
                c.realm
            ));
        }
        let challenge =
            HeaderValue::try_from(format!("Basic realm=\"{}\", charset=\"UTF-8\"", c.realm))
                .map_err(|_| format!("realm {:?} does not fit in a header", c.realm))?;
        Ok(BasicAuth {
            users,
            decoy,
            challenge,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            verified: Mutex::new(HashSet::default()),
            counters: Counters::new(site, "basic_auth"),
        })
    }

    fn challenge(&self) -> Local {
        refused(
            Local::with_body(
                401,
                vec![(header::WWW_AUTHENTICATE, self.challenge.clone())],
                Bytes::new(),
            ),
            "basic_auth",
        )
    }
}

#[async_trait]
impl Plugin for BasicAuth {
    fn name(&self) -> &'static str {
        "basic_auth"
    }

    fn needs_path(&self) -> bool {
        self.matcher.is_some()
    }

    fn gate(&self, req: &RequestView<'_>, _held: &mut Vec<Held>) -> Gate {
        if self.matcher.as_ref().is_some_and(|m| !m.is_match(req)) {
            return Gate::Pass;
        }
        let Some(cred) = credentials(req.headers) else {
            self.counters.deny.inc();
            return Gate::Answer(self.challenge());
        };
        let known = self.verified.lock().unwrap().contains(&cache_key(&cred));
        if known {
            self.counters.allow.inc();
            Gate::Pass
        } else {
            Gate::Check
        }
    }

    async fn check(&self, r: &CheckRequest<'_>) -> Result<UpstreamEdits, Local> {
        let Some(cred) = credentials(&r.req.headers) else {
            return Err(self.challenge());
        };
        let colon = cred.iter().position(|b| *b == b':').unwrap_or(cred.len());
        let user = std::str::from_utf8(&cred[..colon]).ok();
        let (hash, known) = match user.and_then(|u| self.users.get(u)) {
            Some(h) => (h.clone(), true),
            None => (self.decoy.clone(), false),
        };
        let password = cred.get(colon + 1..).unwrap_or_default().to_vec();
        match verify(password, hash).await {
            Some(true) if known => {
                let mut verified = self.verified.lock().unwrap();
                if verified.len() >= VERIFIED_MAX {
                    verified.clear();
                }
                verified.insert(cache_key(&cred));
                self.counters.allow.inc();
                Ok(Vec::new())
            }
            Some(_) => {
                self.counters.deny.inc();
                Err(self.challenge())
            }
            None => {
                self.counters.error.inc();
                Err(refused(Local::status(503), "basic_auth"))
            }
        }
    }
}

/// Largest answer taken from an auth service (a login page, say).
const FORWARD_MAX_BODY: usize = 1 << 20;

pub struct ForwardAuth {
    url: Uri,
    copy: Vec<HeaderName>,
    timeout: Duration,
    matcher: Option<Matcher>,
    client: httpc::Client,
    counters: Counters,
    warned: AtomicU64,
}

static X_FORWARDED_METHOD: HeaderName = HeaderName::from_static("x-forwarded-method");
static X_FORWARDED_URI: HeaderName = HeaderName::from_static("x-forwarded-uri");
static X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
static X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
static X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");

/// Client headers that go to the auth service: not those about the client's own connection,
/// and not the forwarding headers conduit sets itself.
fn passed_to_auth(name: &HeaderName) -> bool {
    let n = name.as_str();
    !matches!(
        n,
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "upgrade"
            | "expect"
            | "forwarded"
            | "x-real-ip"
    ) && !n.starts_with("x-forwarded-")
}

/// Headers of the auth service's answer that go back to the client with it.
fn passed_to_client(name: &HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "content-length"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "upgrade"
    )
}

impl ForwardAuth {
    pub fn new(c: &ForwardAuthConfig, site: &str) -> Result<ForwardAuth, String> {
        let url: Uri = c.url.parse().map_err(|_| format!("url {:?}", c.url))?;
        if !matches!(url.scheme_str(), Some("http" | "https")) || url.host().is_none() {
            return Err(format!(
                "url {:?}: expected http:// or https:// with a host",
                c.url
            ));
        }
        if c.timeout.0.is_zero() {
            return Err("timeout must be above 0".into());
        }
        let copy = c
            .copy_headers
            .iter()
            .map(|h| HeaderName::try_from(h.as_str()).map_err(|_| format!("copy_headers: {h:?}")))
            .collect::<Result<_, _>>()?;
        Ok(ForwardAuth {
            url,
            copy,
            timeout: c.timeout.0,
            matcher: c.matcher.as_ref().map(Matcher::compile).transpose()?,
            client: httpc::Client::new(c.ca_file.as_deref(), c.timeout.0, FORWARD_MAX_BODY, 64),
            counters: Counters::new(site, "forward_auth"),
            warned: AtomicU64::new(0),
        })
    }

    fn request_headers(&self, r: &CheckRequest<'_>) -> Vec<(HeaderName, HeaderValue)> {
        let req = r.req;
        let mut h: Vec<(HeaderName, HeaderValue)> = Vec::with_capacity(req.headers.len() + 5);
        h.extend(
            req.headers
                .iter()
                .filter(|(n, _)| passed_to_auth(n))
                .map(|(n, v)| (n.clone(), v.clone())),
        );
        h.push((
            X_FORWARDED_METHOD.clone(),
            HeaderValue::from_str(req.method.as_str()).unwrap_or(HeaderValue::from_static("GET")),
        ));
        let uri = req.raw_path();
        let uri = if uri.first() == Some(&b'/') {
            HeaderValue::from_bytes(uri).ok()
        } else {
            req.uri
                .path_and_query()
                .and_then(|p| HeaderValue::from_str(p.as_str()).ok())
        };
        h.push((
            X_FORWARDED_URI.clone(),
            uri.unwrap_or(HeaderValue::from_static("/")),
        ));
        let host = req.headers.get(header::HOST).cloned().or_else(|| {
            req.uri
                .authority()
                .and_then(|a| HeaderValue::from_str(a.as_str()).ok())
        });
        if let Some(host) = host {
            h.push((X_FORWARDED_HOST.clone(), host));
        }
        h.push((
            X_FORWARDED_PROTO.clone(),
            HeaderValue::from_static(r.scheme),
        ));
        if let Some(ip) = r.client
            && let Ok(v) = HeaderValue::try_from(ip.to_string())
        {
            h.push((X_FORWARDED_FOR.clone(), v));
        }
        h
    }
}

#[async_trait]
impl Plugin for ForwardAuth {
    fn name(&self) -> &'static str {
        "forward_auth"
    }

    fn needs_path(&self) -> bool {
        self.matcher.is_some()
    }

    fn gate(&self, req: &RequestView<'_>, _held: &mut Vec<Held>) -> Gate {
        if self.matcher.as_ref().is_some_and(|m| !m.is_match(req)) {
            Gate::Pass
        } else {
            Gate::Check
        }
    }

    async fn check(&self, r: &CheckRequest<'_>) -> Result<UpstreamEdits, Local> {
        let headers = self.request_headers(r);
        let reply = tokio::time::timeout(
            self.timeout,
            self.client
                .send(Method::GET, &self.url, &headers, Bytes::new(), true),
        )
        .await;
        let reply = match reply {
            Ok(Ok(reply)) => reply,
            failed => {
                self.counters.error.inc();
                warn_sometimes(&self.warned, || match failed {
                    Ok(Err(e)) => format!("forward_auth {}: {e}", self.url),
                    _ => format!(
                        "forward_auth {}: no answer within {:?}",
                        self.url, self.timeout
                    ),
                });
                return Err(refused(Local::status(502), "forward_auth"));
            }
        };
        if reply.status.is_success() {
            self.counters.allow.inc();
            return Ok(self
                .copy
                .iter()
                .map(|n| (n.clone(), reply.headers.get(n).cloned()))
                .collect());
        }
        self.counters.deny.inc();
        let headers = reply
            .headers
            .iter()
            .filter(|(n, _)| passed_to_client(n))
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();
        Err(refused(
            Local::with_body(reply.status.as_u16(), headers, reply.body),
            "forward_auth",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MatchConfig;
    use pingora::http::RequestHeader;

    fn req(path: &str, headers: &[(&str, &str)]) -> RequestHeader {
        let mut r = RequestHeader::build("GET", path.as_bytes(), None).unwrap();
        for (k, v) in headers {
            r.append_header(k.to_string(), *v).unwrap();
        }
        r
    }

    fn view<'a>(r: &'a RequestHeader, path: &'a [u8]) -> RequestView<'a> {
        RequestView {
            method: &r.method,
            https: true,
            host: "t",
            client: "192.0.2.1".parse().ok(),
            path,
            uri: path,
            query: b"",
            headers: &r.headers,
        }
    }

    fn basic(user_pass: &str) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(user_pass)
        )
    }

    #[test]
    fn credentials_parsing() {
        let h = |v: &str| {
            let mut m = http::HeaderMap::new();
            m.insert(header::AUTHORIZATION, HeaderValue::from_str(v).unwrap());
            credentials(&m)
        };
        assert_eq!(
            h(&basic("alice:pa:ss")).as_deref(),
            Some(&b"alice:pa:ss"[..])
        );
        assert_eq!(h(&basic("alice:")).as_deref(), Some(&b"alice:"[..]));
        assert!(h(&basic("alice:x").replace("Basic", "bAsIc")).is_some());
        assert_eq!(h(&basic("no-colon")), None);
        assert_eq!(h("Basic !!!"), None);
        assert_eq!(h("Bearer abc"), None);
        assert_eq!(h("Basic"), None);
        assert_eq!(credentials(&http::HeaderMap::new()), None);
    }

    fn config(users: &[(&str, &str)]) -> BasicAuthConfig {
        BasicAuthConfig {
            users: users
                .iter()
                .map(|(u, h)| (u.to_string(), h.to_string()))
                .collect(),
            htpasswd: None,
            realm: "restricted".into(),
            matcher: None,
        }
    }

    #[test]
    fn basic_auth_settings() {
        let hash = bcrypt::hash("x", 4).unwrap();
        let err = |c: &BasicAuthConfig| BasicAuth::new(c, "t", &mut Vec::new()).err().unwrap();
        assert!(err(&config(&[])).contains("no users"));
        assert!(err(&config(&[("a", "$apr1$x$y")])).contains("not a bcrypt hash"));
        assert!(err(&config(&[("a:b", &hash)])).contains("invalid user name"));
        let mut c = config(&[("a", &hash)]);
        c.realm = "say \"hi\"".into();
        assert!(err(&c).contains("realm"));

        let dir = std::env::temp_dir().join(format!("conduit-htpasswd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("htpasswd");
        std::fs::write(&file, format!("# team\nbob:{hash}\n\n")).unwrap();
        let mut c = config(&[("a", &hash)]);
        c.htpasswd = Some(file.to_string_lossy().into());
        let mut watched = Vec::new();
        let b = BasicAuth::new(&c, "t", &mut watched).unwrap();
        assert_eq!(watched, std::slice::from_ref(&file));
        assert!(b.users.contains_key("bob") && b.users.contains_key("a"));
        std::fs::write(&file, format!("a:{hash}\n")).unwrap();
        assert!(err(&c).contains("listed twice"));
        std::fs::write(&file, "nocolon\n").unwrap();
        assert!(err(&c).contains("line 1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    async fn check(b: &BasicAuth, r: &RequestHeader) -> Result<UpstreamEdits, Local> {
        b.check(&CheckRequest {
            req: r,
            client: None,
            scheme: "https",
        })
        .await
    }

    #[tokio::test]
    async fn basic_auth_flow() {
        let hash = bcrypt::hash("wonderland", 4).unwrap();
        let mut c = config(&[("alice", &hash)]);
        c.matcher = Some(MatchConfig {
            path: Some(crate::config::StrMatch::Prefix("/admin".into())),
            ..Default::default()
        });
        let b = BasicAuth::new(&c, "auth-test", &mut Vec::new()).unwrap();
        let run = |r: &RequestHeader| b.gate(&view(r, r.raw_path()), &mut Vec::new());

        // Outside the match: nothing asked.
        assert!(matches!(run(&req("/public", &[])), Gate::Pass));
        // No credentials: the challenge at once.
        let Gate::Answer(l) = run(&req("/admin", &[])) else {
            panic!()
        };
        assert_eq!(l.status, 401);
        assert_eq!(l.blocked_by, Some("basic_auth"));
        assert_eq!(
            l.headers[0].1,
            "Basic realm=\"restricted\", charset=\"UTF-8\""
        );

        let good = req("/admin", &[("authorization", &basic("alice:wonderland"))]);
        assert!(matches!(run(&good), Gate::Check));
        assert!(matches!(check(&b, &good).await, Ok(edits) if edits.is_empty()));
        // Verified once, then settled without hashing.
        assert!(matches!(run(&good), Gate::Pass));

        for wrong in ["alice:Wonderland", "bob:wonderland", "alice:"] {
            let r = req("/admin", &[("authorization", &basic(wrong))]);
            assert!(matches!(run(&r), Gate::Check), "{wrong}");
            assert_eq!(check(&b, &r).await.err().unwrap().status, 401, "{wrong}");
            assert!(matches!(run(&r), Gate::Check), "{wrong} was cached");
        }
        let allow = metrics::AUTH.with_label_values(&["auth-test", "basic_auth", "allow"]);
        let deny = metrics::AUTH.with_label_values(&["auth-test", "basic_auth", "deny"]);
        assert_eq!((allow.get(), deny.get()), (2, 4));
    }

    #[test]
    fn forward_auth_request() {
        let c = ForwardAuthConfig {
            url: "http://127.0.0.1:9091/api/verify?rd=x".into(),
            copy_headers: vec!["Remote-User".into()],
            timeout: crate::units::Dur(Duration::from_secs(1)),
            ca_file: None,
            matcher: None,
        };
        let f = ForwardAuth::new(&c, "t").unwrap();
        let r = req(
            "/app?x=1",
            &[
                ("host", "app.test"),
                ("cookie", "session=1"),
                ("x-forwarded-uri", "/evil"),
                ("x-forwarded-for", "203.0.113.9"),
                ("x-real-ip", "203.0.113.9"),
                ("connection", "keep-alive"),
                ("content-length", "5"),
            ],
        );
        let h = f.request_headers(&CheckRequest {
            req: &r,
            client: "192.0.2.7".parse().ok(),
            scheme: "https",
        });
        let get = |n: &str| -> Vec<&str> {
            h.iter()
                .filter(|(k, _)| k == n)
                .map(|(_, v)| v.to_str().unwrap())
                .collect()
        };
        assert_eq!(get("cookie"), ["session=1"]);
        assert_eq!(get("x-forwarded-uri"), ["/app?x=1"]);
        assert_eq!(get("x-forwarded-for"), ["192.0.2.7"]);
        assert_eq!(get("x-forwarded-host"), ["app.test"]);
        assert_eq!(get("x-forwarded-proto"), ["https"]);
        assert_eq!(get("x-forwarded-method"), ["GET"]);
        for gone in ["host", "x-real-ip", "connection", "content-length"] {
            assert!(get(gone).is_empty(), "{gone}");
        }
        let bad = |url: &str| {
            let mut c = c.clone();
            c.url = url.into();
            ForwardAuth::new(&c, "t").err().unwrap()
        };
        assert!(bad("ftp://x/").contains("http://"));
        assert!(bad("/verify").contains("http://"));
    }
}
