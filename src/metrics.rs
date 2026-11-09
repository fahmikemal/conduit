//! Prometheus metrics, exposed by the admin listener at `/metrics`.
//!
//! Everything lives in the default registry, next to anything Pingora registers itself.
//! Request-path metrics go through handles resolved once per (re)load ([`SiteMetrics`],
//! the per-backend counters in `upstream`), so a request never hashes label values.
//! Gauges that describe state (backend health, certificate expiry) are filled in at scrape
//! time from the running configuration.

use std::sync::{LazyLock, Once};
use std::time::Duration;

use prometheus::{
    Encoder, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    TextEncoder, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_int_gauge, register_int_gauge_vec,
};

use crate::runtime::Runtime;
use crate::tls::CertStore;

const LATENCY_BUCKETS: &[f64] = &[
    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
    60.0,
];

macro_rules! lazy {
    ($name:ident: $ty:ty = $init:expr) => {
        pub static $name: LazyLock<$ty> =
            LazyLock::new(|| $init.expect(concat!("register ", stringify!($name))));
    };
}

lazy!(REQUESTS: IntCounterVec = register_int_counter_vec!(
    "conduit_requests_total",
    "Responses sent, by site and status class",
    &["site", "code"]
));
lazy!(DURATION: HistogramVec = register_histogram_vec!(
    "conduit_request_duration_seconds",
    "Time from the request header to the end of the response",
    &["site"],
    LATENCY_BUCKETS.to_vec()
));
lazy!(UPSTREAM_HEADER: HistogramVec = register_histogram_vec!(
    "conduit_upstream_header_duration_seconds",
    "Time from choosing a backend to receiving its response header",
    &["site"],
    LATENCY_BUCKETS.to_vec()
));
lazy!(RESPONSE_BYTES: IntCounterVec = register_int_counter_vec!(
    "conduit_response_body_bytes_total",
    "Response body bytes sent to clients",
    &["site"]
));
lazy!(IN_FLIGHT: IntGauge = register_int_gauge!(
    "conduit_requests_in_flight",
    "Requests being processed"
));
lazy!(BACKEND_REQUESTS: IntCounterVec = register_int_counter_vec!(
    "conduit_backend_requests_total",
    "Requests sent to a backend, retries included",
    &["pool", "backend"]
));
lazy!(BACKEND_FAILURES: IntCounterVec = register_int_counter_vec!(
    "conduit_backend_failures_total",
    "Connect failures and upstream errors per backend",
    &["pool", "backend"]
));
lazy!(BACKEND_UP: IntGaugeVec = register_int_gauge_vec!(
    "conduit_backend_up",
    "1 when a backend passes its active health checks (always 1 without checks)",
    &["pool", "backend"]
));
lazy!(BACKEND_EJECTED: IntGaugeVec = register_int_gauge_vec!(
    "conduit_backend_ejected",
    "1 while a backend is skipped after consecutive errors on live traffic",
    &["pool", "backend"]
));
lazy!(BACKEND_IN_FLIGHT: IntGaugeVec = register_int_gauge_vec!(
    "conduit_backend_requests_in_flight",
    "Requests currently using a backend",
    &["pool", "backend"]
));
lazy!(RETRIES: IntCounterVec = register_int_counter_vec!(
    "conduit_upstream_retries_total",
    "Requests sent to another backend after a connect failure",
    &["pool"]
));
lazy!(RELOADS: IntCounterVec = register_int_counter_vec!(
    "conduit_config_reloads_total",
    "Configuration reloads by result",
    &["result"]
));
lazy!(LAST_RELOAD: IntGauge = register_int_gauge!(
    "conduit_config_last_reload_success_timestamp_seconds",
    "When the running configuration was loaded"
));
lazy!(CERT_EXPIRY: IntGaugeVec = register_int_gauge_vec!(
    "conduit_tls_certificate_expiry_timestamp_seconds",
    "notAfter of each configured certificate",
    &["name"]
));
lazy!(ACME_ORDERS: IntCounterVec = register_int_counter_vec!(
    "conduit_acme_orders_total",
    "Certificate orders to the ACME CA by result",
    &["result"]
));
lazy!(CROWDSEC_DECISIONS: IntGauge = register_int_gauge!(
    "conduit_crowdsec_decisions",
    "CrowdSec decisions held (addresses and ranges)"
));
lazy!(CROWDSEC_PULLS: IntCounterVec = register_int_counter_vec!(
    "conduit_crowdsec_pulls_total",
    "Decision pulls from the CrowdSec local API by result",
    &["result"]
));
lazy!(CROWDSEC_LAST_PULL: IntGauge = register_int_gauge!(
    "conduit_crowdsec_last_pull_success_timestamp_seconds",
    "When decisions were last pulled successfully"
));
lazy!(CROWDSEC_BLOCKED: IntCounterVec = register_int_counter_vec!(
    "conduit_crowdsec_blocked_total",
    "Requests refused because of a decision (crowdsec) or an AppSec verdict (appsec)",
    &["by"]
));
lazy!(APPSEC: IntCounterVec = register_int_counter_vec!(
    "conduit_crowdsec_appsec_requests_total",
    "Requests inspected by CrowdSec AppSec by verdict (error: no verdict)",
    &["result"]
));
lazy!(LIMIT_REFUSED: IntCounterVec = register_int_counter_vec!(
    "conduit_limit_refused_total",
    "Requests refused by rate_limit or concurrency, by site",
    &["site", "limit"]
));
lazy!(EXPLOITS: IntCounterVec = register_int_counter_vec!(
    "conduit_block_exploits_total",
    "Requests block_exploits caught, by site, category and action (block, log)",
    &["site", "category", "action"]
));
lazy!(AUTH: IntCounterVec = register_int_counter_vec!(
    "conduit_auth_requests_total",
    "Requests checked by basic_auth or forward_auth, by site and result (allow, deny, error)",
    &["site", "plugin", "result"]
));
lazy!(CACHE: IntCounterVec = register_int_counter_vec!(
    "conduit_cache_requests_total",
    "Requests of sites with cache: by result (hit, stale, revalidated, miss, uncacheable, bypass)",
    &["site", "result"]
));
lazy!(CACHE_BYTES: IntGauge = register_int_gauge!(
    "conduit_cache_size_bytes",
    "Bytes held by the response cache, bodies and headers"
));
lazy!(CACHE_OBJECTS: IntGauge = register_int_gauge!(
    "conduit_cache_objects",
    "Responses held by the response cache"
));
lazy!(CACHE_EVICTED: IntGauge = register_int_gauge!(
    "conduit_cache_evicted_objects",
    "Responses dropped to stay within cache.max_size since start (a running total)"
));
lazy!(ACCESS_LOG_DROPPED: IntCounter = register_int_counter!(
    "conduit_access_log_dropped_total",
    "Access log lines dropped because the writer fell behind"
));
lazy!(UPSTREAM_REUSE_LOST: IntCounter = register_int_counter!(
    "conduit_upstream_reuse_lost_total",
    "Idle upstream connections Pingora could not hand over to a request because its idle \
     watcher on another thread had not let go yet; a new connection was opened instead"
));
lazy!(BUILD_INFO: IntGaugeVec = register_int_gauge_vec!(
    "conduit_build_info",
    "Always 1; the version label tells which build runs",
    &["version"]
));

const CLASSES: [&str; 6] = ["1xx", "2xx", "3xx", "4xx", "5xx", "other"];

/// Handles for one site, looked up once when the configuration is built.
pub struct SiteMetrics {
    codes: [IntCounter; 6],
    duration: Histogram,
    upstream_header: Histogram,
    bytes: IntCounter,
    /// Per `cache::RESULTS`, for sites with `cache:`.
    cache: Option<[IntCounter; 6]>,
}

impl SiteMetrics {
    pub fn new(site: &str, cache: bool) -> SiteMetrics {
        SiteMetrics {
            codes: CLASSES.map(|c| REQUESTS.with_label_values(&[site, c])),
            duration: DURATION.with_label_values(&[site]),
            upstream_header: UPSTREAM_HEADER.with_label_values(&[site]),
            bytes: RESPONSE_BYTES.with_label_values(&[site]),
            cache: cache
                .then(|| crate::cache::RESULTS.map(|r| CACHE.with_label_values(&[site, r]))),
        }
    }

    pub fn cache(&self, result: &str) {
        if let Some(c) = &self.cache {
            c[crate::cache::result_index(result)].inc();
        }
    }

    pub fn response(&self, status: u16, elapsed: Duration, body_bytes: usize) {
        let class = match status {
            100..=599 => usize::from(status / 100 - 1),
            _ => 5,
        };
        self.codes[class].inc();
        self.duration.observe(elapsed.as_secs_f64());
        self.bytes.inc_by(body_bytes as u64);
    }

    pub fn upstream_header(&self, elapsed: Duration) {
        self.upstream_header.observe(elapsed.as_secs_f64());
    }
}

/// Drop the series of a site that a reload removed.
pub fn forget_site(site: &str) {
    for c in CLASSES {
        let _ = REQUESTS.remove_label_values(&[site, c]);
    }
    let _ = DURATION.remove_label_values(&[site]);
    let _ = UPSTREAM_HEADER.remove_label_values(&[site]);
    let _ = RESPONSE_BYTES.remove_label_values(&[site]);
    for limit in ["rate_limit", "concurrency"] {
        let _ = LIMIT_REFUSED.remove_label_values(&[site, limit]);
    }
    for plugin in ["basic_auth", "forward_auth"] {
        for result in ["allow", "deny", "error"] {
            let _ = AUTH.remove_label_values(&[site, plugin, result]);
        }
    }
    for result in crate::cache::RESULTS {
        let _ = CACHE.remove_label_values(&[site, result]);
    }
    for category in crate::exploits::Category::ALL {
        for action in ["block", "log"] {
            let _ = EXPLOITS.remove_label_values(&[site, category.name(), action]);
        }
    }
}

/// Touch every metric so it is exported (with zero values) before its first event.
pub fn init() {
    LazyLock::force(&IN_FLIGHT);
    LazyLock::force(&RETRIES);
    LazyLock::force(&RELOADS);
    LazyLock::force(&LAST_RELOAD);
    LazyLock::force(&ACCESS_LOG_DROPPED);
    for r in ["success", "failure"] {
        ACME_ORDERS.with_label_values(&[r]);
        CROWDSEC_PULLS.with_label_values(&[r]);
    }
    LazyLock::force(&CROWDSEC_DECISIONS);
    LazyLock::force(&CROWDSEC_LAST_PULL);
    for by in ["crowdsec", "appsec"] {
        CROWDSEC_BLOCKED.with_label_values(&[by]);
    }
    for r in ["allow", "block", "error"] {
        APPSEC.with_label_values(&[r]);
    }
    BUILD_INFO
        .with_label_values(&[env!("CARGO_PKG_VERSION")])
        .set(1);
}

static PROCESS: Once = Once::new();

/// The text exposition of every registered metric, with state gauges refreshed first.
pub fn render(rt: &Runtime, certs: &CertStore) -> Vec<u8> {
    // The default registry registers a process collector of its own when first touched,
    // which is before daemonizing; the collector reads the pid it was created with, the
    // parent's. It is replaced on the first scrape, which runs in the final process.
    PROCESS.call_once(|| {
        use prometheus::process_collector::ProcessCollector;
        let _ = prometheus::unregister(Box::new(ProcessCollector::for_self()));
        if let Err(e) = prometheus::register(Box::new(ProcessCollector::for_self())) {
            log::warn!("process metrics unavailable: {e}");
        }
    });

    let c = crate::cache::stats();
    CACHE_BYTES.set(c.bytes as i64);
    CACHE_OBJECTS.set(c.objects as i64);
    CACHE_EVICTED.set(c.evicted as i64);
    BACKEND_UP.reset();
    BACKEND_EJECTED.reset();
    BACKEND_IN_FLIGHT.reset();
    for pool in rt.pools.values() {
        for b in pool.status() {
            let labels = [&*pool.name, b.label];
            BACKEND_UP
                .with_label_values(&labels)
                .set(i64::from(b.healthy));
            BACKEND_EJECTED
                .with_label_values(&labels)
                .set(i64::from(b.ejected));
            BACKEND_IN_FLIGHT
                .with_label_values(&labels)
                .set(b.in_flight as i64);
        }
    }
    CERT_EXPIRY.reset();
    for c in certs.all() {
        CERT_EXPIRY
            .with_label_values(&[c.name.as_str()])
            .set(c.not_after_unix);
    }

    let mut out = Vec::with_capacity(32 * 1024);
    if let Err(e) = TextEncoder::new().encode(&prometheus::gather(), &mut out) {
        log::error!("encoding metrics: {e}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classes() {
        let m = SiteMetrics::new("metrics-test", false);
        m.response(204, Duration::from_millis(3), 10);
        m.response(503, Duration::from_millis(3), 0);
        m.response(0, Duration::from_millis(3), 0);
        let get = |c: &str| REQUESTS.with_label_values(&["metrics-test", c]).get();
        assert_eq!((get("2xx"), get("5xx"), get("other")), (1, 1, 1));
        assert_eq!(
            RESPONSE_BYTES.with_label_values(&["metrics-test"]).get(),
            10
        );
        forget_site("metrics-test");
        assert_eq!(get("2xx"), 0);
    }
}
