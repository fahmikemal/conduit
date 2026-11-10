//! Upstream pools: backends, load balancing, active health checks and passive ejection.
//!
//! Selection (round robin, random, consistent hashing) and active health state come from
//! pingora-load-balancing. On top of that every backend carries counters shared with the
//! request path: requests in flight (for `least_conn`), consecutive errors seen on live
//! traffic (passive ejection) and its Prometheus handles. A pool whose definition did not
//! change is carried over by a reload together with all of that state.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::FutureExt;
use http::{HeaderName, header};
use log::{info, warn};
use pingora::connectors::TransportConnector;
use pingora::connectors::http::Connector as HttpConnector;
use pingora::http::RequestHeader;
use pingora::lb::health_check::HealthCheck;
use pingora::lb::selection::{BackendIter, BackendSelection, Consistent, Random, RoundRobin};
use pingora::lb::{Backend, Backends, LoadBalancer, discovery};
use pingora::server::ShutdownWatch;
use pingora::services::background::BackgroundService;
use pingora::upstreams::peer::HttpPeer;
use pingora::{Error, ErrorType, Result};
use prometheus::IntCounter;

use crate::config::{Balance, HealthCheckConfig, PoolConfig, ProbeType, UpstreamHttpVersion};
use crate::metrics;
use crate::proxy::Shared;

/// Bound on how many candidates one selection looks at.
const MAX_ITERATIONS: usize = 256;

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Milliseconds on a monotonic clock; never 0, so 0 can mean "not set".
fn now_ms() -> u64 {
    EPOCH.elapsed().as_millis() as u64 + 1
}

/// One address of a pool, with the state the request path updates.
pub struct Target {
    pub addr: SocketAddr,
    pub tls: bool,
    /// SNI and certificate name for TLS backends; empty otherwise.
    pub sni: String,
    pub verify: bool,
    pub http_version: UpstreamHttpVersion,
    pub http2_max_streams: usize,
    pub weight: usize,
    /// `ip:port`, as logged and used as the metrics label.
    pub label: Arc<str>,
    /// Host of the backend URL, sent by health probes.
    host: String,
    in_flight: AtomicUsize,
    consecutive_errors: AtomicU32,
    ejected_until: AtomicU64,
    requests: IntCounter,
    failures: IntCounter,
}

impl Target {
    /// Protocol preferences are part of the pool group: Pingora's reuse hash does not
    /// include ALPN, so strict H2 must not borrow a previously pooled H1 connection.
    pub fn peer(&self) -> HttpPeer {
        let mut peer = HttpPeer::new(self.addr, self.tls, self.sni.clone());
        let (max, min, group) = match self.http_version {
            UpstreamHttpVersion::Http1 => (1, 1, 0),
            UpstreamHttpVersion::Http2 => (2, 2, 2),
            UpstreamHttpVersion::Auto if self.tls => (2, 1, 1),
            UpstreamHttpVersion::Auto => (1, 1, 1),
        };
        peer.group_key = group;
        peer.options.set_http_version(max, min);
        if max == 2 {
            peer.options.max_h2_streams = self.http2_max_streams;
        }
        peer.options.verify_cert = self.verify;
        peer.options.verify_hostname = self.verify;
        peer
    }

    fn ejected(&self, now: u64) -> bool {
        self.ejected_until.load(Relaxed) > now
    }
}

/// A request's use of a backend; counts toward `least_conn` until dropped.
pub struct Lease(Arc<Target>);

impl Lease {
    fn new(t: Arc<Target>) -> Lease {
        t.in_flight.fetch_add(1, Relaxed);
        t.requests.inc();
        Lease(t)
    }

    pub fn target(&self) -> &Target {
        &self.0
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Relaxed);
    }
}

/// What `balance: hash` hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HashKey {
    None,
    ClientIp,
    Uri,
    Path,
    Header(HeaderName),
    Cookie(String),
}

impl HashKey {
    fn parse(s: Option<&str>, balance: Balance) -> Result<HashKey, String> {
        let Some(s) = s else {
            return if balance == Balance::Hash {
                Ok(HashKey::ClientIp)
            } else {
                Ok(HashKey::None)
            };
        };
        if balance != Balance::Hash {
            return Err("hash_key is only used with balance: hash".into());
        }
        Ok(match s.split_once(':') {
            None if s == "client_ip" => HashKey::ClientIp,
            None if s == "uri" => HashKey::Uri,
            None if s == "path" => HashKey::Path,
            Some(("header", h)) => HashKey::Header(
                HeaderName::try_from(h.trim()).map_err(|_| format!("invalid header {h:?}"))?,
            ),
            Some(("cookie", c)) if !c.trim().is_empty() => HashKey::Cookie(c.trim().to_string()),
            _ => {
                return Err(format!(
                    "invalid hash_key {s:?}: use client_ip, uri, path, header:<name> or cookie:<name>"
                ));
            }
        })
    }
}

/// Object-safe view of a `LoadBalancer<S>` for any selection algorithm.
trait Select: Send + Sync {
    fn pick(&self, key: &[u8], accept: &dyn Fn(&Backend, bool) -> bool) -> Option<Backend>;
    fn backends(&self) -> &Backends;
}

impl<S> Select for LoadBalancer<S>
where
    S: BackendSelection + Send + Sync + 'static,
    S::Iter: BackendIter,
{
    fn pick(&self, key: &[u8], accept: &dyn Fn(&Backend, bool) -> bool) -> Option<Backend> {
        self.select_with(key, MAX_ITERATIONS, accept)
    }

    fn backends(&self) -> &Backends {
        LoadBalancer::backends(self)
    }
}

struct Member {
    backend: Backend,
    target: Arc<Target>,
}

/// State of one backend, for `/status` and metrics.
pub struct MemberStatus<'a> {
    pub label: &'a str,
    pub weight: usize,
    pub healthy: bool,
    pub ejected: bool,
    pub in_flight: usize,
    pub http_version: &'static str,
}

pub struct Pool {
    pub name: Arc<str>,
    pub balance: Balance,
    hash_key: HashKey,
    lb: Box<dyn Select>,
    /// Sorted by address.
    members: Vec<Member>,
    pub retries: usize,
    passive_failures: u32,
    passive_eject: Duration,
    probe_interval: Option<Duration>,
    next_probe: AtomicU64,
    probing: AtomicBool,
    rotor: AtomicUsize,
    retries_total: IntCounter,
    /// The resolved definition; equal signatures mean the pool can be reused.
    signature: String,
}

/// A backend URL resolved to one address.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Resolved {
    addr: SocketAddr,
    weight: usize,
    tls: bool,
    host: String,
    sni: String,
    http_version: UpstreamHttpVersion,
}

fn resolve(cfg: &PoolConfig) -> Result<Vec<Resolved>, String> {
    if cfg.backends.is_empty() {
        return Err("no backends".into());
    }
    let mut out = Vec::new();
    for b in &cfg.backends {
        let url = b.url.as_str();
        if b.weight == 0 {
            return Err(format!("backend {url:?}: weight must be at least 1"));
        }
        let (tls, h2c, rest) = if let Some(r) = url.strip_prefix("http://") {
            (false, false, r)
        } else if let Some(r) = url.strip_prefix("https://") {
            (true, false, r)
        } else if let Some(r) = url.strip_prefix("h2c://") {
            (false, true, r)
        } else {
            return Err(format!(
                "backend {url:?} must start with http://, https:// or h2c://"
            ));
        };
        if h2c
            && cfg
                .http_version
                .is_some_and(|v| v != UpstreamHttpVersion::Http2)
        {
            return Err("h2c:// requires http_version: 2 or an unset http_version".into());
        }
        let http_version = if h2c {
            UpstreamHttpVersion::Http2
        } else {
            cfg.http_version.unwrap_or_default()
        };
        let authority = rest.trim_end_matches('/');
        if authority.is_empty() || authority.contains('/') {
            return Err(format!("backend {url:?} must be scheme://host[:port]"));
        }
        let has_port = authority
            .rsplit_once(':')
            .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
            && !authority.ends_with(']');
        let with_port = if has_port {
            authority.to_string()
        } else {
            format!("{authority}:{}", if tls { 443 } else { 80 })
        };
        let host = with_port
            .rsplit_once(':')
            .map_or(authority, |(h, _)| h)
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        // Names are resolved when the configuration is (re)loaded, not per request.
        let addrs: Vec<SocketAddr> = with_port
            .to_socket_addrs()
            .map_err(|e| format!("cannot resolve backend {url:?}: {e}"))?
            .collect();
        if addrs.is_empty() {
            return Err(format!("backend {url:?} resolved to no address"));
        }
        let sni = if tls {
            cfg.sni.clone().unwrap_or_else(|| host.clone())
        } else {
            String::new()
        };
        for addr in addrs {
            if out.iter().any(|r: &Resolved| r.addr == addr) {
                return Err(format!("backend address {addr} is listed twice"));
            }
            out.push(Resolved {
                addr,
                weight: b.weight,
                tls,
                host: host.clone(),
                sni: sni.clone(),
                http_version,
            });
        }
    }
    out.sort();
    Ok(out)
}

impl Pool {
    /// Build a pool, or return `prev` when its definition (after DNS) is unchanged so health
    /// and ejection state carry over.
    pub fn build(
        name: &str,
        cfg: &PoolConfig,
        prev: Option<&Arc<Pool>>,
    ) -> Result<Arc<Pool>, String> {
        if cfg.http2_max_streams == 0 || cfg.http2_max_streams > u32::MAX as usize {
            return Err("http2_max_streams must be between 1 and 4294967295".into());
        }
        let resolved = resolve(cfg)?;
        let hash_key = HashKey::parse(cfg.hash_key.as_deref(), cfg.balance)?;
        if let Some(hc) = &cfg.health_check {
            if hc.rise == 0 || hc.fall == 0 {
                return Err("health_check: rise and fall must be at least 1".into());
            }
            if hc.kind == ProbeType::Http && !hc.path.starts_with('/') {
                return Err(format!(
                    "health_check: path {:?} must start with /",
                    hc.path
                ));
            }
        }
        let signature = format!(
            "{resolved:?}|{:?}|{hash_key:?}|{:?}|{:?}|{}|{}|{}",
            cfg.balance,
            cfg.health_check,
            cfg.passive,
            cfg.retries,
            cfg.verify_tls,
            cfg.http2_max_streams
        );
        if let Some(p) = prev
            && p.signature == signature
        {
            return Ok(p.clone());
        }

        let name: Arc<str> = Arc::from(name);
        let mut members = Vec::with_capacity(resolved.len());
        for r in &resolved {
            let label: Arc<str> = Arc::from(r.addr.to_string());
            let target = Arc::new(Target {
                addr: r.addr,
                tls: r.tls,
                sni: r.sni.clone(),
                verify: cfg.verify_tls,
                http_version: r.http_version,
                http2_max_streams: cfg.http2_max_streams,
                weight: r.weight,
                host: r.host.clone(),
                in_flight: AtomicUsize::new(0),
                consecutive_errors: AtomicU32::new(0),
                ejected_until: AtomicU64::new(0),
                requests: metrics::BACKEND_REQUESTS.with_label_values(&[&*name, &*label]),
                failures: metrics::BACKEND_FAILURES.with_label_values(&[&*name, &*label]),
                label,
            });
            let backend = Backend::new_with_weight(&r.addr.to_string(), r.weight)
                .map_err(|e| format!("backend {}: {e}", r.addr))?;
            members.push(Member { backend, target });
        }

        let set = members.iter().map(|m| m.backend.clone()).collect();
        let mut backends = Backends::new(discovery::Static::new(set));
        if let Some(hc) = &cfg.health_check {
            backends.set_health_check(Box::new(Probe::new(&name, hc, &members)));
        }
        let lb: Box<dyn Select> = match cfg.balance {
            Balance::RoundRobin | Balance::LeastConn => {
                Box::new(load_balancer::<RoundRobin>(backends)?)
            }
            Balance::Random => Box::new(load_balancer::<Random>(backends)?),
            Balance::Hash => Box::new(load_balancer::<Consistent>(backends)?),
        };

        Ok(Arc::new(Pool {
            retries_total: metrics::RETRIES.with_label_values(&[&*name]),
            name,
            balance: cfg.balance,
            hash_key,
            lb,
            members,
            retries: cfg.retries,
            passive_failures: cfg.passive.failures,
            passive_eject: cfg.passive.eject.0,
            probe_interval: cfg.health_check.as_ref().map(|h| h.interval.0),
            next_probe: AtomicU64::new(0),
            probing: AtomicBool::new(false),
            rotor: AtomicUsize::new(0),
            signature,
        }))
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    fn member(&self, b: &Backend) -> Option<&Member> {
        let addr = b.addr.as_inet()?;
        self.members
            .binary_search_by(|m| m.target.addr.cmp(addr))
            .ok()
            .map(|i| &self.members[i])
    }

    /// The key `balance: hash` hashes for this request; empty for other algorithms.
    pub fn key(&self, req: &RequestHeader, client: Option<IpAddr>) -> Vec<u8> {
        match &self.hash_key {
            HashKey::None => Vec::new(),
            HashKey::ClientIp => match client {
                Some(IpAddr::V4(ip)) => ip.octets().to_vec(),
                Some(IpAddr::V6(ip)) => ip.octets().to_vec(),
                None => Vec::new(),
            },
            HashKey::Uri => req.uri.to_string().into_bytes(),
            HashKey::Path => req.uri.path().as_bytes().to_vec(),
            HashKey::Header(h) => req
                .headers
                .get(h)
                .map_or_else(Vec::new, |v| v.as_bytes().to_vec()),
            HashKey::Cookie(name) => req
                .headers
                .get_all(header::COOKIE)
                .iter()
                .flat_map(|v| v.as_bytes().split(|b| *b == b';'))
                .find_map(|kv| {
                    let kv = kv.trim_ascii();
                    let eq = kv.iter().position(|b| *b == b'=')?;
                    (kv[..eq] == *name.as_bytes()).then(|| kv[eq + 1..].to_vec())
                })
                .unwrap_or_default(),
        }
    }

    /// Choose a backend that is healthy, not ejected and not in `exclude`. When none is,
    /// fall back to any backend not in `exclude`: trying one beats failing every request
    /// when the health checks themselves are wrong.
    pub fn select(&self, key: &[u8], exclude: &[SocketAddr]) -> Option<Lease> {
        let now = now_ms();
        let fresh = |t: &Target| !exclude.contains(&t.addr);
        let good = |t: &Target, healthy: bool| healthy && !t.ejected(now) && fresh(t);
        let any = |t: &Target, _healthy: bool| fresh(t);
        let target = self.choose(key, &good).or_else(|| {
            let t = self.choose(key, &any);
            if let Some(t) = &t {
                log::debug!("pool {}: no healthy backend, trying {}", self.name, t.label);
            }
            t
        })?;
        Some(Lease::new(target))
    }

    fn choose(&self, key: &[u8], accept: &dyn Fn(&Target, bool) -> bool) -> Option<Arc<Target>> {
        if self.balance == Balance::LeastConn {
            return self.least_conn(accept);
        }
        // One backend, the common case: Pingora's selection would allocate a "seen" set and
        // try the same backend up to its iteration limit before giving the same answer.
        if let [m] = self.members.as_slice() {
            let healthy = self.lb.backends().ready(&m.backend);
            return accept(&m.target, healthy).then(|| m.target.clone());
        }
        let b = self.lb.pick(key, &|b, healthy| {
            self.member(b).is_some_and(|m| accept(&m.target, healthy))
        })?;
        self.member(&b).map(|m| m.target.clone())
    }

    /// Fewest requests in flight per unit of weight; ties rotate.
    fn least_conn(&self, accept: &dyn Fn(&Target, bool) -> bool) -> Option<Arc<Target>> {
        let n = self.members.len();
        let start = self.rotor.fetch_add(1, Relaxed);
        let mut best: Option<(&Arc<Target>, usize)> = None;
        for i in 0..n {
            let m = &self.members[(start + i) % n];
            if !accept(&m.target, self.lb.backends().ready(&m.backend)) {
                continue;
            }
            let load = m.target.in_flight.load(Relaxed);
            let better = best.is_none_or(|(b, b_load)| load * b.weight < b_load * m.target.weight);
            if better {
                best = Some((&m.target, load));
            }
        }
        best.map(|(t, _)| t.clone())
    }

    pub fn note_retry(&self) {
        self.retries_total.inc();
    }

    /// A connect failure or an upstream error on live traffic.
    pub fn report_failure(&self, t: &Target) {
        t.failures.inc();
        if self.passive_failures == 0 {
            return;
        }
        let n = t.consecutive_errors.fetch_add(1, Relaxed) + 1;
        let now = now_ms();
        if n >= self.passive_failures && !t.ejected(now) {
            t.ejected_until
                .store(now + self.passive_eject.as_millis() as u64, Relaxed);
            t.consecutive_errors.store(0, Relaxed);
            warn!(
                "pool {}: backend {} ejected for {:?} after {n} consecutive errors",
                self.name, t.label, self.passive_eject
            );
        }
    }

    pub fn report_success(&self, t: &Target) {
        if t.consecutive_errors.load(Relaxed) != 0 {
            t.consecutive_errors.store(0, Relaxed);
        }
    }

    pub fn status(&self) -> impl Iterator<Item = MemberStatus<'_>> {
        let now = now_ms();
        self.members.iter().map(move |m| MemberStatus {
            label: &m.target.label,
            weight: m.target.weight,
            healthy: self.lb.backends().ready(&m.backend),
            ejected: m.target.ejected(now),
            in_flight: m.target.in_flight.load(Relaxed),
            http_version: m.target.http_version.as_str(),
        })
    }

    /// Start a round of active checks if one is due and none is running.
    fn probe_if_due(self: &Arc<Self>) {
        let Some(interval) = self.probe_interval else {
            return;
        };
        let now = now_ms();
        if now < self.next_probe.load(Relaxed) || self.probing.swap(true, Relaxed) {
            return;
        }
        self.next_probe
            .store(now + interval.as_millis() as u64, Relaxed);
        let pool = self.clone();
        tokio::spawn(async move {
            pool.lb.backends().run_health_check(true).await;
            pool.probing.store(false, Relaxed);
        });
    }
}

fn load_balancer<S>(backends: Backends) -> Result<LoadBalancer<S>, String>
where
    S: BackendSelection + 'static,
    S::Iter: BackendIter,
{
    let lb = LoadBalancer::<S>::from_backends(backends);
    // Static discovery completes without awaiting anything.
    lb.update()
        .now_or_never()
        .ok_or("backend discovery did not complete")?
        .map_err(|e| format!("backend discovery: {e}"))?;
    Ok(lb)
}

/// The health checks of all pools share these. Every connector owns OpenSSL contexts that
/// hold the system CA store (several hundred KB each, eagerly loaded), and probes close
/// their connections, so a connector per pool would only cost memory.
static PROBE_HTTP: LazyLock<HttpConnector> = LazyLock::new(|| HttpConnector::new(None));
static PROBE_TCP: LazyLock<TransportConnector> = LazyLock::new(|| TransportConnector::new(None));

/// Active health check for one pool, connecting the way proxied requests do (TLS and SNI
/// per backend).
struct Probe {
    pool: Arc<str>,
    kind: ProbeType,
    path: String,
    host: Option<String>,
    expect: Vec<u16>,
    timeout: Duration,
    rise: usize,
    fall: usize,
    /// Sorted by address, as in the pool.
    targets: Vec<Arc<Target>>,
}

impl Probe {
    fn new(pool: &Arc<str>, hc: &HealthCheckConfig, members: &[Member]) -> Probe {
        Probe {
            pool: pool.clone(),
            kind: hc.kind,
            path: hc.path.clone(),
            host: hc.host.clone(),
            expect: hc.expect.clone(),
            timeout: hc.timeout.0,
            rise: hc.rise,
            fall: hc.fall,
            targets: members.iter().map(|m| m.target.clone()).collect(),
        }
    }

    fn target(&self, b: &Backend) -> Option<&Arc<Target>> {
        let addr = b.addr.as_inet()?;
        self.targets
            .binary_search_by(|t| t.addr.cmp(addr))
            .ok()
            .map(|i| &self.targets[i])
    }

    async fn run(&self, t: &Target) -> Result<()> {
        let mut peer = t.peer();
        let o = &mut peer.options;
        o.connection_timeout = Some(self.timeout);
        o.total_connection_timeout = Some(self.timeout);
        o.read_timeout = Some(self.timeout);
        o.write_timeout = Some(self.timeout);
        // A cancelled probe must own the last client reference to its connection. With
        // multiple streams Pingora inserts it into InUsePool before the probe finishes,
        // which could retain a connection when the outer probe timeout drops this future.
        o.max_h2_streams = 1;
        o.verify_cert = t.verify;
        o.verify_hostname = t.verify;
        if self.kind == ProbeType::Tcp {
            return PROBE_TCP.get_stream(&peer).await.map(|_| ());
        }
        let (mut session, _) = PROBE_HTTP.get_http_session(&peer).await?;
        if t.http_version == UpstreamHttpVersion::Http2 && session.as_http2().is_none() {
            return Error::e_explain(ErrorType::ConnectError, "upstream did not negotiate HTTP/2");
        }
        let mut req = RequestHeader::build("GET", self.path.as_bytes(), None)?;
        req.insert_header(header::HOST, self.host.as_deref().unwrap_or(&t.host))?;
        if session.as_http2().is_some() {
            let host = self.host.as_deref().unwrap_or(&t.host);
            let authority = if host.parse::<std::net::Ipv6Addr>().is_ok() {
                format!("[{host}]")
            } else {
                host.to_string()
            };
            req.insert_header(header::HOST, authority.as_str())?;
            let uri = http::Uri::builder()
                .scheme(if t.tls { "https" } else { "http" })
                .authority(authority)
                .path_and_query(self.path.as_str())
                .build()
                .map_err(|e| Error::because(ErrorType::InvalidHTTPHeader, "health check URI", e))?;
            req.set_uri(uri);
        }
        req.insert_header(header::USER_AGENT, "conduit-health-check")?;
        if session.as_http2().is_none() {
            req.insert_header(header::CONNECTION, "close")?;
        }
        session.write_request_header(Box::new(req)).await?;
        session.finish_request_body().await?;
        session.read_response_header().await?;
        let status = session.response_header().map_or(0, |r| r.status.as_u16());
        // Probes inspect only the response header. Cancel any unread stream and expire
        // its connection rather than leaving an active H2 entry in the shared connector.
        PROBE_HTTP
            .release_http_session(session, &peer, Some(Duration::ZERO))
            .await;
        let ok = if self.expect.is_empty() {
            (200..400).contains(&status)
        } else {
            self.expect.contains(&status)
        };
        if ok {
            Ok(())
        } else {
            Error::e_explain(
                ErrorType::CustomCode("unexpected status", status),
                "health check",
            )
        }
    }
}

#[async_trait]
impl HealthCheck for Probe {
    async fn check(&self, b: &Backend) -> Result<()> {
        let Some(t) = self.target(b) else {
            return Error::e_explain(ErrorType::InternalError, "unknown backend");
        };
        match tokio::time::timeout(self.timeout * 2, self.run(t)).await {
            Ok(r) => r,
            Err(_) => Error::e_explain(ErrorType::ReadTimedout, "health check timed out"),
        }
    }

    async fn health_status_change(&self, b: &Backend, healthy: bool) {
        let addr = self
            .target(b)
            .map_or_else(|| b.addr.to_string(), |t| t.label.to_string());
        if healthy {
            info!("pool {}: backend {addr} is healthy again", self.pool);
        } else {
            warn!(
                "pool {}: backend {addr} failed its health checks",
                self.pool
            );
        }
    }

    fn health_threshold(&self, success: bool) -> usize {
        if success { self.rise } else { self.fall }
    }
}

/// Runs the active checks of every pool in the current configuration on their intervals.
pub struct HealthService {
    shared: Arc<Shared>,
}

impl HealthService {
    pub fn new(shared: Arc<Shared>) -> HealthService {
        HealthService { shared }
    }
}

#[async_trait]
impl BackgroundService for HealthService {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tick.tick() => {
                    for pool in self.shared.runtime.load().pools.values() {
                        pool.probe_if_due();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackendConfig, PassiveConfig};

    fn pool_cfg(urls: &[&str], balance: Balance) -> PoolConfig {
        let mut c = PoolConfig::single(String::new());
        c.backends = urls
            .iter()
            .map(|u| BackendConfig {
                url: u.to_string(),
                weight: 1,
            })
            .collect();
        c.balance = balance;
        c
    }

    fn pick(p: &Pool, key: &[u8], exclude: &[SocketAddr]) -> Option<String> {
        p.select(key, exclude).map(|l| l.target().label.to_string())
    }

    #[test]
    fn urls() {
        let mut c = pool_cfg(
            &["http://127.0.0.1:3000", "https://localhost:8443/"],
            Balance::RoundRobin,
        );
        let r = resolve(&c).unwrap();
        assert!(
            r.iter()
                .any(|r| r.addr == "127.0.0.1:3000".parse().unwrap() && !r.tls)
        );
        let tls: Vec<_> = r.iter().filter(|r| r.tls).collect();
        assert!(!tls.is_empty() && tls.iter().all(|r| r.sni == "localhost"));
        c.sni = Some("app.internal".into());
        assert!(
            resolve(&c)
                .unwrap()
                .iter()
                .filter(|r| r.tls)
                .all(|r| r.sni == "app.internal")
        );
        assert_eq!(
            resolve(&pool_cfg(&["http://[::1]"], Balance::RoundRobin)).unwrap()[0]
                .addr
                .port(),
            80
        );
        for bad in ["10.0.0.1:80", "http://10.0.0.1:80/app", "http://"] {
            assert!(
                resolve(&pool_cfg(&[bad], Balance::RoundRobin)).is_err(),
                "{bad}"
            );
        }
        assert!(
            resolve(&pool_cfg(
                &["http://127.0.0.1:1", "http://127.0.0.1:1/"],
                Balance::Random
            ))
            .is_err()
        );
    }

    #[test]
    fn round_robin_spreads_and_skips_excluded() {
        let p = Pool::build(
            "rr",
            &pool_cfg(
                &[
                    "http://127.0.0.1:1",
                    "http://127.0.0.1:2",
                    "http://127.0.0.1:3",
                ],
                Balance::RoundRobin,
            ),
            None,
        )
        .unwrap();
        let mut seen: Vec<String> = (0..6).filter_map(|_| pick(&p, b"", &[])).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3);
        let ex: Vec<SocketAddr> = vec![
            "127.0.0.1:1".parse().unwrap(),
            "127.0.0.1:2".parse().unwrap(),
        ];
        for _ in 0..4 {
            assert_eq!(pick(&p, b"", &ex).as_deref(), Some("127.0.0.1:3"));
        }
        let all: Vec<SocketAddr> = (1..=3)
            .map(|i| format!("127.0.0.1:{i}").parse().unwrap())
            .collect();
        assert!(pick(&p, b"", &all).is_none());
    }

    #[test]
    fn passive_ejection_and_panic_mode() {
        let mut c = pool_cfg(
            &["http://127.0.0.1:1", "http://127.0.0.1:2"],
            Balance::RoundRobin,
        );
        c.passive = PassiveConfig {
            failures: 2,
            eject: crate::units::Dur(Duration::from_secs(60)),
        };
        let p = Pool::build("passive", &c, None).unwrap();
        let bad = p.members[0].target.clone();
        p.report_failure(&bad);
        assert!(!bad.ejected(now_ms()));
        p.report_failure(&bad);
        assert!(bad.ejected(now_ms()));
        for _ in 0..4 {
            assert_eq!(pick(&p, b"", &[]).as_deref(), Some("127.0.0.1:2"));
        }
        // The only other backend already failed this request: the ejected one is tried.
        let ex = ["127.0.0.1:2".parse().unwrap()];
        assert_eq!(pick(&p, b"", &ex).as_deref(), Some("127.0.0.1:1"));
        // A success resets the count before ejection.
        let other = p.members[1].target.clone();
        p.report_failure(&other);
        p.report_success(&other);
        p.report_failure(&other);
        assert!(!other.ejected(now_ms()));
    }

    #[test]
    fn least_conn_prefers_idle_backends() {
        let p = Pool::build(
            "lc",
            &pool_cfg(
                &["http://127.0.0.1:1", "http://127.0.0.1:2"],
                Balance::LeastConn,
            ),
            None,
        )
        .unwrap();
        let busy = p.select(b"", &[]).unwrap();
        let busy_addr = busy.target().label.to_string();
        for _ in 0..5 {
            let l = p.select(b"", &[]).unwrap();
            assert_ne!(l.target().label.to_string(), busy_addr);
        }
        drop(busy);
        let a = p.select(b"", &[]).unwrap();
        let b = p.select(b"", &[]).unwrap();
        assert_ne!(a.target().label, b.target().label);
    }

    #[test]
    fn hashing_is_sticky() {
        let urls: Vec<String> = (1..=5).map(|i| format!("http://127.0.0.1:{i}")).collect();
        let urls: Vec<&str> = urls.iter().map(String::as_str).collect();
        let mut c = pool_cfg(&urls, Balance::Hash);
        c.hash_key = Some("cookie:sid".into());
        let p = Pool::build("hash", &c, None).unwrap();
        let mut req = RequestHeader::build("GET", b"/", None).unwrap();
        req.insert_header("cookie", "a=1; sid=user-42; b=2")
            .unwrap();
        let key = p.key(&req, None);
        assert_eq!(key, b"user-42");
        let first = pick(&p, &key, &[]).unwrap();
        for _ in 0..10 {
            assert_eq!(pick(&p, &key, &[]).unwrap(), first);
        }
        let spread: std::collections::HashSet<String> = (0..50)
            .filter_map(|i| pick(&p, format!("user-{i}").as_bytes(), &[]))
            .collect();
        assert!(spread.len() >= 3, "{spread:?}");
        assert!(HashKey::parse(Some("client_ip"), Balance::RoundRobin).is_err());
        assert!(HashKey::parse(Some("cookie:"), Balance::Hash).is_err());
    }

    #[test]
    fn unchanged_pool_is_reused() {
        let c = pool_cfg(&["http://127.0.0.1:1"], Balance::RoundRobin);
        let a = Pool::build("reuse", &c, None).unwrap();
        let b = Pool::build("reuse", &c, Some(&a)).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        let mut c2 = c.clone();
        c2.retries = 3;
        let d = Pool::build("reuse", &c2, Some(&a)).unwrap();
        assert!(!Arc::ptr_eq(&a, &d));
    }
}
