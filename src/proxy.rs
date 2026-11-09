//! The request pipeline: local answers (ACME, unknown host, HTTPS redirect, body limit,
//! plugins) first, then a backend from the site's pool, forwarding headers, metrics and
//! the access log.

use std::borrow::Cow;
use std::cell::Cell;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderName, HeaderValue, Method, header};
use pingora::cache::key::HashBinary;
use pingora::cache::{CacheKey, CacheMeta, CacheOptionOverrides, NoCacheReason, RespCacheable};
use pingora::http::{CasedHeaderName, RequestHeader, ResponseHeader};
use pingora::modules::http::compression::ResponseCompression;
use pingora::protocols::Digest;
use pingora::proxy::{FailToProxy, ProxyHttp, RangeType, Session, range_header_filter};
use pingora::upstreams::peer::{H1UpgradePolicy, HttpPeer};
use pingora::{Error, ErrorSource, ErrorType, Result};

use crate::accesslog::{AccessLog, Entry};
use crate::cache;
use crate::config::{AdminSection, ListenSection, UpgradeMode};
use crate::files::{self, Accepted, Found, Lookup};
use crate::metrics;
use crate::pathnorm;
use crate::plugin::{CheckRequest, Effects, Local, Step, UpstreamEdits};
use crate::replace;
use crate::rules::RequestView;
use crate::runtime::{Mtls, Runtime, Site};
use crate::tls::{CertStore, ClientCert, ClientStatus};
use crate::upstream::{Lease, Pool};

/// Outcome of the latest configuration load, for `/status`.
pub struct ReloadStatus {
    pub at: SystemTime,
    pub ok: bool,
    pub error: Option<String>,
}

/// State shared by the proxy, the TLS callback, the admin endpoints and the reloader.
pub struct Shared {
    pub runtime: ArcSwap<Runtime>,
    pub certs: Arc<ArcSwap<CertStore>>,
    pub access_log: AccessLog,
    pub config_path: PathBuf,
    /// Settings that only change with a restart or graceful upgrade.
    pub listen: ListenSection,
    pub admin: AdminSection,
    pub started: SystemTime,
    pub last_reload: Mutex<ReloadStatus>,
    /// Set by the ACME service after storing a certificate; the reloader picks it up.
    pub reload_now: tokio::sync::Notify,
    /// Set by the reloader after a reload, so the ACME service sees new sites at once.
    pub acme_wake: tokio::sync::Notify,
    pub acme: crate::acme::Status,
    /// Keep pooled upstream connections apart per worker thread; see [worker_pool_group].
    pub pool_per_worker: bool,
}

pub struct Conduit {
    shared: Arc<Shared>,
}

impl Conduit {
    pub fn new(shared: Arc<Shared>) -> Self {
        Conduit { shared }
    }
}

/// The pool group of upstream connections opened on this worker thread.
///
/// With `work_stealing: false` each worker thread runs its own Tokio runtime, and a socket
/// stays registered with the runtime that opened it. Reused by a request on another worker,
/// each of its readiness events would wake the opening worker, which then wakes the request's
/// worker. Pingora pools connections by peer, and the group is part of the peer, so one group
/// per worker keeps each worker on its own connections.
fn worker_pool_group() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    thread_local! {
        static GROUP: u64 = NEXT.fetch_add(1, Relaxed);
    }
    GROUP.with(|g| *g)
}

pub struct Ctx {
    start: Instant,
    rt: Option<Arc<Runtime>>,
    site: Option<Arc<Site>>,
    host: String,
    https: bool,
    client: Option<IpAddr>,
    trusted_peer: bool,
    request_id: String,
    /// The backend of the current attempt.
    lease: Option<Lease>,
    /// Backends this request failed to connect to.
    failed: Vec<SocketAddr>,
    attempts: u32,
    upstream_start: Option<Instant>,
    upstream_connect: Option<Duration>,
    upstream_header: Option<Duration>,
    upstream_status: Option<u16>,
    request_body: u64,
    /// What refused the client, if something did.
    blocked_by: Option<&'static str>,
    /// What plugins left for the request: in-flight counts (`concurrency`, released when
    /// the request ends), a route, body rewrites.
    fx: Effects,
    /// Body rewrites under way for this response.
    replacing: Vec<replace::Stream>,
    /// Header changes from auth plugins for the request sent upstream.
    upstream_edits: UpstreamEdits,
    /// The site caches and this request may use the cache.
    cache_eligible: bool,
    /// How the cache served the response, for the access log.
    cache_result: Option<&'static str>,
    /// Set once the upstream switched protocols, cleared by `logging`.
    upgrade_log: Option<Box<UpgradeLog>>,
}

/// The access log line of an upgraded connection, for when its task is dropped before
/// `logging` runs: the process ends (after an upgrade or on stop) while the connection is
/// still open, and the runtime drops what it still runs. Kept only for upgraded requests.
struct UpgradeLog {
    shared: Arc<Shared>,
    client: Option<IpAddr>,
    method: String,
    uri: String,
    proto: &'static str,
    tls: Option<String>,
    user_agent: String,
    referer: String,
}

impl Ctx {
    fn new() -> Ctx {
        metrics::IN_FLIGHT.inc();
        Ctx {
            start: Instant::now(),
            rt: None,
            site: None,
            host: String::new(),
            https: false,
            client: None,
            trusted_peer: false,
            request_id: String::new(),
            lease: None,
            failed: Vec::new(),
            attempts: 0,
            upstream_start: None,
            upstream_connect: None,
            upstream_header: None,
            upstream_status: None,
            request_body: 0,
            blocked_by: None,
            fx: Effects::default(),
            replacing: Vec::new(),
            upstream_edits: Vec::new(),
            cache_eligible: false,
            cache_result: None,
            upgrade_log: None,
        }
    }
}

impl Drop for Ctx {
    fn drop(&mut self) {
        metrics::IN_FLIGHT.dec();
        if let Some(u) = self.upgrade_log.take() {
            // Bytes are unknown here; the line says when and why the connection ended.
            u.shared.access_log.write(&Entry {
                ts: SystemTime::now(),
                request_id: &self.request_id,
                site: self.site.as_ref().map(|s| &*s.id),
                host: &self.host,
                client: u.client,
                method: &u.method,
                scheme: if self.https { "https" } else { "http" },
                uri: Cow::Borrowed(&u.uri),
                proto: u.proto,
                status: 101,
                bytes_sent: 0,
                bytes_received: self.request_body,
                duration_ms: self.start.elapsed(),
                upstream: self.lease.as_ref().map(|l| &*l.target().label),
                upstream_status: self.upstream_status,
                upstream_connect_ms: self.upstream_connect,
                upstream_header_ms: self.upstream_header,
                retries: self.attempts.saturating_sub(1),
                tls: u.tls.as_deref(),
                user_agent: Cow::Borrowed(&u.user_agent),
                referer: Cow::Borrowed(&u.referer),
                error: Some("Cancelled"),
                blocked_by: self.blocked_by,
                cache: self.cache_result,
            });
        }
    }
}

static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
static CACHE_STATUS: HeaderName = HeaderName::from_static("cache-status");

// Names added to every proxied message, parsed once and kept in the case they are sent in, so
// inserting them neither parses nor copies the name per request.
static X_FORWARDED_FOR: CasedHeaderName =
    CasedHeaderName::from_static("x-forwarded-for", "X-Forwarded-For");
static X_REAL_IP: CasedHeaderName = CasedHeaderName::from_static("x-real-ip", "X-Real-IP");
static X_FORWARDED_PROTO: CasedHeaderName =
    CasedHeaderName::from_static("x-forwarded-proto", "X-Forwarded-Proto");
static X_FORWARDED_HOST: CasedHeaderName =
    CasedHeaderName::from_static("x-forwarded-host", "X-Forwarded-Host");
// Sent in lower case, as it always has been.
static X_REQUEST_ID_SENT: CasedHeaderName =
    CasedHeaderName::from_static("x-request-id", "x-request-id");

/// 24 hex digits: a random per-process prefix and a counter.
fn new_request_id() -> String {
    static PREFIX: LazyLock<u32> = LazyLock::new(|| {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(std::process::id());
        h.finish() as u32
    });
    // Each worker takes numbers in blocks, so requests do not all bump one shared counter
    // (its cache line would move between cores on every request). IDs stay unique.
    const BLOCK: u64 = 4096;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    thread_local! {
        static RANGE: Cell<(u64, u64)> = const { Cell::new((0, 0)) };
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let n = RANGE.with(|r| {
        let (mut next, mut end) = r.get();
        if next == end {
            next = NEXT.fetch_add(BLOCK, Relaxed);
            end = next + BLOCK;
        }
        r.set((next + 1, end));
        next
    });
    let prefix = *PREFIX;
    let mut id = [0u8; 24];
    for (i, b) in id.iter_mut().enumerate() {
        let nibble = if i < 8 {
            (prefix >> (28 - 4 * i)) & 0xf
        } else {
            ((n >> (60 - 4 * (i - 8))) & 0xf) as u32
        };
        *b = HEX[nibble as usize];
    }
    // ASCII hex digits only.
    String::from_utf8(id.to_vec()).expect("hex is UTF-8")
}

/// An address as header text. IPv4, nearly every client, is written digit by digit rather
/// than through `Display` and `to_string`.
fn ip_text(ip: IpAddr) -> Bytes {
    match ip {
        IpAddr::V4(v4) => {
            let mut out = Vec::with_capacity(15);
            let mut num = itoa::Buffer::new();
            for (i, octet) in v4.octets().into_iter().enumerate() {
                if i > 0 {
                    out.push(b'.');
                }
                out.extend_from_slice(num.format(octet).as_bytes());
            }
            Bytes::from(out)
        }
        IpAddr::V6(_) => Bytes::from(ip.to_string()),
    }
}

/// An id set by a trusted proxy in front, when it looks like one.
pub(crate) fn incoming_request_id(req: &RequestHeader) -> Option<String> {
    let v = req.headers.get(&X_REQUEST_ID)?.as_bytes();
    let ok = (1..=128).contains(&v.len())
        && v.iter()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/+=".contains(b));
    ok.then(|| String::from_utf8_lossy(v).into_owned())
}

/// Host without port, lowercased; empty when missing or malformed.
pub(crate) fn request_host(req: &RequestHeader) -> String {
    let raw = req
        .headers
        .get(header::HOST)
        .map(HeaderValue::as_bytes)
        .or_else(|| req.uri.authority().map(|a| a.as_str().as_bytes()))
        .unwrap_or_default();
    let host = if raw.first() == Some(&b'[') {
        match raw.iter().position(|b| *b == b']') {
            Some(end) => &raw[..=end],
            None => return String::new(),
        }
    } else {
        raw.iter()
            .rposition(|b| *b == b':')
            .map_or(raw, |i| &raw[..i])
    };
    let host = host.strip_suffix(b".").unwrap_or(host);
    let valid = !host.is_empty()
        && host.iter().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'[' | b']' | b':')
        });
    if !valid {
        return String::new();
    }
    String::from_utf8_lossy(host).to_ascii_lowercase()
}

/// Path and query as received.
pub(crate) fn request_uri(req: &RequestHeader) -> &[u8] {
    let raw = req.raw_path();
    if raw.first() == Some(&b'/') {
        raw
    } else {
        req.uri
            .path_and_query()
            .map_or(b"/", |p| p.as_str().as_bytes())
    }
}

pub(crate) fn header_str(req: &RequestHeader, name: HeaderName) -> Cow<'_, str> {
    req.headers
        .get(name)
        .map_or(Cow::Borrowed(""), |v| String::from_utf8_lossy(v.as_bytes()))
}

/// Whether the request's protocol upgrade reaches the upstream under the site's `upgrade`.
fn forwards_upgrade(mode: UpgradeMode, session: &Session) -> bool {
    session.is_upgrade_req()
        && match mode {
            UpgradeMode::Any => true,
            UpgradeMode::Websocket => session
                .req_header()
                .headers
                .get(header::UPGRADE)
                .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket")),
            UpgradeMode::Off => false,
        }
}

pub(crate) fn has_body(req: &RequestHeader) -> bool {
    req.headers.contains_key(header::TRANSFER_ENCODING)
        || req
            .headers
            .get(header::CONTENT_LENGTH)
            .is_some_and(|v| v.as_bytes() != b"0")
}

const ACME_PREFIX: &[u8] = b"/.well-known/acme-challenge/";

fn acme_response(root: &std::path::Path, token: &[u8]) -> Local {
    let valid = !token.is_empty()
        && token.len() <= 256
        && token
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_');
    if !valid {
        return Local::status(404);
    }
    let path = root.join(String::from_utf8_lossy(token).as_ref());
    match std::fs::read(path) {
        Ok(body) => Local::with_body(
            200,
            vec![(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))],
            Bytes::from(body),
        ),
        Err(_) => Local::status(404),
    }
}

/// The scheme the client used: the connection's, or what a trusted proxy in front says.
pub(crate) fn client_scheme(https: bool, trusted_peer: bool, req: &RequestHeader) -> &'static str {
    match req
        .headers
        .get("x-forwarded-proto")
        .map(HeaderValue::as_bytes)
    {
        Some(b"http") if trusted_peer => "http",
        Some(b"https") if trusted_peer => "https",
        _ if https => "https",
        _ => "http",
    }
}

/// What `decide` needs to know about the connection.
pub(crate) struct Conn<'a> {
    pub(crate) https: bool,
    pub(crate) host: &'a str,
    pub(crate) client: Option<IpAddr>,
    /// Set when the handshake asked for a client certificate.
    pub(crate) client_cert: Option<&'a ClientCert>,
}

/// The connection's client certificate record, when its handshake asked for one.
fn client_cert(session: &Session) -> Option<&ClientCert> {
    session
        .digest()?
        .ssl_digest
        .as_ref()?
        .extension
        .get::<ClientCert>()
}

/// Whether an mTLS site refuses the request, from what the connection's handshake showed.
fn check_client_cert(m: &Mtls, conn: &Conn<'_>) -> Option<Local> {
    let status = match conn.client_cert {
        Some(c) if c.trust == m.trust.id => &c.status,
        // The handshake was for another site, or for no site with these CAs: an HTTP/2
        // connection reused for this host, or an SNI naming another host. 421 makes clients
        // retry on a connection of its own.
        _ if conn.https => return Some(refused(421, "mtls")),
        _ => &ClientStatus::Missing,
    };
    let body = match status {
        ClientStatus::Valid { .. } => return None,
        ClientStatus::Missing if m.optional => return None,
        ClientStatus::Missing => "client certificate required\n".to_string(),
        // The client knows its own certificate; telling it what is wrong costs nothing.
        ClientStatus::Invalid(why) => format!("client certificate refused: {why}\n"),
    };
    let mut local = Local::with_body(
        403,
        vec![(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        Bytes::from(body),
    );
    local.blocked_by = Some("mtls");
    Some(local)
}

fn refused(status: u16, by: &'static str) -> Local {
    let mut local = Local::status(status);
    local.blocked_by = Some(by);
    local
}

static X_CLIENT_VERIFY: HeaderName = HeaderName::from_static("x-client-verify");
static X_CLIENT_CERT_SUBJECT: HeaderName = HeaderName::from_static("x-client-cert-subject");
static X_CLIENT_CERT_FINGERPRINT: HeaderName = HeaderName::from_static("x-client-cert-fingerprint");

/// What upstreams of an mTLS site learn about the client certificate. Clients cannot set
/// these headers themselves.
fn client_cert_headers(up: &mut RequestHeader, m: &Mtls, cert: Option<&ClientCert>) -> Result<()> {
    for name in [
        &X_CLIENT_VERIFY,
        &X_CLIENT_CERT_SUBJECT,
        &X_CLIENT_CERT_FINGERPRINT,
    ] {
        up.remove_header(name);
    }
    match cert.filter(|c| c.trust == m.trust.id).map(|c| &c.status) {
        Some(ClientStatus::Valid {
            subject,
            fingerprint,
        }) => {
            up.insert_header(X_CLIENT_VERIFY.clone(), "SUCCESS")?;
            up.insert_header(X_CLIENT_CERT_SUBJECT.clone(), subject.clone())?;
            up.insert_header(X_CLIENT_CERT_FINGERPRINT.clone(), fingerprint.clone())?;
        }
        _ => up.insert_header(X_CLIENT_VERIFY.clone(), "NONE")?,
    }
    Ok(())
}

/// The pool a request goes to: its route's, else its site's.
fn pool_of<'a>(site: &'a Site, fx: &'a Effects) -> Option<&'a Arc<Pool>> {
    fx.route
        .as_ref()
        .and_then(|r| r.pool.as_ref())
        .or(site.pool.as_ref())
}

/// The path and query of a request target (the query without '?', `None` when absent).
fn split_target(uri: &[u8]) -> (&[u8], Option<&[u8]>) {
    match uri.iter().position(|b| *b == b'?') {
        Some(i) => (&uri[..i], Some(&uri[i + 1..])),
        None => (uri, None),
    }
}

/// The target a routed request is sent upstream with, when its route changes the path.
pub(crate) fn routed_target(fx: &Effects, req: &RequestHeader) -> Option<Result<Vec<u8>>> {
    let route = fx.route.as_ref().filter(|r| r.rewrites())?;
    let (raw_path, query) = split_target(request_uri(req));
    // `OPTIONS *` has no path to change.
    if raw_path.first() != Some(&b'/') {
        return None;
    }
    // Plugins saw the same normalization succeed.
    Some(match pathnorm::normalize(raw_path) {
        Ok(path) => Ok(route.target(&path, query)),
        Err(_) => Err(Error::explain(ErrorType::HTTPStatus(400), "invalid path")),
    })
}

/// Decide whether the request is answered locally. Pure apart from reading small files.
/// What plugins leave for the rest of the request (in-flight counts, a route, body
/// rewrites) goes to `fx`.
pub(crate) fn decide(
    rt: &Runtime,
    site: Option<&Site>,
    req: &RequestHeader,
    conn: &Conn<'_>,
    fx: &mut Effects,
) -> Step {
    let uri = request_uri(req);
    // Pingora passes a query-only target (`GET ?x`) through as is: rules would see `/`
    // while the upstream reads something else. nginx answers 400; so does conduit.
    if uri.first() != Some(&b'/') && !(uri == b"*" && req.method == Method::OPTIONS) {
        return Step::Answer(Local::status(400));
    }
    let raw_path = uri.split(|b| *b == b'?').next().unwrap_or(uri);

    if let Some(token) = raw_path.strip_prefix(ACME_PREFIX) {
        if let Some(answer) = crate::acme::challenge_answer(token) {
            return Step::Answer(Local::with_body(
                200,
                vec![(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))],
                Bytes::from(answer),
            ));
        }
        if let Some(root) = &rt.acme_webroot {
            return Step::Answer(acme_response(root, token));
        }
    }

    let Some(site) = site else {
        let get = req.method == Method::GET || req.method == Method::HEAD;
        return Step::Answer(match (&rt.default_page, get) {
            (Some(page), true) => Local::with_body(
                200,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )],
                page.clone(),
            ),
            _ => Local::status(404),
        });
    };

    if site.force_https && !conn.https {
        let mut location = Vec::with_capacity(8 + conn.host.len() + uri.len());
        location.extend_from_slice(b"https://");
        location.extend_from_slice(conn.host.as_bytes());
        location.extend_from_slice(uri);
        return Step::Answer(Local::redirect(301, &location));
    }

    if let Some(m) = &site.mtls
        && let Some(refusal) = check_client_cert(m, conn)
    {
        return Step::Answer(refusal);
    }

    if let Some(max) = site.max_body {
        let declared = req
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if declared.is_some_and(|len| len > max) {
            return Step::Answer(Local::status(413));
        }
    }

    run_plugins(site, req, conn, fx, 0)
}

/// The site's plugins from `from` on.
fn run_plugins(
    site: &Site,
    req: &RequestHeader,
    conn: &Conn<'_>,
    fx: &mut Effects,
    from: usize,
) -> Step {
    let uri = request_uri(req);
    let (raw_path, query) = match uri.iter().position(|b| *b == b'?') {
        Some(i) => (&uri[..i], &uri[i + 1..]),
        None => (uri, &b""[..]),
    };
    // Plugins matching on the path see it decoded and normalized, so `/%5F/`, `//_` and
    // `/a/../_` all read `/_/` and encoding cannot slip past a rule.
    let normalized;
    let path = if site.plugins.needs_path() {
        match pathnorm::normalize(raw_path) {
            Ok(p) => {
                normalized = p;
                &normalized[..]
            }
            Err(_) => return Step::Answer(Local::status(400)),
        }
    } else {
        raw_path
    };
    site.plugins.run(
        &RequestView {
            method: &req.method,
            https: conn.https,
            host: conn.host,
            client: conn.client,
            path,
            uri,
            query,
            headers: &req.headers,
        },
        fx,
        from,
    )
}

/// CrowdSec AppSec, for sites that ask for it: the request's headers and, when it can be held
/// and replayed upstream, its body go to AppSec, which may refuse the request.
async fn appsec(session: &mut Session, ctx: &Ctx) -> Result<Option<Local>> {
    let (Some(site), Some(rt), Some(client)) = (&ctx.site, &ctx.rt, ctx.client) else {
        return Ok(None);
    };
    if !site.plugins.appsec() {
        return Ok(None);
    }
    let Some(plan) = &rt.crowdsec else {
        return Ok(None);
    };
    let Some(appsec) = &plan.appsec else {
        return Ok(None);
    };
    let len = appsec.body_len(session.req_header());
    let body = if len > 0 {
        // Pingora sends what was read here to the upstream first (the retry buffer), so the
        // body is read once and forwarded whole.
        session.enable_retry_buffering();
        let mut buf = Vec::with_capacity(len);
        while let Some(chunk) = session.read_request_body().await? {
            buf.extend_from_slice(&chunk);
            if buf.len() > len {
                return Err(Error::explain(
                    ErrorType::HTTPStatus(400),
                    "request body longer than its Content-Length",
                ));
            }
        }
        Bytes::from(buf)
    } else {
        Bytes::new()
    };
    let req = session.req_header();
    let verdict = appsec
        .inspect(req, request_uri(req), client, &ctx.host, body)
        .await;
    Ok(match verdict {
        crate::crowdsec::Verdict::Allow => None,
        crate::crowdsec::Verdict::Block(status) => Some(plan.refuse(status, "appsec")),
    })
}

/// Turn response compression on for this request (level 1: the fastest, as nginx's default).
/// Pingora's module saw the request before conduit picked the site, while it was still off,
/// and ignored `Accept-Encoding` then; it is shown the request again.
pub(crate) fn enable_compression(session: &mut Session) {
    let req = session.downstream_session.req_header();
    if let Some(c) = session
        .downstream_modules_ctx
        .get_mut::<ResponseCompression>()
        && c.is_header_phase()
        && !c.is_enabled()
    {
        c.adjust_level(1);
        c.request_filter(req);
    }
}

/// What the file server and the cache decide once every check let the request through.
enum Next {
    Proxy,
    Files(Lookup),
    Answer(Local),
}

/// The second value says whether the request may use the site's cache. A request a
/// `proxy` rule routed is proxied, never served from `files`.
fn files_and_cache(
    site: &Site,
    req: &RequestHeader,
    conn: &Conn<'_>,
    routed: bool,
) -> (Next, bool) {
    let files = site.files.as_ref().filter(|_| !routed);
    if files.is_none() && site.cache.is_none() {
        return (Next::Proxy, false);
    }
    let uri = request_uri(req);
    let (raw_path, query) = match uri.iter().position(|b| *b == b'?') {
        Some(i) => (&uri[..i], &uri[i + 1..]),
        None => (uri, &b""[..]),
    };
    let Ok(path) = pathnorm::normalize(raw_path) else {
        return (Next::Answer(Local::status(400)), false);
    };
    let view = RequestView {
        method: &req.method,
        https: conn.https,
        host: conn.host,
        client: conn.client,
        path: &path,
        uri,
        query,
        headers: &req.headers,
    };
    if let Some(files) = files {
        let get = req.method == Method::GET || req.method == Method::HEAD;
        if get && files.takes(&view) {
            let lookup = Lookup {
                raw_path: raw_path.to_vec(),
                query: query.to_vec(),
                accepted: Accepted::from_request(req),
                path,
            };
            return (Next::Files(lookup), false);
        }
        if site.pool.is_none() {
            let mut l = Local::status(405);
            l.headers
                .push((header::ALLOW, HeaderValue::from_static("GET, HEAD")));
            return (Next::Answer(l), false);
        }
    }
    let eligible = site.cache.as_ref().is_some_and(|c| c.takes(&view, req));
    (Next::Proxy, eligible)
}

/// A small HTML page for status responses that have no body of their own.
fn status_page(code: u16) -> Bytes {
    let reason = http::StatusCode::from_u16(code)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("");
    Bytes::from(format!(
        "<html>\r\n<head><title>{code} {reason}</title></head>\r\n<body>\r\n\
         <center><h1>{code} {reason}</h1></center>\r\n<hr><center>conduit</center>\r\n\
         </body>\r\n</html>\r\n"
    ))
}

/// Headers every response gets: `Server`, a site's plugin changes and HSTS, and the request id.
fn decorate(resp: &mut ResponseHeader, ctx: &Ctx) -> Result<()> {
    let server = match &ctx.rt {
        Some(rt) => rt.server_header.clone(),
        // Before the request was routed: what Pingora's own responses carry.
        None => pingora::protocols::http::server_header(),
    };
    if let Some(server) = server {
        resp.insert_header(header::SERVER, server)?;
    }
    if let Some(site) = &ctx.site {
        site.plugins.response(resp)?;
        if ctx.https
            && let Some(hsts) = &site.hsts
        {
            resp.insert_header(header::STRICT_TRANSPORT_SECURITY, hsts.clone())?;
        }
    }
    if !ctx.request_id.is_empty() {
        resp.insert_header(&X_REQUEST_ID_SENT, ctx.request_id.as_str())?;
    }
    Ok(())
}

async fn respond(session: &mut Session, mut local: Local, ctx: &Ctx) -> Result<()> {
    if local.body.is_empty()
        && local.status >= 300
        && local.status != 304
        && !local.headers.iter().any(|(n, _)| n == header::CONTENT_TYPE)
    {
        local.body = status_page(local.status);
        local
            .headers
            .push((header::CONTENT_TYPE, HeaderValue::from_static("text/html")));
    }
    let mut resp = ResponseHeader::build(local.status, Some(local.headers.len() + 4))?;
    // Appended: an auth service may answer with several Set-Cookie.
    for (name, value) in local.headers {
        resp.append_header(name, value)?;
    }
    resp.insert_header(header::CONTENT_LENGTH, local.body.len())?;
    decorate(&mut resp, ctx)?;
    let head = session.req_header().method == Method::HEAD;
    if has_body(session.req_header()) {
        // The request body was never read; do not reuse the connection.
        session.set_keepalive(None);
    }
    let end = local.body.is_empty() || head;
    session.write_response_header(Box::new(resp), end).await?;
    if !end {
        session.write_response_body(Some(local.body), true).await?;
    }
    Ok(())
}

/// Content types worth compressing when the upstream sent them uncompressed.
pub(crate) fn compressible(resp: &ResponseHeader) -> bool {
    let Some(ct) = resp
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let ct = ct.split(';').next().unwrap_or("").trim();
    const TYPES: [&str; 7] = [
        "application/javascript",
        "application/x-javascript",
        "application/json",
        "application/manifest+json",
        "application/xml",
        "application/rss+xml",
        "image/svg+xml",
    ];
    ct.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("text/"))
        || TYPES.iter().any(|t| ct.eq_ignore_ascii_case(t))
}

pub(crate) fn proto(req: &RequestHeader) -> &'static str {
    match req.version {
        http::Version::HTTP_2 => "HTTP/2",
        http::Version::HTTP_10 => "HTTP/1.0",
        http::Version::HTTP_09 => "HTTP/0.9",
        http::Version::HTTP_3 => "HTTP/3",
        _ => "HTTP/1.1",
    }
}

/// The status sent to the client, or what it would have been.
fn final_status(session: &Session, e: Option<&Error>) -> u16 {
    session
        .response_written()
        .map(|r| r.status.as_u16())
        .unwrap_or_else(|| match e.map(|e| (e.etype(), e.esource())) {
            Some((ErrorType::HTTPStatus(code), _)) => *code,
            Some((t, ErrorSource::Upstream)) => upstream_error_status(t),
            Some((_, ErrorSource::Internal | ErrorSource::Unset)) => 500,
            // The client went away before a response.
            Some((_, ErrorSource::Downstream)) | None => 499,
        })
}

/// 504 when the upstream timed out, 502 for any other upstream failure, as nginx answers.
fn upstream_error_status(t: &ErrorType) -> u16 {
    use ErrorType::{ConnectTimedout, ReadTimedout, TLSHandshakeTimedout, WriteTimedout};
    match t {
        ConnectTimedout | TLSHandshakeTimedout | ReadTimedout | WriteTimedout => 504,
        _ => 502,
    }
}

#[async_trait]
impl ProxyHttp for Conduit {
    type CTX = Ctx;

    fn new_ctx(&self) -> Ctx {
        Ctx::new()
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Ctx) -> Result<bool> {
        let rt = self.shared.runtime.load_full();
        // Pingora keeps an idle HTTP/1.1 client connection open forever, or as long as the
        // client's own Keep-Alive header asks.
        if session.get_keepalive().is_some() {
            session.set_keepalive(Some(rt.client_keepalive_secs));
        }
        ctx.https = session.digest().is_some_and(|d| d.ssl_digest.is_some());
        let peer = session
            .client_addr()
            .and_then(|a| a.as_inet())
            .map(|a| a.ip());
        let req = session.req_header();
        ctx.host = request_host(req);
        let site = rt.router.lookup(&ctx.host).cloned();
        if let Some(peer) = peer {
            let name = site
                .as_ref()
                .and_then(|s| s.real_ip_header.as_ref())
                .unwrap_or(&rt.realip.header);
            let (client, trusted) = rt.realip.resolve(peer, req.headers.get(name));
            ctx.client = Some(client);
            ctx.trusted_peer = trusted;
        }
        ctx.request_id = ctx
            .trusted_peer
            .then(|| incoming_request_id(req))
            .flatten()
            .unwrap_or_else(new_request_id);

        let conn = Conn {
            https: ctx.https,
            host: &ctx.host,
            client: ctx.client,
            client_cert: client_cert(session),
        };
        let mut step = decide(&rt, site.as_deref(), req, &conn, &mut ctx.fx);
        // Plugins that wait (auth) are awaited in order; the chain then goes on after them.
        if let (Step::Check(_), Some(site)) = (&step, &site) {
            let scheme = client_scheme(ctx.https, ctx.trusted_peer, req);
            while let Step::Check(i) = step {
                let check = CheckRequest {
                    req,
                    client: ctx.client,
                    scheme,
                };
                step = match site.plugins.check(i, &check).await {
                    Err(local) => Step::Answer(local),
                    Ok(edits) => {
                        ctx.upstream_edits.extend(edits);
                        run_plugins(site, req, &conn, &mut ctx.fx, i + 1)
                    }
                };
            }
        }
        let (next, cache_eligible) = match (&step, &site) {
            (Step::Pass, Some(site)) => files_and_cache(site, req, &conn, ctx.fx.route.is_some()),
            _ => (Next::Proxy, false),
        };
        ctx.cache_eligible = cache_eligible;
        ctx.site = site;
        ctx.rt = Some(rt);
        let local = match step {
            Step::Answer(local) => Some(local),
            _ => appsec(session, ctx).await?,
        };
        if let Some(local) = local {
            ctx.blocked_by = local.blocked_by;
            respond(session, local, ctx).await?;
            return Ok(true);
        }
        match next {
            Next::Proxy => {}
            Next::Answer(local) => {
                respond(session, local, ctx).await?;
                return Ok(true);
            }
            Next::Files(lookup) => {
                let site = ctx.site.clone().expect("files come from a site");
                let files = site.files.as_ref().expect("site serves files");
                match files.lookup(lookup).await {
                    Found::File(f) => {
                        files::send(session, f, |r| decorate(r, ctx), site.compression).await?;
                        return Ok(true);
                    }
                    Found::Missing if files.pass_thru => {}
                    found => {
                        let local = found.into_local().unwrap_or_else(|| Local::status(404));
                        respond(session, local, ctx).await?;
                        return Ok(true);
                    }
                }
            }
        }

        if ctx.site.as_ref().is_some_and(|s| s.compression) {
            enable_compression(session);
        }
        Ok(false)
    }

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body: &mut Option<Bytes>,
        _end_of_stream: bool,
        ctx: &mut Ctx,
    ) -> Result<()> {
        if let Some(chunk) = body.as_ref() {
            ctx.request_body += chunk.len() as u64;
            // After a 101 the client's bytes are the new protocol's stream (WebSocket frames),
            // not a request body; nginx's client_max_body_size leaves them alone too. Only a
            // completed handshake counts: an `Upgrade` header the upstream declined does not
            // lift the limit.
            if !session.was_upgraded()
                && ctx
                    .site
                    .as_ref()
                    .and_then(|s| s.max_body)
                    .is_some_and(|max| ctx.request_body > max)
            {
                return Err(Error::explain(
                    ErrorType::HTTPStatus(413),
                    "request body larger than max_body_size",
                ));
            }
        }
        Ok(())
    }

    async fn upstream_peer(&self, session: &mut Session, ctx: &mut Ctx) -> Result<Box<HttpPeer>> {
        let (Some(site), Some(rt)) = (ctx.site.as_ref(), ctx.rt.as_ref()) else {
            return Err(Error::explain(ErrorType::InternalError, "no site selected"));
        };
        let Some(pool) = pool_of(site, &ctx.fx) else {
            return Err(Error::explain(
                ErrorType::HTTPStatus(404),
                "site has no upstream",
            ));
        };
        let key = pool.key(session.req_header(), ctx.client);
        let Some(lease) = pool.select(&key, &ctx.failed) else {
            return Err(Error::explain(
                ErrorType::HTTPStatus(502),
                format!("pool {}: no backend left to try", pool.name),
            ));
        };
        if ctx.attempts > 0 {
            pool.note_retry();
        }
        ctx.attempts += 1;
        let t = lease.target();
        let mut peer = HttpPeer::new(t.addr, t.tls, t.sni.clone());
        if self.shared.pool_per_worker {
            peer.group_key = worker_pool_group();
        }
        let o = &mut peer.options;
        o.connection_timeout = Some(site.connect_timeout);
        o.read_timeout = Some(site.read_timeout);
        o.write_timeout = Some(site.write_timeout);
        o.idle_timeout = Some(rt.upstream_idle_timeout);
        o.http_upstream_request_policy.h1_upgrade = match site.upgrade {
            UpgradeMode::Websocket => H1UpgradePolicy::WebSocketOnly,
            UpgradeMode::Any => H1UpgradePolicy::Preserve,
            UpgradeMode::Off => H1UpgradePolicy::Deny,
        };
        if forwards_upgrade(site.upgrade, session) {
            // Once upgraded, each side's read timeout is the connection's idle limit: every
            // byte in either direction restarts both. Pingora's own default for the client
            // side is a fixed 60s.
            o.read_timeout = Some(site.upgrade_idle_timeout);
            session.set_read_timeout(Some(site.upgrade_idle_timeout));
        }
        if t.tls {
            o.total_connection_timeout = Some(site.connect_timeout * 2);
            o.verify_cert = t.verify;
            o.verify_hostname = t.verify;
        }
        ctx.lease = Some(lease);
        ctx.upstream_start = Some(Instant::now());
        ctx.upstream_connect = None;
        Ok(Box::new(peer))
    }

    fn fail_to_connect(
        &self,
        _session: &mut Session,
        _peer: &HttpPeer,
        ctx: &mut Ctx,
        mut e: Box<Error>,
    ) -> Box<Error> {
        if let (Some(pool), Some(lease)) = (
            ctx.site.as_ref().and_then(|s| pool_of(s, &ctx.fx)),
            &ctx.lease,
        ) {
            pool.report_failure(lease.target());
            ctx.failed.push(lease.target().addr);
            // Nothing reached the backend, so another one may take any request.
            let n = ctx.failed.len();
            e.set_retry(n <= pool.retries && n < pool.len());
        }
        e
    }

    async fn connected_to_upstream(
        &self,
        _session: &mut Session,
        _reused: bool,
        _peer: &HttpPeer,
        #[cfg(unix)] _fd: std::os::unix::io::RawFd,
        #[cfg(windows)] _sock: std::os::windows::io::RawSocket,
        _digest: Option<&Digest>,
        ctx: &mut Ctx,
    ) -> Result<()> {
        ctx.upstream_connect = ctx.upstream_start.map(|s| s.elapsed());
        Ok(())
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        up: &mut RequestHeader,
        ctx: &mut Ctx,
    ) -> Result<()> {
        if session.is_upgrade_req()
            && ctx
                .site
                .as_ref()
                .is_some_and(|s| s.upgrade == UpgradeMode::Any)
        {
            // Pingora forwards an arbitrary upgrade's headers untouched, since its handshake
            // may need any of them. Drop the hop-by-hop ones no handshake uses, as nginx does.
            for name in ["keep-alive", "te", "proxy-connection"] {
                up.remove_header(name);
            }
        }
        if let Some(client) = ctx.client {
            let ip = HeaderValue::from_maybe_shared(ip_text(client))
                .expect("an IP address is a valid header value");
            let mut prior = up
                .headers
                .get_all(X_FORWARDED_FOR.name())
                .iter()
                .filter_map(|v| v.to_str().ok());
            let xff = match prior.next() {
                // Shares the bytes of `ip`.
                None => ip.clone(),
                Some(first) => {
                    let mut s = String::from(first);
                    for v in prior {
                        s.push_str(", ");
                        s.push_str(v);
                    }
                    s.push_str(", ");
                    s.push_str(ip.to_str().unwrap_or_default());
                    HeaderValue::from_maybe_shared(Bytes::from(s)).map_err(|_| {
                        Error::explain(ErrorType::InvalidHTTPHeader, "X-Forwarded-For")
                    })?
                }
            };
            up.insert_header(&X_FORWARDED_FOR, xff)?;
            up.insert_header(&X_REAL_IP, ip)?;
        }

        // The Host the client asked for, as received. HTTP/2 clients send :authority
        // instead, which HTTP/1 upstreams need as Host.
        let original_host = session
            .req_header()
            .headers
            .get(header::HOST)
            .cloned()
            .or_else(|| {
                up.uri
                    .authority()
                    .and_then(|a| HeaderValue::from_str(a.as_str()).ok())
            });
        let site = ctx.site.as_ref();
        match (site.and_then(|s| s.upstream_host.clone()), &original_host) {
            (Some(h), _) => up.insert_header(header::HOST, h)?,
            (None, Some(h)) if !up.headers.contains_key(header::HOST) => {
                up.insert_header(header::HOST, h.clone())?
            }
            _ => {}
        }

        // Only a trusted proxy in front (e.g. Cloudflare) may state the original scheme
        // and host.
        up.insert_header(
            &X_FORWARDED_PROTO,
            HeaderValue::from_static(client_scheme(
                ctx.https,
                ctx.trusted_peer,
                session.req_header(),
            )),
        )?;
        let keep_fwd_host = ctx.trusted_peer && up.headers.contains_key(X_FORWARDED_HOST.name());
        if !keep_fwd_host {
            match original_host {
                Some(h) => up.insert_header(&X_FORWARDED_HOST, h)?,
                None => {
                    up.remove_header(X_FORWARDED_HOST.name());
                }
            }
        }
        up.insert_header(&X_REQUEST_ID_SENT, ctx.request_id.as_str())?;
        if let Some(site) = site {
            if let Some(m) = &site.mtls {
                client_cert_headers(up, m, client_cert(session))?;
            }
            for (name, value) in &ctx.upstream_edits {
                match value {
                    Some(v) => up.insert_header(name.clone(), v.clone())?,
                    None => {
                        up.remove_header(name);
                    }
                }
            }
            site.plugins.upstream_request(up)?;
        }
        if let Some(target) = routed_target(&ctx.fx, session.req_header()) {
            up.set_raw_path(&target?)?;
        }
        if !ctx.fx.replace.is_empty() {
            // A body to rewrite has to arrive uncompressed and whole.
            for h in [header::ACCEPT_ENCODING, header::RANGE, header::IF_RANGE] {
                up.remove_header(&h);
            }
        }
        Ok(())
    }

    async fn upstream_response_filter(
        &self,
        session: &mut Session,
        resp: &mut ResponseHeader,
        ctx: &mut Ctx,
    ) -> Result<()> {
        ctx.upstream_status = Some(resp.status.as_u16());
        if resp.status == http::StatusCode::SWITCHING_PROTOCOLS && self.shared.access_log.enabled()
        {
            let req = session.req_header();
            ctx.upgrade_log = Some(Box::new(UpgradeLog {
                shared: self.shared.clone(),
                client: ctx.client.or_else(|| {
                    session
                        .client_addr()
                        .and_then(|a| a.as_inet())
                        .map(|a| a.ip())
                }),
                method: req.method.as_str().to_owned(),
                uri: String::from_utf8_lossy(request_uri(req)).into_owned(),
                proto: proto(req),
                tls: session
                    .digest()
                    .and_then(|d| d.ssl_digest.as_ref())
                    .map(|s| s.version.to_string()),
                user_agent: header_str(req, header::USER_AGENT).into_owned(),
                referer: header_str(req, header::REFERER).into_owned(),
            }));
        }
        ctx.upstream_header = ctx.upstream_start.map(|s| s.elapsed());
        if let (Some(site), Some(lease)) = (&ctx.site, &ctx.lease) {
            if let Some(pool) = pool_of(site, &ctx.fx) {
                pool.report_success(lease.target());
            }
            if let Some(d) = ctx.upstream_header {
                site.metrics.upstream_header(d);
            }
        }
        Ok(())
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        resp: &mut ResponseHeader,
        ctx: &mut Ctx,
    ) -> Result<()> {
        decorate(resp, ctx)?;
        if !ctx.fx.replace.is_empty() {
            let req = session.req_header();
            if replace::rewritable(resp, req.method == Method::HEAD) {
                ctx.replacing = ctx
                    .fx
                    .replace
                    .iter()
                    .filter(|r| r.takes(resp))
                    .map(|r| replace::Stream::new(r.clone()))
                    .collect();
                if !ctx.replacing.is_empty() {
                    replace::rewritten_headers(resp, req.version == http::Version::HTTP_11)?;
                }
            }
        }
        if let Some(site) = &ctx.site
            && site.cache.is_some()
        {
            let (result, status) = cache::outcome(session.cache.phase(), ctx.cache_eligible);
            ctx.cache_result = Some(result);
            resp.insert_header(CACHE_STATUS.clone(), status)?;
        }
        // Like nginx: only whole responses of these statuses are compressed. A 206 carries
        // a byte range of the uncompressed representation and must stay as it is.
        if ctx.site.as_ref().is_some_and(|s| s.compression)
            && let Some(c) = session
                .downstream_modules_ctx
                .get_mut::<ResponseCompression>()
            && c.is_header_phase()
            && (!compressible(resp) || !matches!(resp.status.as_u16(), 200 | 403 | 404))
        {
            c.adjust_level(0);
        }
        Ok(())
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Ctx,
    ) -> Result<Option<Duration>> {
        if ctx.replacing.is_empty() {
            return Ok(None);
        }
        let mut data = body.take().map(Vec::from).unwrap_or_default();
        for s in &mut ctx.replacing {
            data = s.feed(&data, end_of_stream);
        }
        // Nothing may be all there is to send yet: Pingora skips an empty chunk.
        *body = (!data.is_empty()).then(|| Bytes::from(data));
        Ok(None)
    }

    /// Pingora cuts byte ranges of cached responses itself, from the upstream's bytes: a body
    /// `body_replace` changes is sent whole instead.
    fn range_header_filter(
        &self,
        session: &mut Session,
        resp: &mut ResponseHeader,
        ctx: &mut Ctx,
    ) -> RangeType {
        if !ctx.fx.replace.is_empty() {
            return RangeType::None;
        }
        range_header_filter(session.req_header(), resp, Some(200))
    }

    fn request_cache_filter(&self, session: &mut Session, ctx: &mut Ctx) -> Result<()> {
        let Some(rt) = ctx.rt.as_ref().filter(|_| ctx.cache_eligible) else {
            return Ok(());
        };
        let mut overrides = CacheOptionOverrides::default();
        overrides.wait_timeout = Some(rt.cache.lock_timeout.0);
        session.cache.enable(
            &*cache::STORE,
            Some(&*cache::EVICTION),
            Some(&*cache::PREDICTOR),
            Some(cache::lock()),
            Some(overrides),
        );
        if let Some(max) = rt.cache.max_object_size.0 {
            session.cache.set_max_file_size_bytes(max as usize);
        }
        Ok(())
    }

    fn cache_key_callback(&self, session: &Session, ctx: &mut Ctx) -> Result<CacheKey> {
        let Some(site) = &ctx.site else {
            return Err(Error::explain(ErrorType::InternalError, "no site selected"));
        };
        let req = session.req_header();
        // What conduit itself tells the upstream about the client is part of the key: two
        // users behind forward_auth, or two client certificates, never share an entry.
        // The same goes for an X-Forwarded-Host a trusted proxy passes on: upstreams build
        // links from it, and a client could otherwise poison the entry everyone gets.
        let mut identity: Vec<&[u8]> = Vec::new();
        for (name, value) in &ctx.upstream_edits {
            identity.push(name.as_str().as_bytes());
            identity.push(value.as_ref().map_or(b"", |v| v.as_bytes()));
        }
        if let Some(m) = &site.mtls
            && let Some(ClientStatus::Valid { fingerprint, .. }) = client_cert(session)
                .filter(|c| c.trust == m.trust.id)
                .map(|c| &c.status)
        {
            identity.extend([b"\0mtls".as_slice(), fingerprint.as_bytes()]);
        }
        if ctx.trusted_peer
            && let Some(h) = req.headers.get("x-forwarded-host")
        {
            identity.extend([b"\0x-forwarded-host".as_slice(), h.as_bytes()]);
        }
        // A rule may send the same URI elsewhere (by a header, say), and body rewrites
        // ask the upstream for another representation: neither shares entries.
        let target = routed_target(&ctx.fx, req).transpose()?;
        if let Some(pool) = ctx.fx.route.as_ref().and_then(|r| r.pool.as_ref()) {
            identity.extend([b"\0pool".as_slice(), pool.name.as_bytes()]);
        }
        if let Some(t) = &target {
            identity.extend([b"\0target".as_slice(), t]);
        }
        if !ctx.fx.replace.is_empty() {
            identity.push(b"\0body_replace");
        }
        Ok(cache::key(
            &site.id,
            client_scheme(ctx.https, ctx.trusted_peer, req),
            &ctx.host,
            request_uri(req),
            &identity,
        ))
    }

    fn response_cache_filter(
        &self,
        session: &Session,
        resp: &ResponseHeader,
        ctx: &mut Ctx,
    ) -> Result<RespCacheable> {
        Ok(match ctx.site.as_ref().and_then(|s| s.cache.as_ref()) {
            Some(c) => c.cacheable(session.req_header(), resp),
            None => RespCacheable::Uncacheable(NoCacheReason::NeverEnabled),
        })
    }

    fn cache_vary_filter(
        &self,
        meta: &CacheMeta,
        _ctx: &mut Ctx,
        req: &RequestHeader,
    ) -> Option<HashBinary> {
        cache::variance(meta, req)
    }

    /// Stale entries stand in while being revalidated and when the upstream fails, within
    /// the windows the response (`stale-while-revalidate`, `stale-if-error`) or the site's
    /// `stale_if_error` allow; Pingora checks those windows before asking.
    fn should_serve_stale(
        &self,
        _session: &mut Session,
        _ctx: &mut Ctx,
        error: Option<&Error>,
    ) -> bool {
        error.is_none_or(|e| *e.esource() == ErrorSource::Upstream)
    }

    /// Pingora's default error response, with an HTML body and the site's headers.
    async fn fail_to_proxy(&self, session: &mut Session, e: &Error, ctx: &mut Ctx) -> FailToProxy {
        use ErrorType::{ConnectionClosed, HTTPStatus, ReadError, WriteError};
        let code = match e.etype() {
            HTTPStatus(code) => *code,
            t => match e.esource() {
                ErrorSource::Upstream => upstream_error_status(t),
                ErrorSource::Downstream => match e.etype() {
                    WriteError | ReadError | ConnectionClosed => 0, // the client is gone
                    _ => 400,
                },
                ErrorSource::Internal | ErrorSource::Unset => 500,
            },
        };
        if code > 0 {
            let body = if session.req_header().method == Method::HEAD {
                Bytes::new()
            } else {
                status_page(code)
            };
            let resp = ResponseHeader::build(code, Some(4)).and_then(|mut r| {
                r.insert_header(header::CONTENT_TYPE, "text/html")?;
                r.insert_header(header::CONTENT_LENGTH, body.len())?;
                decorate(&mut r, ctx)?;
                Ok(r)
            });
            // Does nothing if a response has already started.
            let sent = match resp {
                Ok(resp) => {
                    session
                        .as_downstream_mut()
                        .write_error_response(resp, body)
                        .await
                }
                Err(e) => Err(e),
            };
            if let Err(err) = sent {
                log::debug!("cannot send {code} to the client: {err}");
            }
        }
        FailToProxy {
            error_code: code,
            can_reuse_downstream: false,
        }
    }

    fn suppress_error_log(&self, _session: &Session, _ctx: &Ctx, error: &Error) -> bool {
        // Clients hanging up are routine and would drown real upstream problems.
        *error.esource() == ErrorSource::Downstream
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Ctx) {
        ctx.upgrade_log = None;
        let elapsed = ctx.start.elapsed();
        let status = final_status(session, e);

        // An upstream error that ended the request after connecting (reset, timeout)
        // counts against the backend like a connect failure does.
        if let (Some(err), Some(pool), Some(lease)) = (
            e,
            ctx.site.as_ref().and_then(|s| pool_of(s, &ctx.fx)),
            &ctx.lease,
        ) && *err.esource() == ErrorSource::Upstream
            && ctx.upstream_status.is_none()
            && !ctx.failed.contains(&lease.target().addr)
        {
            pool.report_failure(lease.target());
        }

        // A background revalidation of a stale cache entry answered no client; like
        // nginx's default (log_subrequest off) it is not counted or logged as a request.
        if session.subrequest_ctx.is_some() {
            return;
        }
        let body_bytes = session.body_bytes_sent();
        match (&ctx.site, &ctx.rt) {
            (Some(site), _) => {
                site.metrics.response(status, elapsed, body_bytes);
                if let Some(r) = ctx.cache_result {
                    site.metrics.cache(r);
                }
            }
            (None, Some(rt)) => rt.default_metrics.response(status, elapsed, body_bytes),
            (None, None) => self
                .shared
                .runtime
                .load()
                .default_metrics
                .response(status, elapsed, body_bytes),
        }

        let log = &self.shared.access_log;
        if !log.enabled() {
            return;
        }
        let req = session.req_header();
        let tls = session
            .digest()
            .and_then(|d| d.ssl_digest.as_ref())
            .map(|s| &*s.version);
        let client = ctx.client.or_else(|| {
            session
                .client_addr()
                .and_then(|a| a.as_inet())
                .map(|a| a.ip())
        });
        log.write(&Entry {
            ts: SystemTime::now(),
            request_id: &ctx.request_id,
            site: ctx.site.as_ref().map(|s| &*s.id),
            host: &ctx.host,
            client,
            method: req.method.as_str(),
            scheme: if ctx.https { "https" } else { "http" },
            uri: String::from_utf8_lossy(request_uri(req)),
            proto: proto(req),
            status,
            bytes_sent: body_bytes,
            bytes_received: ctx.request_body,
            duration_ms: elapsed,
            upstream: ctx.lease.as_ref().map(|l| &*l.target().label),
            upstream_status: ctx.upstream_status,
            upstream_connect_ms: ctx.upstream_connect,
            upstream_header_ms: ctx.upstream_header,
            retries: ctx.attempts.saturating_sub(1),
            tls,
            user_agent: header_str(req, header::USER_AGENT),
            referer: header_str(req, header::REFERER),
            error: e.map(|e| e.etype().as_str()),
            blocked_by: ctx.blocked_by,
            cache: ctx.cache_result,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;
    use crate::runtime::build;

    #[test]
    fn each_worker_thread_has_its_own_pool_group() {
        let here = worker_pool_group();
        assert_eq!(worker_pool_group(), here);
        let there = std::thread::spawn(worker_pool_group).join().unwrap();
        assert_ne!(there, here);
        // 0 is HttpPeer's default group, used by peers built elsewhere (health checks).
        assert_ne!(here, 0);
    }

    fn req(method: &str, target: &str, headers: &[(&str, &str)]) -> RequestHeader {
        let mut r = RequestHeader::build(method, target.as_bytes(), None).unwrap();
        for (k, v) in headers {
            r.insert_header(k.to_string(), *v).unwrap();
        }
        r
    }

    fn runtime(yaml: &str) -> Runtime {
        build(&parse(yaml).unwrap(), None, None).unwrap().runtime
    }

    const BASE: &str = r#"
listen: { http: ["127.0.0.1:0"] }
sites:
  - id: forgejo
    hosts: [forgejo.example.com]
    upstream: http://127.0.0.1:3000
    force_https: true
    plugins:
      - rules:
          - match: { method: [GET, HEAD], headers: { user-agent: { regex: "(?i)mozilla" } } }
            action: { redirect: { to: "https://git.example.com{request_uri}" } }
  - id: pb
    hosts: [pb.example.com]
    upstream: http://127.0.0.1:3001
    max_body_size: 1k
    plugins:
      - ip_filter: { deny: [192.0.2.0/24] }
      - rules:
          - match: { path: { regex: "(?i)^/_(/|$)" } }
            action: { deny: 403 }
"#;

    fn answer(step: Step) -> Option<Local> {
        match step {
            Step::Pass => None,
            Step::Answer(local) => Some(local),
            Step::Check(i) => panic!("plugin #{} waits", i + 1),
        }
    }

    fn run_with(rt: &Runtime, r: &RequestHeader, https: bool, client: &str) -> Option<Local> {
        let host = request_host(r);
        let conn = Conn {
            https,
            host: &host,
            client: client.parse().ok(),
            client_cert: None,
        };
        answer(decide(
            rt,
            rt.router.lookup(&host).map(|s| &**s),
            r,
            &conn,
            &mut Effects::default(),
        ))
    }

    fn run(rt: &Runtime, r: &RequestHeader, https: bool) -> Option<Local> {
        run_with(rt, r, https, "198.51.100.7")
    }

    #[test]
    fn host_parsing() {
        assert_eq!(
            request_host(&req("GET", "/", &[("host", "Forgejo.Example.com:443")])),
            "forgejo.example.com"
        );
        assert_eq!(
            request_host(&req("GET", "/", &[("host", "[::1]:8080")])),
            "[::1]"
        );
        assert_eq!(request_host(&req("GET", "/", &[("host", "a b")])), "");
        assert_eq!(request_host(&req("GET", "/", &[])), "");
    }

    /// Found by fuzzing: a target without a path reached plugins as `/`.
    #[test]
    fn target_without_path_is_400() {
        let rt = runtime(BASE);
        let h = [("host", "forgejo.example.com")];
        for target in ["?_/../x", "?", "*"] {
            let l = run(&rt, &req("GET", target, &h), true).unwrap();
            assert_eq!(l.status, 400, "{target}");
        }
        let star = run(&rt, &req("OPTIONS", "*", &h), true);
        assert!(star.is_none_or(|l| l.status != 400));
        let absolute = req("GET", "http://forgejo.example.com/x?y", &h);
        assert_eq!(request_uri(&absolute), b"/x?y");
    }

    #[test]
    fn https_redirect_runs_before_plugins() {
        let rt = runtime(BASE);
        let r = req(
            "GET",
            "/x?y=1",
            &[
                ("host", "forgejo.example.com"),
                ("user-agent", "Mozilla/5.0"),
            ],
        );
        let l = run(&rt, &r, false).unwrap();
        assert_eq!(l.status, 301);
        assert_eq!(l.headers[0].1, "https://forgejo.example.com/x?y=1");
        let l = run(&rt, &r, true).unwrap();
        assert_eq!(l.status, 302);
        assert_eq!(l.headers[0].1, "https://git.example.com/x?y=1");
        // git clients are proxied
        let git = req(
            "GET",
            "/repo.git/info/refs",
            &[("host", "forgejo.example.com"), ("user-agent", "git/2.47")],
        );
        assert!(run(&rt, &git, true).is_none());
    }

    #[test]
    fn plugins_see_normalized_path_and_client() {
        let rt = runtime(BASE);
        for target in ["/_/", "/%5F/", "//_", "/a/../_/x"] {
            let r = req("GET", target, &[("host", "pb.example.com")]);
            assert_eq!(run(&rt, &r, true).map(|l| l.status), Some(403), "{target}");
        }
        let ok = req("GET", "/api/health", &[("host", "pb.example.com")]);
        assert!(run(&rt, &ok, true).is_none());
        assert_eq!(run_with(&rt, &ok, true, "192.0.2.9").unwrap().status, 403);
        let bad = req("GET", "/../etc", &[("host", "pb.example.com")]);
        assert_eq!(run(&rt, &bad, true).unwrap().status, 400);
    }

    #[test]
    fn routed_requests_skip_files_and_change_target() {
        let rt = runtime(&format!(
            r#"
listen: {{ http: ["127.0.0.1:0"] }}
sites:
  - id: s
    hosts: [s.test]
    upstream: http://127.0.0.1:3000
    files: {{ root: {} }}
    plugins:
      - rules:
          - match: {{ path: {{ prefix: /api/ }} }}
            action: {{ proxy: {{ strip_prefix: /api, add_prefix: /v1 }} }}
          - match: {{ path: {{ prefix: /raw/ }} }}
            action: {{ proxy: {{ upstream: "http://127.0.0.1:3001" }} }}
      - body_replace:
          match: {{ path: {{ prefix: /raw/ }} }}
          replace: [{{ from: a, to: b }}]
"#,
            std::env::temp_dir().display()
        ));
        let site = rt.router.lookup("s.test").unwrap();
        let conn = Conn {
            https: false,
            host: "s.test",
            client: None,
            client_cert: None,
        };
        let step = |target: &str| {
            let r = req("GET", target, &[("host", "s.test")]);
            let mut fx = Effects::default();
            let pass = matches!(decide(&rt, Some(site), &r, &conn, &mut fx), Step::Pass);
            assert!(pass, "{target}");
            let next = files_and_cache(site, &r, &conn, fx.route.is_some()).0;
            let target = routed_target(&fx, &r).map(|t| String::from_utf8(t.unwrap()).unwrap());
            let pool = pool_of(site, &fx).unwrap().name.to_string();
            (
                matches!(next, Next::Files(_)),
                target,
                pool,
                fx.replace.len(),
            )
        };
        assert_eq!(
            step("/api/a%20b?x"),
            (false, Some("/v1/a%20b?x".into()), "site:s".into(), 0)
        );
        assert_eq!(step("/raw/x?y"), (false, None, "site:s:rule1.2".into(), 1));
        assert_eq!(step("/x"), (true, None, "site:s".into(), 0));
    }

    #[test]
    fn body_limit_and_default_site() {
        let rt = runtime(BASE);
        let big = req(
            "POST",
            "/upload",
            &[("host", "pb.example.com"), ("content-length", "4096")],
        );
        assert_eq!(run(&rt, &big, true).unwrap().status, 413);
        let unknown = req("GET", "/", &[("host", "nope.example.org")]);
        assert_eq!(run(&rt, &unknown, true).unwrap().status, 404);
    }

    #[test]
    fn acme_challenge() {
        let dir = std::env::temp_dir().join(format!("conduit-acme-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tok-123_A"), "tok-123_A.thumb").unwrap();
        let yaml = format!(
            "{BASE}tls: {{ acme_webroot: {:?} }}\n",
            dir.to_string_lossy()
        );
        let rt = runtime(&yaml);
        // served even for force_https sites and unknown hosts
        let r = req(
            "GET",
            "/.well-known/acme-challenge/tok-123_A",
            &[("host", "forgejo.example.com")],
        );
        let l = run(&rt, &r, false).unwrap();
        assert_eq!((l.status, &l.body[..]), (200, &b"tok-123_A.thumb"[..]));
        let traversal = req(
            "GET",
            "/.well-known/acme-challenge/..%2Fx",
            &[("host", "a")],
        );
        assert_eq!(run(&rt, &traversal, false).unwrap().status, 404);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn acme_challenge_from_memory() {
        let rt = runtime(BASE);
        let mut published = crate::acme::Published::default();
        published.add("mem-tok_1", "mem-tok_1.thumb");
        // answered on any host, before the HTTPS redirect
        for host in ["forgejo.example.com", "unknown.example.org"] {
            let r = req(
                "GET",
                "/.well-known/acme-challenge/mem-tok_1",
                &[("host", host)],
            );
            let l = run(&rt, &r, false).unwrap();
            assert_eq!((l.status, &l.body[..]), (200, &b"mem-tok_1.thumb"[..]));
        }
        // Tokens this process did not publish go where they went before: to the site.
        let other = req(
            "GET",
            "/.well-known/acme-challenge/other",
            &[("host", "pb.example.com")],
        );
        assert!(run(&rt, &other, false).is_none());
        drop(published);
        let r = req(
            "GET",
            "/.well-known/acme-challenge/mem-tok_1",
            &[("host", "pb.example.com")],
        );
        assert!(run(&rt, &r, false).is_none());
    }

    #[test]
    fn request_ids() {
        let a = new_request_id();
        let b = new_request_id();
        assert_eq!(a.len(), 24);
        assert_ne!(a, b);
        assert_eq!(a[..8], b[..8]);
        assert!(
            a.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')),
            "{a}"
        );
        // The counter in the last 16 digits only grows (other tests may draw ids too).
        let n = |id: &str| u64::from_str_radix(&id[8..], 16).unwrap();
        assert!(n(&b) > n(&a), "{a} {b}");
        // Threads draw from separate blocks: no id is handed out twice.
        let threads: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(|| (0..5000).map(|_| new_request_id()).collect::<Vec<_>>()))
            .collect();
        let ids: Vec<String> = threads
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        let ok = req("GET", "/", &[("x-request-id", "cf-7a1b2c3d")]);
        assert_eq!(incoming_request_id(&ok).as_deref(), Some("cf-7a1b2c3d"));
        let bad = req("GET", "/", &[("x-request-id", "a b")]);
        assert!(incoming_request_id(&bad).is_none());
    }

    #[test]
    fn upstream_timeouts_are_504() {
        for t in [
            ErrorType::ConnectTimedout,
            ErrorType::TLSHandshakeTimedout,
            ErrorType::ReadTimedout,
            ErrorType::WriteTimedout,
        ] {
            assert_eq!(upstream_error_status(&t), 504, "{t:?}");
        }
        for t in [
            ErrorType::ConnectRefused,
            ErrorType::ConnectionClosed,
            ErrorType::ReadError,
            ErrorType::InvalidHTTPHeader,
        ] {
            assert_eq!(upstream_error_status(&t), 502, "{t:?}");
        }
    }

    #[test]
    fn ip_text_matches_display() {
        for ip in [
            "0.0.0.0",
            "127.0.0.1",
            "10.20.30.140",
            "255.255.255.255",
            "1.20.100.9",
            "::1",
            "2001:db8::8a2e:370:7334",
            "::ffff:192.0.2.1",
        ] {
            let ip: IpAddr = ip.parse().unwrap();
            assert_eq!(&ip_text(ip)[..], ip.to_string().as_bytes());
        }
    }

    #[test]
    fn compressible_types() {
        let mut r = ResponseHeader::build(200, None).unwrap();
        r.insert_header("content-type", "text/html; charset=utf-8")
            .unwrap();
        assert!(compressible(&r));
        r.insert_header("content-type", "application/octet-stream")
            .unwrap();
        assert!(!compressible(&r));
        r.insert_header("content-type", "application/json").unwrap();
        assert!(compressible(&r));
    }

    #[test]
    fn client_certificates() {
        let dir = std::env::temp_dir().join(format!("conduit-mtls-decide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (ca, ca2) = (dir.join("ca.pem"), dir.join("ca2.pem"));
        std::fs::write(&ca, crate::tls::test_certs::self_signed(&["ca"]).0).unwrap();
        std::fs::write(&ca2, crate::tls::test_certs::self_signed(&["ca2"]).0).unwrap();
        let rt = runtime(&format!(
            r#"
listen: {{ http: ["127.0.0.1:0"], https: ["127.0.0.1:0"] }}
sites:
  - {{ id: secure, hosts: [secure.test], upstream: "http://127.0.0.1:3000", mtls: {{ ca: {ca} }} }}
  - {{ id: opt, hosts: [opt.test], upstream: "http://127.0.0.1:3000", mtls: {{ ca: {ca}, optional: true }} }}
  - {{ id: other, hosts: [other.test], upstream: "http://127.0.0.1:3000", mtls: {{ ca: {ca2} }} }}
  - {{ id: open, hosts: [open.test], upstream: "http://127.0.0.1:3000" }}
"#,
            ca = ca.display(),
            ca2 = ca2.display()
        ));
        assert!(rt.mtls);
        let site = |h: &str| rt.router.lookup(h).unwrap().clone();
        let id = |h: &str| site(h).mtls.as_ref().unwrap().trust.id;
        // One CA file, one trust.
        assert!(Arc::ptr_eq(
            &site("secure.test").mtls.as_ref().unwrap().trust,
            &site("opt.test").mtls.as_ref().unwrap().trust
        ));
        assert_ne!(id("secure.test"), id("other.test"));
        let valid = |trust| ClientCert {
            trust,
            status: ClientStatus::Valid {
                subject: HeaderValue::from_static("CN=alice"),
                fingerprint: HeaderValue::from_static("00"),
            },
        };
        let missing = ClientCert {
            trust: id("secure.test"),
            status: ClientStatus::Missing,
        };
        let invalid = ClientCert {
            trust: id("secure.test"),
            status: ClientStatus::Invalid("certificate has expired".into()),
        };
        let check = |host: &str, https: bool, cert: Option<&ClientCert>| {
            let r = req("GET", "/", &[("host", host)]);
            let conn = Conn {
                https,
                host,
                client: None,
                client_cert: cert,
            };
            answer(decide(
                &rt,
                Some(&site(host)),
                &r,
                &conn,
                &mut Effects::default(),
            ))
            .map(|l| {
                (
                    l.status,
                    String::from_utf8_lossy(&l.body).into_owned(),
                    l.blocked_by,
                )
            })
        };
        let ok = valid(id("secure.test"));
        assert_eq!(check("secure.test", true, Some(&ok)), None);
        let (status, body, by) = check("secure.test", true, Some(&missing)).unwrap();
        assert_eq!(
            (status, body.as_str(), by),
            (403, "client certificate required\n", Some("mtls"))
        );
        let (status, body, _) = check("secure.test", true, Some(&invalid)).unwrap();
        assert_eq!(
            (status, body.as_str()),
            (403, "client certificate refused: certificate has expired\n")
        );
        // A connection set up for another host or other CAs, or for none: a new connection.
        assert_eq!(check("secure.test", true, None).unwrap().0, 421);
        assert_eq!(
            check("secure.test", true, Some(&valid(id("other.test"))))
                .unwrap()
                .0,
            421
        );
        assert_eq!(check("other.test", true, Some(&ok)).unwrap().0, 421);
        // Plain HTTP has no certificate.
        assert_eq!(check("secure.test", false, None).unwrap().0, 403);
        // Same CAs: a connection verified for one site serves the other.
        assert_eq!(check("opt.test", true, Some(&ok)), None);
        // optional: no certificate is fine, a bad one is not.
        assert_eq!(check("opt.test", true, Some(&missing)), None);
        assert_eq!(check("opt.test", false, None), None);
        assert_eq!(check("opt.test", true, Some(&invalid)).unwrap().0, 403);
        // Sites without mtls ignore it all.
        assert_eq!(check("open.test", true, None), None);

        // Upstream headers: set from the connection, never from the client.
        let m = site("secure.test");
        let m = m.mtls.as_ref().unwrap();
        let mut up = req(
            "GET",
            "/",
            &[
                ("x-client-verify", "SUCCESS"),
                ("x-client-cert-subject", "CN=admin"),
            ],
        );
        client_cert_headers(&mut up, m, Some(&ok)).unwrap();
        assert_eq!(up.headers["x-client-verify"], "SUCCESS");
        assert_eq!(up.headers["x-client-cert-subject"], "CN=alice");
        assert_eq!(up.headers["x-client-cert-fingerprint"], "00");
        let mut up = req("GET", "/", &[("x-client-cert-subject", "CN=admin")]);
        client_cert_headers(&mut up, m, Some(&missing)).unwrap();
        assert_eq!(up.headers["x-client-verify"], "NONE");
        assert!(!up.headers.contains_key("x-client-cert-subject"));
        let mut up = req("GET", "/", &[]);
        client_cert_headers(&mut up, m, Some(&valid(id("other.test")))).unwrap();
        assert_eq!(up.headers["x-client-verify"], "NONE");

        assert!(!runtime(BASE).mtls);
        std::fs::remove_dir_all(&dir).ok();
    }
}
