//! The YAML configuration file, as written by the operator.
//!
//! Everything here is plain data. `runtime::build` validates it and compiles it into the
//! structures the proxy uses per request.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, IgnoredAny, MapAccess, Visitor};

use crate::units::{Dur, Size};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, deserialize_with = "or_default")]
    pub server: ServerSection,
    #[serde(default, deserialize_with = "or_default")]
    pub listen: ListenSection,
    #[serde(default, deserialize_with = "or_default")]
    pub admin: AdminSection,
    #[serde(default, deserialize_with = "or_default")]
    pub defaults: Defaults,
    #[serde(default, deserialize_with = "or_default")]
    pub access_log: AccessLogSection,
    #[serde(default, deserialize_with = "or_default")]
    pub real_ip: RealIpSection,
    #[serde(default, deserialize_with = "or_default")]
    pub tls: TlsSection,
    #[serde(default, deserialize_with = "or_default")]
    pub default_site: DefaultSite,
    /// CrowdSec decisions (local API) and request inspection (AppSec), for sites with the
    /// `crowdsec` plugin.
    #[serde(default)]
    pub crowdsec: Option<CrowdSecSection>,
    /// The response cache shared by sites with `cache:`.
    #[serde(default, deserialize_with = "or_default")]
    pub cache: CacheSection,
    /// Named backend pools that sites refer to by name.
    #[serde(default, deserialize_with = "or_default")]
    pub upstreams: BTreeMap<String, PoolConfig>,
    #[serde(default, deserialize_with = "or_default")]
    pub sites: Vec<SiteConfig>,
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse(text: &str) -> Result<Config, String> {
    yaml(text)
}

/// Reads YAML 1.2 the way the config file is read: only `true`/`false` are booleans,
/// duplicate keys are errors, and errors name the line and column without quoting the
/// file (it holds secrets, and errors go to the log and `/status`).
pub fn yaml<'de, T: Deserialize<'de>>(text: &'de str) -> Result<T, String> {
    let options = serde_saphyr::options! {
        strict_booleans: true,
        with_snippet: false,
    };
    serde_saphyr::from_str_with_options(text, options).map_err(|e| match e {
        // The crate's own wording points at its Options, which operators cannot set.
        serde_saphyr::Error::DuplicateMappingKey { key, location } => format!(
            "duplicate key {} at line {}, column {}",
            key.as_deref().unwrap_or("(not a plain value)"),
            location.line(),
            location.column()
        ),
        e => e.to_string(),
    })
}

/// A section left empty (everything under `default_site:` commented out) reads as null:
/// take it as the section's defaults, as if it were not written.
fn or_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Reads a map in place of the value the visitor was asked for, keeping the
/// deserializer's own error positions.
fn from_map<'de, T: Deserialize<'de>, A: MapAccess<'de>>(map: A) -> Result<T, A::Error> {
    T::deserialize(MapAccessDeserializer::new(map))
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct ServerSection {
    /// Worker threads shared by every listener.
    pub threads: usize,
    /// How the worker threads share work. false (default): one single-threaded runtime per
    /// thread (thread per core); each connection stays on the thread it was handed to at
    /// accept, chosen at random, so there is no cross-thread scheduling, and no rebalancing
    /// either. true: one Tokio runtime whose threads steal tasks from each other, which
    /// evens out a few heavy long-lived connections at the cost of throughput and latency
    /// for many ordinary ones (see the README's benchmark notes).
    pub work_stealing: bool,
    pub pid_file: String,
    pub upgrade_sock: String,
    /// Where stderr goes once daemonized. Stays on stderr when unset.
    pub error_log: Option<String>,
    /// Idle upstream connections kept per worker thread.
    pub upstream_keepalive_pool_size: usize,
    /// How long a process that is stopping (SIGTERM) or being replaced (SIGQUIT during an
    /// upgrade) keeps finishing in-flight requests. Pingora waits this long unconditionally.
    pub shutdown_grace: Dur,
    /// Extra time for worker runtimes to wind down after the grace period.
    pub shutdown_timeout: Dur,
    /// Let the kernel back the heap with transparent huge pages. Under the common `always`
    /// policy that adds 2 MB per allocator arena, roughly doubling idle memory, for about 1%
    /// more throughput. Off: conduit opts its own process out.
    pub transparent_huge_pages: bool,
    /// Per-connection write buffer on the listeners. A response whose header and body fit
    /// leaves in one write (and one packet) instead of two. A `Content-Length` body is
    /// flushed when complete or when the buffer fills, so at most this much waits while an
    /// upstream stalls mid-body; chunked bodies are flushed chunk by chunk. 0: unbuffered.
    pub write_buffer: Size,
}

impl Default for ServerSection {
    fn default() -> Self {
        ServerSection {
            threads: 2,
            work_stealing: false,
            pid_file: "/run/conduit/conduit.pid".into(),
            upgrade_sock: "/run/conduit/upgrade.sock".into(),
            error_log: None,
            upstream_keepalive_pool_size: 128,
            shutdown_grace: Dur(Duration::from_secs(60)),
            shutdown_timeout: Dur(Duration::from_secs(5)),
            transparent_huge_pages: false,
            write_buffer: Size(Some(16 << 10)),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct ListenSection {
    pub http: Vec<String>,
    pub https: Vec<String>,
}

/// Operator endpoints: `/metrics` (Prometheus), `/healthz`, `/status` (JSON).
#[derive(Debug, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct AdminSection {
    /// e.g. `127.0.0.1:9180`. Off when unset. Never expose it publicly.
    pub listen: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct Defaults {
    pub connect_timeout: Dur,
    pub read_timeout: Dur,
    pub write_timeout: Dur,
    /// How long an idle upstream connection waits in the pool for reuse.
    pub upstream_idle_timeout: Dur,
    /// How long an idle HTTP/1.1 client connection waits for its next request. A client's
    /// own `Keep-Alive: timeout=` is not followed.
    pub keepalive_timeout: Dur,
    pub max_body_size: Size,
    /// gzip text-like responses the upstream left uncompressed.
    pub compression: bool,
    /// `Server` header of every response, sent exactly as written (no version is added). It
    /// replaces the upstream's own. Default `conduit`; `off`: conduit's responses carry none
    /// and the upstream's passes through.
    pub server_header: ServerHeader,
}

/// `defaults.server_header`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerHeader {
    /// `off` (or `false`): no `Server` from conduit.
    Off,
    /// The value as written; `runtime::build` checks it.
    Value(String),
}

impl Default for ServerHeader {
    fn default() -> Self {
        ServerHeader::Value("conduit".into())
    }
}

impl<'de> Deserialize<'de> for ServerHeader {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Read as a string, so the YAML reader hands `false` over as the word, like `off`.
        let v = String::deserialize(d)?;
        Ok(match v.as_str() {
            "off" | "false" => ServerHeader::Off,
            _ => ServerHeader::Value(v),
        })
    }
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            connect_timeout: Dur(Duration::from_secs(10)),
            read_timeout: Dur(Duration::from_secs(90)),
            write_timeout: Dur(Duration::from_secs(90)),
            upstream_idle_timeout: Dur(Duration::from_secs(60)),
            keepalive_timeout: Dur(Duration::from_secs(75)),
            max_body_size: Size(Some(2000 << 20)),
            compression: true,
            server_header: ServerHeader::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct AccessLogSection {
    /// File receiving one JSON object per request. Logging is off when unset.
    pub path: Option<String>,
    /// Lines buffered in memory before new lines are dropped (counted and reported), split
    /// evenly across the worker threads, at least 1024 each.
    pub queue_capacity: usize,
}

impl Default for AccessLogSection {
    fn default() -> Self {
        AccessLogSection {
            path: None,
            queue_capacity: 65536,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct RealIpSection {
    /// Header carrying the client address, read only from trusted peers.
    pub header: String,
    /// Trusted peers: CIDRs, or `cloudflare` for Cloudflare's published ranges.
    pub trusted: Vec<String>,
    /// Walk a comma-separated header from the right, skipping trusted addresses.
    pub recursive: bool,
}

impl Default for RealIpSection {
    fn default() -> Self {
        RealIpSection {
            header: "X-Real-IP".into(),
            trusted: vec![],
            recursive: true,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct TlsSection {
    pub certs: Vec<CertConfig>,
    /// Certificate served when the SNI matches nothing. Without it such handshakes fail.
    pub default_cert: Option<String>,
    /// Directory served at `/.well-known/acme-challenge/` on every host (HTTP-01 webroot).
    pub acme_webroot: Option<String>,
    /// How often certificate, config and file mtimes are checked for changes.
    pub watch_interval: Dur,
    /// Certificates obtained and renewed automatically for sites with `acme: true`.
    pub acme: Option<AcmeSection>,
}

impl Default for TlsSection {
    fn default() -> Self {
        TlsSection {
            certs: vec![],
            default_cert: None,
            acme_webroot: None,
            watch_interval: Dur(Duration::from_secs(60)),
            acme: None,
        }
    }
}

pub const LETSENCRYPT: &str = "https://acme-v02.api.letsencrypt.org/directory";
pub const LETSENCRYPT_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

/// ACME (RFC 8555) with HTTP-01 validation, answered by conduit itself on `listen.http`.
/// Enabling it accepts the CA's terms of service.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct AcmeSection {
    /// Directory URL, or `letsencrypt` / `letsencrypt-staging`.
    pub directory: String,
    /// Contact address for the account (expiry and policy mail from the CA).
    pub email: Option<String>,
    /// Account keys and issued certificates (key and chain in one PEM file per site).
    pub storage: String,
    /// CA certificate (PEM) trusted for the directory's HTTPS instead of the system store,
    /// e.g. a test CA.
    pub ca_file: Option<String>,
    /// Certificate profile offered by the CA, e.g. `tlsserver` or `shortlived`.
    pub profile: Option<String>,
    /// Renew this long before expiry when the CA suggests no renewal window (ARI).
    /// Default: when a third of the lifetime is left. Capped at half the lifetime.
    pub renew_before: Option<Dur>,
}

impl Default for AcmeSection {
    fn default() -> Self {
        AcmeSection {
            directory: "letsencrypt".into(),
            email: None,
            storage: "/var/lib/conduit/acme".into(),
            ca_file: None,
            profile: None,
            renew_before: None,
        }
    }
}

impl AcmeSection {
    pub fn directory_url(&self) -> &str {
        match self.directory.as_str() {
            "letsencrypt" => LETSENCRYPT,
            "letsencrypt-staging" => LETSENCRYPT_STAGING,
            url => url,
        }
    }
}

/// CrowdSec bouncer: decisions pulled from the local API every `update_interval` and held
/// in memory, so checking a client costs no network round trip.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct CrowdSecSection {
    /// Local API (LAPI) base URL.
    pub lapi_url: String,
    /// Bouncer key (`cscli bouncers add conduit`). Give it inline or in a file.
    pub api_key: Option<String>,
    pub api_key_file: Option<String>,
    /// How often new and expired decisions are pulled.
    pub update_interval: Dur,
    /// Only decisions from these origins (`crowdsec`, `cscli`, `CAPI`, `lists`, ...).
    pub origins: Vec<String>,
    /// Only decisions whose scenario contains / does not contain one of these strings.
    pub scenarios_containing: Vec<String>,
    pub scenarios_not_containing: Vec<String>,
    /// CA certificate (PEM) for an `https://` LAPI or AppSec URL, instead of the system store.
    pub ca_file: Option<String>,
    /// HTML returned with 403 to blocked clients. A built-in page by default.
    pub ban_page: Option<String>,
    /// The AppSec component (WAF), for sites with `crowdsec: { appsec: true }`.
    pub appsec: Option<AppSecSection>,
}

impl Default for CrowdSecSection {
    fn default() -> Self {
        CrowdSecSection {
            lapi_url: "http://127.0.0.1:8080".into(),
            api_key: None,
            api_key_file: None,
            update_interval: Dur(Duration::from_secs(10)),
            origins: vec![],
            scenarios_containing: vec![],
            scenarios_not_containing: vec![],
            ca_file: None,
            ban_page: None,
            appsec: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct AppSecSection {
    pub url: String,
    /// The request waits at most this long for the verdict.
    pub timeout: Dur,
    /// When AppSec cannot answer (timeout, error): `allow` the request or `block` it.
    pub failure_action: FailureAction,
    /// Request bodies of known length up to this size are inspected too (at most 64k).
    /// Larger or chunked bodies are not: the request is inspected by its headers alone.
    pub body_limit: Size,
}

impl Default for AppSecSection {
    fn default() -> Self {
        AppSecSection {
            url: "http://127.0.0.1:7422".into(),
            timeout: Dur(Duration::from_millis(200)),
            failure_action: FailureAction::Allow,
            body_limit: Size(Some(64 << 10)),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FailureAction {
    Allow,
    Block,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CertConfig {
    pub name: String,
    /// PEM chain, leaf first.
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields, default)]
pub struct DefaultSite {
    /// HTML file returned with 200 for GET/HEAD on unknown hosts. Otherwise they get 404.
    pub page: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    /// Stable identifier, used in logs and metrics.
    pub id: String,
    /// Exact names, or `*.example.com` for any subdomain.
    pub hosts: Vec<String>,
    /// A pool name from `upstreams:`, a URL (`http://10.0.0.1:3000`), or an inline pool.
    /// Optional for a site that only serves `files`.
    #[serde(default)]
    pub upstream: Option<UpstreamRef>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Certificate name for these hosts, when SNI-based selection would pick another one.
    #[serde(default)]
    pub cert: Option<String>,
    /// Obtain and renew one certificate for all hosts of this site over ACME (`tls.acme`).
    #[serde(default)]
    pub acme: bool,
    #[serde(default)]
    pub force_https: bool,
    /// `Strict-Transport-Security` value added to HTTPS responses.
    #[serde(default)]
    pub hsts: Option<String>,
    #[serde(default)]
    pub max_body_size: Option<Size>,
    #[serde(default)]
    pub connect_timeout: Option<Dur>,
    #[serde(default)]
    pub read_timeout: Option<Dur>,
    #[serde(default)]
    pub write_timeout: Option<Dur>,
    /// Which HTTP/1.1 protocol upgrades (`Upgrade` answered by `101 Switching Protocols`)
    /// reach the upstream.
    #[serde(default)]
    pub upgrade: UpgradeMode,
    /// An upgraded connection is closed after this long with no data in either direction.
    /// Default: `read_timeout`.
    #[serde(default)]
    pub upgrade_idle_timeout: Option<Dur>,
    /// Overrides `real_ip.header` for this site, e.g. `CF-Connecting-IP`.
    #[serde(default)]
    pub real_ip_header: Option<String>,
    #[serde(default)]
    pub compression: Option<bool>,
    /// `Host` sent upstream. By default the client's own `Host` is passed through.
    #[serde(default)]
    pub upstream_host: Option<String>,
    /// Client certificates (mutual TLS) for this site.
    #[serde(default)]
    pub mtls: Option<MtlsConfig>,
    /// Run in order on every request of this site.
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    /// Serve GET and HEAD requests from a directory.
    #[serde(default)]
    pub files: Option<FilesConfig>,
    /// Keep upstream responses in the shared cache (`cache:` section).
    #[serde(default)]
    pub cache: Option<SiteCacheConfig>,
}

/// A site's `upgrade:` setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeMode {
    /// WebSocket handshakes (`Upgrade: websocket`) only, with the hop-by-hop headers
    /// normalized; any other upgrade reaches the upstream as an ordinary request.
    #[default]
    Websocket,
    /// Every upgrade, its handshake headers as the client sent them: also Tailscale's control
    /// protocol (headscale), DERP and other non-WebSocket protocols, like nginx with
    /// `proxy_set_header Upgrade $http_upgrade`.
    Any,
    /// None: the upstream sees an ordinary request and cannot switch protocols.
    Off,
}

/// A static file server: requests map to files under `root`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesConfig {
    pub root: String,
    /// Only these requests are served from files; others go upstream. Default: all.
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
    /// Removed from the request path before it is looked up under `root`.
    #[serde(default)]
    pub strip_prefix: Option<String>,
    /// Files served for a directory, first found wins.
    #[serde(default = "default_index")]
    pub index: Vec<String>,
    /// Paths tried in order, `{path}` being the request path; e.g. `["{path}", "/index.html"]`
    /// for a single-page app. A path ending in `/` is a directory (its index file).
    #[serde(default)]
    pub try_files: Vec<String>,
    /// List directories that have no index file.
    #[serde(default)]
    pub browse: bool,
    /// Serve `<file>.br`, `.zst` or `.gz` next to a file when the client accepts that
    /// encoding, in the order given: any of `br`, `zstd`, `gzip`.
    #[serde(default)]
    pub precompressed: Vec<String>,
    /// File or directory names never served, like dotfiles: exact names or `*.ext`.
    #[serde(default)]
    pub hide: Vec<String>,
    /// A file that does not exist goes to the upstream instead of a 404.
    #[serde(default)]
    pub pass_thru: bool,
}

fn default_index() -> Vec<String> {
    vec!["index.html".into(), "index.txt".into()]
}

/// `cache:` of a site: `true`, or settings.
#[derive(Debug, Clone, Default)]
pub struct SiteCacheConfig {
    /// Only these requests use the cache. Default: every GET and HEAD.
    pub matcher: Option<MatchConfig>,
    /// Freshness for responses that state none (no `Cache-Control` max-age, no `Expires`),
    /// for statuses cacheable by default (200, 301, 404, ...). Off unless set.
    pub default_ttl: Option<Dur>,
    /// How long a stale response may stand in when the upstream fails, unless the response
    /// says otherwise (`stale-if-error`).
    pub stale_if_error: Option<Dur>,
}

impl<'de> Deserialize<'de> for SiteCacheConfig {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Settings {
            #[serde(rename = "match", default)]
            matcher: Option<MatchConfig>,
            #[serde(default)]
            default_ttl: Option<Dur>,
            #[serde(default)]
            stale_if_error: Option<Dur>,
        }
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = SiteCacheConfig;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("true or a map of cache settings")
            }
            fn visit_bool<E: de::Error>(self, on: bool) -> Result<SiteCacheConfig, E> {
                match on {
                    true => Ok(SiteCacheConfig::default()),
                    false => Err(E::custom(
                        "cache: false is the same as leaving it out; remove it",
                    )),
                }
            }
            fn visit_unit<E: de::Error>(self) -> Result<SiteCacheConfig, E> {
                Ok(SiteCacheConfig::default())
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<SiteCacheConfig, A::Error> {
                let s: Settings = from_map(map)?;
                Ok(SiteCacheConfig {
                    matcher: s.matcher,
                    default_ttl: s.default_ttl,
                    stale_if_error: s.stale_if_error,
                })
            }
        }
        d.deserialize_any(V)
    }
}

/// Memory for cached responses, shared by every site with `cache:`.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct CacheSection {
    /// Bodies and headers kept, all sites together; least recently used go first.
    pub max_size: Size,
    /// Larger responses are passed through without being stored.
    pub max_object_size: Size,
    /// How long requests for something being fetched wait for that fetch instead of all
    /// going upstream at once.
    pub lock_timeout: Dur,
}

impl Default for CacheSection {
    fn default() -> Self {
        CacheSection {
            max_size: Size(Some(64 << 20)),
            max_object_size: Size(Some(8 << 20)),
            lock_timeout: Dur(Duration::from_secs(5)),
        }
    }
}

/// Mutual TLS: the site asks clients for a certificate signed by one of `ca`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MtlsConfig {
    /// PEM file with the CA certificates that sign client certificates.
    pub ca: String,
    /// Let clients without a certificate through (upstream sees `X-Client-Verify: NONE`).
    /// A certificate that fails verification is refused either way.
    #[serde(default)]
    pub optional: bool,
}

/// `upstream:` of a site: written as a pool name, a URL, or a pool definition.
#[derive(Debug, Clone)]
pub enum UpstreamRef {
    Named(String),
    Inline(Box<PoolConfig>),
}

impl<'de> Deserialize<'de> for UpstreamRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = UpstreamRef;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a pool name, a URL, or a pool definition")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<UpstreamRef, E> {
                Ok(match s.contains("://") {
                    true => UpstreamRef::Inline(Box::new(PoolConfig::single(s.to_owned()))),
                    false => UpstreamRef::Named(s.to_owned()),
                })
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<UpstreamRef, A::Error> {
                Ok(UpstreamRef::Inline(Box::new(from_map(map)?)))
            }
        }
        d.deserialize_any(V)
    }
}

/// A set of interchangeable backends.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PoolConfig {
    /// `http://`, `https://` or `h2c://host:port`, or `{ url: ..., weight: 3 }`. A host name
    /// becomes one backend per address it resolves to.
    pub backends: Vec<BackendConfig>,
    #[serde(default)]
    pub balance: Balance,
    /// With `balance: hash`: `client_ip`, `uri`, `path`, `header:<name>` or `cookie:<name>`.
    #[serde(default)]
    pub hash_key: Option<String>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    #[serde(default)]
    pub passive: PassiveConfig,
    /// Further backends tried when connecting fails (the request was not sent, so this is
    /// safe for every method).
    #[serde(default = "one")]
    pub retries: usize,
    /// Verify certificates of `https://` backends.
    #[serde(default = "yes")]
    pub verify_tls: bool,
    /// SNI and certificate name for `https://` backends; defaults to the URL's host.
    #[serde(default)]
    pub sni: Option<String>,
    /// Upstream protocol: 1.1 (default), 2 (required), or auto (TLS negotiation).
    #[serde(default)]
    pub http_version: Option<UpstreamHttpVersion>,
    /// Concurrent streams allowed on one HTTP/2 upstream connection.
    #[serde(default = "default_h2_streams")]
    pub http2_max_streams: usize,
}

impl PoolConfig {
    pub fn single(url: String) -> PoolConfig {
        PoolConfig {
            backends: vec![BackendConfig { url, weight: 1 }],
            balance: Balance::default(),
            hash_key: None,
            health_check: None,
            passive: PassiveConfig::default(),
            retries: 1,
            verify_tls: true,
            sni: None,
            http_version: None,
            http2_max_streams: default_h2_streams(),
        }
    }
}

fn default_h2_streams() -> usize {
    128
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpstreamHttpVersion {
    #[default]
    Http1,
    Http2,
    Auto,
}

impl UpstreamHttpVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http1 => "1.1",
            Self::Http2 => "2",
            Self::Auto => "auto",
        }
    }
}

impl<'de> Deserialize<'de> for UpstreamHttpVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = UpstreamHttpVersion;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("upstream HTTP version 1.1, 2, or auto")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                match s {
                    "1.1" => Ok(UpstreamHttpVersion::Http1),
                    "2" => Ok(UpstreamHttpVersion::Http2),
                    "auto" => Ok(UpstreamHttpVersion::Auto),
                    _ => Err(E::custom("http_version must be 1.1, 2, or auto")),
                }
            }

            fn visit_u64<E: de::Error>(self, n: u64) -> Result<Self::Value, E> {
                if n == 2 {
                    Ok(UpstreamHttpVersion::Http2)
                } else {
                    Err(E::custom("http_version must be 1.1, 2, or auto"))
                }
            }

            fn visit_f64<E: de::Error>(self, n: f64) -> Result<Self::Value, E> {
                if n == 1.1 {
                    Ok(UpstreamHttpVersion::Http1)
                } else {
                    Err(E::custom("http_version must be 1.1, 2, or auto"))
                }
            }
        }
        d.deserialize_any(V)
    }
}

fn one() -> usize {
    1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendConfig {
    pub url: String,
    pub weight: usize,
}

impl<'de> Deserialize<'de> for BackendConfig {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Full {
            url: String,
            #[serde(default = "one")]
            weight: usize,
        }
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = BackendConfig;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a URL or { url, weight }")
            }
            fn visit_str<E: de::Error>(self, url: &str) -> Result<BackendConfig, E> {
                Ok(BackendConfig {
                    url: url.to_owned(),
                    weight: 1,
                })
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<BackendConfig, A::Error> {
                let f: Full = from_map(map)?;
                Ok(BackendConfig {
                    url: f.url,
                    weight: f.weight,
                })
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Balance {
    #[default]
    RoundRobin,
    Random,
    /// Fewest requests in flight relative to weight.
    LeastConn,
    /// Consistent (ketama) hashing of `hash_key`: the same key keeps the same backend.
    Hash,
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeType {
    #[default]
    Http,
    Tcp,
}

/// Active checks: each backend is probed every `interval`.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct HealthCheckConfig {
    #[serde(rename = "type")]
    pub kind: ProbeType,
    pub path: String,
    /// `Host` of the probe; defaults to the backend URL's host.
    pub host: Option<String>,
    /// Accepted status codes; any 2xx or 3xx when empty.
    pub expect: Vec<u16>,
    pub interval: Dur,
    pub timeout: Dur,
    /// Consecutive successes that bring a backend back.
    pub rise: usize,
    /// Consecutive failures that take a backend out.
    pub fall: usize,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        HealthCheckConfig {
            kind: ProbeType::Http,
            path: "/".into(),
            host: None,
            expect: vec![],
            interval: Dur(Duration::from_secs(5)),
            timeout: Dur(Duration::from_secs(2)),
            rise: 2,
            fall: 3,
        }
    }
}

/// Taking a backend out after errors seen on real traffic.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct PassiveConfig {
    /// Consecutive connect or upstream errors before ejection; 0 turns it off.
    pub failures: u32,
    /// How long an ejected backend is skipped.
    pub eject: Dur,
}

impl Default for PassiveConfig {
    fn default() -> Self {
        PassiveConfig {
            failures: 3,
            eject: Dur(Duration::from_secs(30)),
        }
    }
}

/// One entry of `plugins:`, written as `{ <name>: <settings> }`.
#[derive(Debug, Clone)]
pub enum PluginConfig {
    /// Conditions on the request and a local response for the first match.
    Rules(Vec<RuleConfig>),
    /// Set or remove request and response headers.
    Headers(HeadersConfig),
    /// Allow or deny clients by address.
    IpFilter(IpFilterConfig),
    /// Refuse clients with a CrowdSec decision; optionally inspect requests with AppSec.
    CrowdSec(CrowdSecPluginConfig),
    /// Requests per time window, per client (or another key).
    RateLimit(RateLimitConfig),
    /// Requests in flight at once, per client (or another key).
    Concurrency(ConcurrencyConfig),
    /// Username and password, checked against bcrypt hashes.
    BasicAuth(BasicAuthConfig),
    /// Ask an auth service (Authelia, Authentik, oauth2-proxy, ...) about each request.
    ForwardAuth(ForwardAuthConfig),
    /// Replace strings in upstream response bodies.
    BodyReplace(BodyReplaceConfig),
    /// Refuse requests that carry common exploit payloads (like an nginx block-exploits include).
    BlockExploits(BlockExploitsConfig),
}

impl<'de> Deserialize<'de> for PluginConfig {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        const ONE_KEY: &str = "each plugin is a map with one key: the plugin name";
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = PluginConfig;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(ONE_KEY)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<PluginConfig, A::Error> {
                use de::Error;
                let Some(name) = m.next_key::<String>()? else {
                    return Err(A::Error::custom(ONE_KEY));
                };
                let plugin = match name.as_str() {
                    "rules" => PluginConfig::Rules(m.next_value()?),
                    "headers" => PluginConfig::Headers(m.next_value()?),
                    "ip_filter" => PluginConfig::IpFilter(m.next_value()?),
                    // `- crowdsec:` with nothing after it reads as null: all defaults.
                    "crowdsec" => {
                        PluginConfig::CrowdSec(m.next_value::<Option<_>>()?.unwrap_or_default())
                    }
                    "rate_limit" => PluginConfig::RateLimit(m.next_value()?),
                    "concurrency" => PluginConfig::Concurrency(m.next_value()?),
                    "basic_auth" => PluginConfig::BasicAuth(m.next_value()?),
                    "forward_auth" => PluginConfig::ForwardAuth(m.next_value()?),
                    "body_replace" => PluginConfig::BodyReplace(m.next_value()?),
                    // `- block_exploits:` alone, like `crowdsec`: all defaults.
                    "block_exploits" => PluginConfig::BlockExploits(
                        m.next_value::<Option<_>>()?.unwrap_or_default(),
                    ),
                    other => {
                        return Err(A::Error::custom(format!(
                            "unknown plugin {other:?}: expected rules, headers, ip_filter, \
                             crowdsec, rate_limit, concurrency, basic_auth, forward_auth, \
                             body_replace or block_exploits"
                        )));
                    }
                };
                match m.next_key::<IgnoredAny>()? {
                    Some(_) => Err(A::Error::custom(ONE_KEY)),
                    None => Ok(plugin),
                }
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields, default)]
pub struct HeadersConfig {
    /// Applied to the request sent upstream.
    pub request: HeaderOps,
    /// Applied to every response of the site, including local ones.
    pub response: HeaderOps,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields, default)]
pub struct HeaderOps {
    /// Replace or add.
    pub set: BTreeMap<String, String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IpFilterConfig {
    /// When given, only these CIDRs pass.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Always refused, even when also allowed.
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default = "forbidden")]
    pub status: u16,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields, default)]
pub struct CrowdSecPluginConfig {
    /// Also send each request to the AppSec component (`crowdsec.appsec`) before proxying.
    pub appsec: bool,
}

/// `rate_limit`: a sliding window of `per`. Only admitted requests use up the allowance, so a
/// client sending faster than the limit still gets the limit through.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RateLimitConfig {
    pub requests: u32,
    #[serde(default = "one_second")]
    pub per: Dur,
    /// `client_ip` (IPv6 by /64), `host`, or `header:<name>` (requests without the header
    /// count by client address).
    #[serde(default = "client_ip")]
    pub key: String,
    /// Only matching requests are counted and limited.
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
    #[serde(default = "too_many_requests")]
    pub status: u16,
}

/// `basic_auth`: bcrypt hashes (`$2a$`, `$2b$`, `$2y$`) as `htpasswd -nB` or
/// `caddy hash-password` print them.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BasicAuthConfig {
    /// Username -> hash.
    #[serde(default)]
    pub users: BTreeMap<String, String>,
    /// An htpasswd file of `user:hash` lines, re-read when it changes.
    #[serde(default)]
    pub htpasswd: Option<String>,
    #[serde(default = "restricted")]
    pub realm: String,
    /// Only matching requests need a password.
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
}

fn restricted() -> String {
    "restricted".into()
}

/// `forward_auth`: a GET to `url` with the client's headers and `X-Forwarded-Method`,
/// `-Uri`, `-Host`, `-Proto` and `-For`. A 2xx answer lets the request through; any other
/// answer goes back to the client as it is (a redirect to a login page, a 401).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForwardAuthConfig {
    /// The auth service's verification endpoint.
    pub url: String,
    /// Headers of a 2xx answer set on the request sent upstream (`Remote-User`, ...). Absent
    /// from the answer, they are removed, so clients cannot set them.
    #[serde(default)]
    pub copy_headers: Vec<String>,
    #[serde(default = "five_seconds")]
    pub timeout: Dur,
    /// CA file for an https:// url; default: the system store.
    #[serde(default)]
    pub ca_file: Option<String>,
    /// Only matching requests are checked.
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
}

fn five_seconds() -> Dur {
    Dur(Duration::from_secs(5))
}

/// `body_replace`: strings replaced in the bodies of upstream responses, as they stream.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BodyReplaceConfig {
    /// Tried at each position in the order written; the first that matches is replaced.
    pub replace: Vec<ReplacePair>,
    /// Response content types (without parameters) whose bodies are changed.
    #[serde(default = "html_only")]
    pub types: Vec<String>,
    /// Only matching requests have their responses changed.
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReplacePair {
    pub from: String,
    pub to: String,
}

fn html_only() -> Vec<String> {
    vec!["text/html".into()]
}

/// `block_exploits`: requests carrying common exploit payloads in the query, the path or
/// the headers are refused (`block`) or only counted and logged (`log`).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct BlockExploitsConfig {
    pub mode: ExploitMode,
    pub status: u16,
    /// Categories checked; default: all but `spam`.
    pub categories: Option<Vec<String>>,
    /// Rule ids switched off (they appear in the log line of each detection).
    pub disable: Vec<String>,
    /// Query parameters left out of the inspection (a search box, say).
    pub skip_params: Vec<String>,
    /// Only matching requests are inspected.
    #[serde(rename = "match")]
    pub matcher: Option<MatchConfig>,
    /// Matching requests skip the categories given (all when none are given).
    pub except: Vec<ExploitException>,
}

impl Default for BlockExploitsConfig {
    fn default() -> Self {
        BlockExploitsConfig {
            mode: ExploitMode::Block,
            status: 403,
            categories: None,
            disable: Vec::new(),
            skip_params: Vec::new(),
            matcher: None,
            except: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExploitMode {
    /// Refuse the request.
    #[default]
    Block,
    /// Let it through; count and log it only.
    Log,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExploitException {
    #[serde(rename = "match")]
    pub matcher: MatchConfig,
    #[serde(default)]
    pub categories: Vec<String>,
}

/// `concurrency`: requests being processed at once.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConcurrencyConfig {
    pub max: u32,
    #[serde(default = "client_ip")]
    pub key: String,
    #[serde(rename = "match", default)]
    pub matcher: Option<MatchConfig>,
    #[serde(default = "too_many_requests")]
    pub status: u16,
}

fn one_second() -> Dur {
    Dur(Duration::from_secs(1))
}

fn client_ip() -> String {
    "client_ip".into()
}

fn too_many_requests() -> u16 {
    429
}

fn forbidden() -> u16 {
    403
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "match", default)]
    pub matcher: MatchConfig,
    pub action: ActionConfig,
}

/// All given conditions must hold.
#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct MatchConfig {
    /// Any of these methods.
    pub method: Vec<String>,
    /// The decoded, normalized path (`%5F` becomes `_`, `//` and `..` are resolved).
    pub path: Option<StrMatch>,
    /// The raw request target as received, path and query.
    pub uri: Option<StrMatch>,
    /// The raw query string.
    pub query: Option<StrMatch>,
    pub scheme: Option<String>,
    pub headers: BTreeMap<String, HeaderMatch>,
}

/// Written as `{ exact: ... }`, `{ prefix: ... }` or `{ regex: ... }`.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(try_from = "StrMatchRaw")]
pub enum StrMatch {
    Exact(String),
    Prefix(String),
    Regex(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrMatchRaw {
    exact: Option<String>,
    prefix: Option<String>,
    regex: Option<String>,
}

impl TryFrom<StrMatchRaw> for StrMatch {
    type Error = String;

    fn try_from(r: StrMatchRaw) -> Result<Self, String> {
        match (r.exact, r.prefix, r.regex) {
            (Some(s), None, None) => Ok(StrMatch::Exact(s)),
            (None, Some(s), None) => Ok(StrMatch::Prefix(s)),
            (None, None, Some(s)) => Ok(StrMatch::Regex(s)),
            _ => Err("give exactly one of exact, prefix, regex".into()),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct HeaderMatch {
    /// `true`: present and non-empty. `false`: missing or empty.
    pub present: Option<bool>,
    pub equals: Option<String>,
    pub regex: Option<String>,
}

/// Written as `{ deny: 403 }`, `{ redirect: {...} }`, `{ respond: {...} }`, `{ file: {...} }`
/// or `{ proxy: {...} }`.
#[derive(Debug, Deserialize, Clone)]
#[serde(try_from = "ActionRaw")]
pub enum ActionConfig {
    /// Answer with this status and no body.
    Deny(u16),
    Redirect(RedirectConfig),
    Respond(RespondConfig),
    File(FileConfig),
    /// Proxy the request, to another upstream or with another path; later plugins still run.
    Proxy(ProxyConfig),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionRaw {
    deny: Option<u16>,
    redirect: Option<RedirectConfig>,
    respond: Option<RespondConfig>,
    file: Option<FileConfig>,
    proxy: Option<ProxyConfig>,
}

impl TryFrom<ActionRaw> for ActionConfig {
    type Error = String;

    fn try_from(r: ActionRaw) -> Result<Self, String> {
        let given = [
            r.deny.is_some(),
            r.redirect.is_some(),
            r.respond.is_some(),
            r.file.is_some(),
            r.proxy.is_some(),
        ];
        if given.iter().filter(|g| **g).count() != 1 {
            return Err("give exactly one of deny, redirect, respond, file, proxy".into());
        }
        Ok(if let Some(s) = r.deny {
            ActionConfig::Deny(s)
        } else if let Some(x) = r.redirect {
            ActionConfig::Redirect(x)
        } else if let Some(x) = r.respond {
            ActionConfig::Respond(x)
        } else if let Some(x) = r.file {
            ActionConfig::File(x)
        } else {
            ActionConfig::Proxy(r.proxy.unwrap_or_default())
        })
    }
}

/// `proxy:` action: where a matching request goes and with which path. The path changes
/// work on the normalized path (as rules see it); the result is sent percent-encoded where
/// needed, followed by the original query string.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ProxyConfig {
    /// A pool name, a URL or a pool definition, as a site's `upstream:`. Default: the site's.
    #[serde(default)]
    pub upstream: Option<UpstreamRef>,
    /// Removed from the start of the path when present (`/api` turns `/api/x` into `/x`).
    #[serde(default)]
    pub strip_prefix: Option<String>,
    /// Put in front of the path, after `strip_prefix`.
    #[serde(default)]
    pub add_prefix: Option<String>,
    /// The first match of `regex` in the path is replaced by `to` (`$1`, `${name}`).
    #[serde(default)]
    pub rewrite: Option<RewriteConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RewriteConfig {
    pub regex: String,
    pub to: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RedirectConfig {
    #[serde(default = "found")]
    pub status: u16,
    /// Target; `{scheme}`, `{host}`, `{path}` and `{request_uri}` are substituted.
    pub to: String,
}

fn found() -> u16 {
    302
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RespondConfig {
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub path: String,
    pub content_type: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_config() {
        let c = parse(
            r#"
listen: { http: ["127.0.0.1:8080"] }
sites:
  - id: "1"
    hosts: [a.example.com]
    upstream: http://127.0.0.1:3000
"#,
        )
        .unwrap();
        assert_eq!(c.sites.len(), 1);
        assert!(c.sites[0].enabled);
        assert_eq!(c.server.threads, 2);
        assert_eq!(c.defaults.max_body_size.0, Some(2000 << 20));
        assert!(c.admin.listen.is_none());
        // Big enough for header and a typical body to share one packet.
        assert_eq!(c.server.write_buffer.0, Some(16 << 10));
        assert!(!c.server.transparent_huge_pages);
    }

    #[test]
    fn plugins_parse() {
        let c = parse(
            r#"
sites:
  - id: "66"
    hosts: [x.example.com]
    upstream: http://10.0.0.1:80
    plugins:
      - ip_filter: { deny: [192.0.2.0/24] }
      - rules:
          - match: { path: { regex: "(?i)^/_(/|$)" } }
            action: { deny: 403 }
          - match:
              method: [GET, HEAD]
              headers: { user-agent: { regex: "(?i)mozilla" } }
            action: { redirect: { to: "https://new.example.com{request_uri}" } }
      - headers: { response: { set: { X-Frame-Options: DENY }, remove: [Server] } }
"#,
        )
        .unwrap();
        let p = &c.sites[0].plugins;
        assert!(matches!(&p[0], PluginConfig::IpFilter(f) if f.status == 403));
        let PluginConfig::Rules(rules) = &p[1] else {
            panic!("{p:?}")
        };
        assert!(matches!(rules[0].action, ActionConfig::Deny(403)));
        assert!(matches!(&rules[1].action, ActionConfig::Redirect(r) if r.status == 302));
        assert!(matches!(&p[2], PluginConfig::Headers(h) if h.response.remove == ["Server"]));

        let err = parse("sites:\n  - { id: a, hosts: [a], upstream: x, plugins: [{ waf: {} }] }\n")
            .unwrap_err();
        assert!(err.contains("unknown plugin"), "{err}");
        let err = parse(
            "sites:\n  - { id: a, hosts: [a], upstream: x, plugins: [{ ip_filter: { alow: [] } }] }\n",
        )
        .unwrap_err();
        assert!(err.contains("alow"), "{err}");
    }

    #[test]
    fn upstream_forms() {
        let c = parse(
            r#"
upstreams:
  app:
    backends: [http://10.0.0.1:3000, { url: "http://10.0.0.2:3000", weight: 3 }]
    balance: hash
    hash_key: client_ip
    health_check: { path: /healthz, interval: 2s }
sites:
  - { id: a, hosts: [a], upstream: app }
  - { id: b, hosts: [b], upstream: "https://10.0.0.3:8443" }
  - id: c
    hosts: [c]
    upstream: { backends: ["https://10.0.0.4"], verify_tls: false }
"#,
        )
        .unwrap();
        let app = &c.upstreams["app"];
        assert_eq!(app.backends[1].weight, 3);
        assert_eq!(app.balance, Balance::Hash);
        let hc = app.health_check.as_ref().unwrap();
        assert_eq!((hc.path.as_str(), hc.fall), ("/healthz", 3));
        assert!(matches!(&c.sites[0].upstream, Some(UpstreamRef::Named(n)) if n == "app"));
        assert!(
            matches!(&c.sites[1].upstream, Some(UpstreamRef::Inline(p)) if p.verify_tls && p.retries == 1)
        );
        assert!(matches!(&c.sites[2].upstream, Some(UpstreamRef::Inline(p)) if !p.verify_tls));
        let err = parse(
            "sites:\n  - { id: a, hosts: [a], upstream: { backends: [], balanse: random } }\n",
        )
        .unwrap_err();
        assert!(err.contains("balanse"), "{err}");
    }

    #[test]
    fn example_config_builds() {
        let mut cfg = parse(include_str!("../conduit.example.yaml")).unwrap();
        assert_eq!(cfg.sites.len(), 17);
        assert_eq!(cfg.upstreams.len(), 3);
        assert_eq!(cfg.cache.max_object_size, Size(Some(8 << 20)));
        // Certificates and served files do not exist here; everything else must compile.
        cfg.tls.certs.clear();
        cfg.tls.acme_webroot = None;
        let cs = cfg.crowdsec.as_mut().unwrap();
        assert!(cs.api_key_file.take().is_some());
        cs.api_key = Some("example".into());
        // Nor do the CA and htpasswd files.
        let hash = bcrypt::hash("x", 4).unwrap();
        let www = std::env::temp_dir().to_string_lossy().into_owned();
        for site in &mut cfg.sites {
            if site.mtls.take().is_some() {
                assert_eq!(site.id, "internal");
            }
            if let Some(f) = &mut site.files {
                f.root = www.clone();
            }
            for p in &mut site.plugins {
                match p {
                    PluginConfig::Rules(rules) => {
                        rules.retain(|r| !matches!(r.action, ActionConfig::File(_)))
                    }
                    PluginConfig::BasicAuth(b) => {
                        assert!(b.htpasswd.take().is_some());
                        b.users.insert("staff".into(), hash.clone());
                    }
                    _ => {}
                }
            }
        }
        let built = crate::runtime::build(&cfg, None, None).unwrap();
        assert_eq!(built.runtime.sites.len(), 17);
        let docs = built.runtime.router.lookup("docs.example.com").unwrap();
        assert!(docs.pool.is_none() && docs.files.is_some());
        let cms = built.runtime.router.lookup("cms.example.com").unwrap();
        assert!(cms.cache.is_some());
        let app = built.runtime.router.lookup("app.example.com").unwrap();
        assert_eq!(
            app.plugins.names(),
            ["block_exploits", "crowdsec", "headers"]
        );
        assert!(app.plugins.appsec());
        let pb = built.runtime.router.lookup("pb.example.com").unwrap();
        assert_eq!(pb.plugins.names(), ["rate_limit", "rate_limit", "rules"]);
        let portal = built.runtime.router.lookup("portal.example.com").unwrap();
        assert_eq!(portal.plugins.names(), ["rules", "body_replace"]);
        let pools: Vec<_> = portal.plugins.pools().iter().map(|p| &*p.name).collect();
        assert_eq!(pools, ["site:portal:rule1.1", "site:portal:rule1.2"]);
        assert!(portal.plugins.uses_site_pool());
        let internal = built.runtime.router.lookup("internal.example.com").unwrap();
        assert_eq!(internal.plugins.names(), ["rate_limit", "basic_auth"]);
        let grafana = built.runtime.router.lookup("grafana.example.com").unwrap();
        assert_eq!(grafana.plugins.names(), ["forward_auth"]);
        assert_eq!(grafana.upgrade, UpgradeMode::Websocket);
        let headscale = built
            .runtime
            .router
            .lookup("headscale.example.com")
            .unwrap();
        assert_eq!(headscale.upgrade, UpgradeMode::Any);
        assert_eq!(headscale.upgrade_idle_timeout, Duration::from_secs(900));
        // the default is the site's read_timeout
        assert_eq!(grafana.upgrade_idle_timeout, grafana.read_timeout);
        // 3 named pools, 11 written in sites and 2 in proxy rules
        assert_eq!(built.runtime.pools.len(), 16);
        let w: Vec<_> = built
            .warnings
            .iter()
            .filter(|w| !w.contains("no certificate covers") && !w.contains("waiting for an ACME"))
            .collect();
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn acme_section() {
        let c = parse(
            "tls: { acme: {} }\nsites:\n  - { id: a, hosts: [a], upstream: x, acme: true }\n",
        )
        .unwrap();
        let a = c.tls.acme.unwrap();
        assert_eq!(a.directory_url(), LETSENCRYPT);
        assert_eq!(a.storage, "/var/lib/conduit/acme");
        assert!(c.sites[0].acme);
        let c = parse("tls: { acme: { directory: letsencrypt-staging, renew_before: 30d } }\n")
            .unwrap();
        let a = c.tls.acme.unwrap();
        assert_eq!(a.directory_url(), LETSENCRYPT_STAGING);
        assert_eq!(a.renew_before.unwrap().0, Duration::from_secs(30 * 86_400));
        let err = parse("tls: { acme: { emial: a@b.c } }\n").unwrap_err();
        assert!(err.contains("emial"), "{err}");
    }

    #[test]
    fn typos_are_rejected() {
        let err = parse("sites:\n  - id: a\n    hosts: [a]\n    upstream: http://1.2.3.4:1\n    force_http: true\n")
            .unwrap_err();
        assert!(err.contains("force_http"), "{err}");
    }

    #[test]
    fn yaml_rules() {
        let site =
            |extra: &str| format!("sites:\n  - {{ id: a, hosts: [a], upstream: x{extra} }}\n");
        // YAML 1.2: `yes` is not a boolean, and `off` stays a word for enums.
        let err = parse(&site(", acme: yes")).unwrap_err();
        assert!(
            err.contains("true/false") && err.contains("line 2"),
            "{err}"
        );
        assert_eq!(
            parse(&site(", upgrade: off")).unwrap().sites[0].upgrade,
            UpgradeMode::Off
        );
        let err = parse(&format!(
            "{}  - {{ id: b, id: c, hosts: [b], upstream: x }}\n",
            site("")
        ))
        .unwrap_err();
        assert_eq!(err, "duplicate key id at line 3, column 14");
        let err = parse(&site(", cache: false")).unwrap_err();
        assert!(err.contains("remove it"), "{err}");
        let err = parse(&site(", plugins: [{ headers: {}, rules: [] }]")).unwrap_err();
        assert!(err.contains("one key"), "{err}");
        // Errors inside a plugin point at the line in the file.
        let err = parse("sites:\n  - id: a\n    hosts: [a]\n    upstream: x\n    plugins:\n      - ip_filter: { alow: [] }\n")
            .unwrap_err();
        assert!(err.contains("alow") && err.contains("line 6"), "{err}");
        // A section with everything commented out is the section's defaults.
        let c = parse("default_site:\n  # page: /x.html\nlisten:\nsites:\n").unwrap();
        assert!(c.default_site.page.is_none() && c.sites.is_empty());
        // Anchors and merge keys.
        let c = parse("sites:\n  - &base { id: a, hosts: [a], upstream: x, upgrade: any }\n  - { <<: *base, id: b, hosts: [b] }\n")
            .unwrap();
        assert_eq!(
            (c.sites[1].id.as_str(), c.sites[1].upgrade),
            ("b", UpgradeMode::Any)
        );
    }
}
