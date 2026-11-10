//! The compiled, immutable view of the configuration that each request reads.
//!
//! A reload builds a new `Runtime` and `CertStore` and swaps them in atomically; requests in
//! flight keep the snapshot they started with.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use http::{HeaderName, HeaderValue};

use crate::cache::SiteCache;
use crate::config::{
    AcmeSection, CacheSection, CertConfig, Config, ServerHeader, SiteConfig, UpgradeMode,
    UpstreamRef,
};
use crate::crowdsec;
use crate::files::Files;
use crate::metrics::SiteMetrics;
use crate::plugin::Chain;
use crate::realip::RealIp;
use crate::tls::{CertKey, CertStore, ClientTrust};
use crate::upstream::Pool;

pub struct Site {
    pub id: Arc<str>,
    /// `None` for a site that only serves files.
    pub pool: Option<Arc<Pool>>,
    pub force_https: bool,
    pub hsts: Option<HeaderValue>,
    pub max_body: Option<u64>,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub upgrade: UpgradeMode,
    pub upgrade_idle_timeout: Duration,
    pub real_ip_header: Option<HeaderName>,
    pub compression: bool,
    pub upstream_host: Option<HeaderValue>,
    pub mtls: Option<Mtls>,
    pub plugins: Chain,
    pub files: Option<Arc<Files>>,
    pub cache: Option<SiteCache>,
    pub metrics: SiteMetrics,
}

/// A site's `mtls:` settings.
pub struct Mtls {
    pub trust: Arc<ClientTrust>,
    pub optional: bool,
}

/// A site whose certificate conduit obtains and renews over ACME.
pub struct AcmeSite {
    pub id: Arc<str>,
    /// Lowercase, sorted: the certificate's names.
    pub hosts: Vec<String>,
    /// Private key and chain in one PEM file, replaced atomically.
    pub path: PathBuf,
}

/// What the ACME service works on; rebuilt with every reload.
pub struct AcmePlan {
    pub directory: String,
    pub email: Option<String>,
    /// Account and certificates for this directory.
    pub dir: PathBuf,
    /// Root of `tls.acme.storage`, holding the lock shared by all directories.
    pub storage: PathBuf,
    pub ca_file: Option<String>,
    pub profile: Option<String>,
    pub renew_before: Option<Duration>,
    pub sites: Vec<AcmeSite>,
}

/// 64-bit FNV-1a: a stable name for a directory URL across builds and processes.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Storage directory of one ACME directory URL: its host and a hash of the whole URL, so
/// staging and production never mix accounts or certificates.
pub fn acme_dir(storage: &std::path::Path, url: &str) -> PathBuf {
    let host: String = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    storage.join(format!("{host}-{:08x}", fnv1a(url) as u32))
}

fn acme_plan(a: &AcmeSection) -> Result<AcmePlan, String> {
    let directory = a.directory_url().to_string();
    if !directory.starts_with("https://") {
        return Err(format!(
            "tls.acme.directory {directory:?} must be an https:// URL"
        ));
    }
    let storage = PathBuf::from(&a.storage);
    Ok(AcmePlan {
        dir: acme_dir(&storage, &directory),
        directory,
        email: a.email.clone(),
        storage,
        ca_file: a.ca_file.clone(),
        profile: a.profile.clone(),
        renew_before: a.renew_before.map(|d| d.0),
        sites: Vec::new(),
    })
}

/// Checks a site with `acme: true` and loads the certificate stored for it, if any.
fn acme_site(
    sc: &SiteConfig,
    cfg: &Config,
    plan: Option<&mut AcmePlan>,
    certs: &mut CertStore,
    watched: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(plan) = plan else {
        return Err("acme: true needs a tls.acme section".into());
    };
    if sc.cert.is_some() {
        return Err("cert and acme cannot both be set".into());
    }
    if cfg.listen.http.is_empty() {
        return Err("acme needs listen.http: the CA validates over HTTP on port 80".into());
    }
    let mut hosts = Vec::with_capacity(sc.hosts.len());
    for h in &sc.hosts {
        let h = normalize_host(h)?;
        if h.starts_with("*.") {
            return Err(format!(
                "{h}: ACME here validates over HTTP, which cannot prove a wildcard; \
                 use a certificate file for it"
            ));
        }
        hosts.push(h);
    }
    hosts.sort();
    hosts.dedup();
    if hosts.len() > 100 {
        return Err("an ACME certificate holds at most 100 names; split the site".into());
    }
    let path = plan.dir.join(format!("{}.pem", sc.id));
    watched.push(path.clone());
    if path.exists() {
        let file = path.to_string_lossy().into_owned();
        let stored = CertConfig {
            name: format!("acme:{}", sc.id),
            cert: file.clone(),
            key: file,
        };
        match CertKey::load(&stored).and_then(|ck| certs.add_managed(ck, &hosts)) {
            Ok(missing) if missing.is_empty() => {}
            Ok(missing) => warnings.push(format!(
                "site {:?}: ACME certificate does not cover {} yet",
                sc.id,
                missing.join(", ")
            )),
            Err(e) => warnings.push(format!(
                "site {:?}: stored ACME certificate not used: {e}",
                sc.id
            )),
        }
    }
    plan.sites.push(AcmeSite {
        id: Arc::from(sc.id.as_str()),
        hosts,
        path,
    });
    Ok(())
}

pub struct Router {
    /// Keys come from the config, so a client's Host cannot crowd one bucket; a fast hash is
    /// enough, and it is looked up on every request.
    exact: HashMap<String, Arc<Site>, foldhash::fast::RandomState>,
    /// (".example.com", site), longest suffix first.
    wildcard: Vec<(String, Arc<Site>)>,
}

impl Router {
    pub fn lookup(&self, host: &str) -> Option<&Arc<Site>> {
        self.exact.get(host).or_else(|| {
            self.wildcard
                .iter()
                .find(|(suffix, _)| host.len() > suffix.len() && host.ends_with(suffix.as_str()))
                .map(|(_, s)| s)
        })
    }

    pub fn len(&self) -> usize {
        self.exact.len() + self.wildcard.len()
    }
}

pub struct Runtime {
    pub router: Router,
    pub sites: Vec<Arc<Site>>,
    /// Every pool, named ones and those written inline in a site (`site:<id>`).
    pub pools: BTreeMap<String, Arc<Pool>>,
    pub realip: RealIp,
    pub default_page: Option<Bytes>,
    /// Requests for hosts that match no site.
    pub default_metrics: SiteMetrics,
    pub acme_webroot: Option<PathBuf>,
    /// Sites with ACME certificates; `None` without `tls.acme`.
    pub acme: Option<Arc<AcmePlan>>,
    /// The `crowdsec:` section; the bouncer service pulls decisions while it is set.
    pub crowdsec: Option<Arc<crowdsec::Plan>>,
    /// Whether any site asks for client certificates; without, handshakes skip the lookup.
    pub mtls: bool,
    pub cache: CacheSection,
    pub upstream_idle_timeout: Duration,
    /// `defaults.keepalive_timeout` in whole seconds, at least 1.
    pub client_keepalive_secs: u64,
    /// `defaults.server_header`.
    pub server_header: Option<HeaderValue>,
    /// Files whose modification makes the reloader rebuild everything.
    pub watched: Vec<PathBuf>,
    pub loaded_at: SystemTime,
}

pub struct Built {
    pub runtime: Runtime,
    pub certs: CertStore,
    pub warnings: Vec<String>,
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

fn normalize_host(h: &str) -> Result<String, String> {
    let h = h.trim().trim_end_matches('.').to_ascii_lowercase();
    let body = h.strip_prefix("*.").unwrap_or(&h);
    if body.is_empty()
        || !body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
    {
        return Err(format!("invalid host name {h:?}"));
    }
    Ok(h)
}

fn compile_site(
    sc: &SiteConfig,
    cfg: &Config,
    pools: &mut BTreeMap<String, Arc<Pool>>,
    prev: Option<&Runtime>,
    crowdsec: Option<&Arc<crowdsec::Plan>>,
    trusts: &mut HashMap<String, Arc<ClientTrust>>,
    watched: &mut Vec<PathBuf>,
) -> Result<Site, String> {
    let d = &cfg.defaults;
    if !valid_id(&sc.id) {
        return Err(format!(
            "site id {:?} must be letters, digits, '.', '-' or '_'",
            sc.id
        ));
    }
    // A pool by name, or an inline one built under `name` (keeping its health state across
    // reloads when unchanged).
    let mut upstream = |r: &UpstreamRef, name: String| match r {
        UpstreamRef::Named(n) => pools
            .get(n)
            .cloned()
            .ok_or_else(|| format!("unknown upstream {n:?}")),
        UpstreamRef::Inline(pc) => {
            let old = prev.and_then(|p| p.pools.get(&name));
            let pool = Pool::build(&name, pc, old).map_err(|e| format!("upstream: {e}"))?;
            pools.insert(name, pool.clone());
            Ok(pool)
        }
    };
    let pool = match &sc.upstream {
        None if sc.files.is_none() => {
            return Err("needs an upstream, files, or both".into());
        }
        None if sc.cache.is_some() => return Err("cache needs an upstream".into()),
        None => None,
        Some(r) => Some(upstream(r, format!("site:{}", sc.id))?),
    };
    let files = sc
        .files
        .as_ref()
        .map(|f| Files::compile(f, pool.is_some()).map(Arc::new))
        .transpose()?;
    let cache = sc
        .cache
        .as_ref()
        .map(|c| SiteCache::compile(c).map_err(|e| format!("cache: {e}")))
        .transpose()?;
    let header = |what: &str, v: &Option<String>| {
        v.as_deref()
            .map(|h| HeaderValue::try_from(h).map_err(|_| format!("invalid {what} {h:?}")))
            .transpose()
    };
    let real_ip_header = sc
        .real_ip_header
        .as_deref()
        .map(|h| HeaderName::try_from(h).map_err(|_| format!("invalid real_ip_header {h:?}")))
        .transpose()?;
    // Sites sharing a CA file share one trust, loaded once per build.
    let mtls = match &sc.mtls {
        None => None,
        Some(m) => {
            let trust = match trusts.get(&m.ca) {
                Some(t) => t.clone(),
                None => {
                    let t = Arc::new(ClientTrust::load(&m.ca)?);
                    watched.push(PathBuf::from(&m.ca));
                    trusts.insert(m.ca.clone(), t.clone());
                    t
                }
            };
            Some(Mtls {
                trust,
                optional: m.optional,
            })
        }
    };
    let plugins = Chain::build(&sc.plugins, &sc.id, crowdsec, watched, &mut upstream)?;
    if pool.is_none() && plugins.uses_site_pool() {
        return Err("a proxy rule without its own upstream needs the site's upstream".into());
    }
    let read_timeout = sc.read_timeout.unwrap_or(d.read_timeout).0;
    Ok(Site {
        id: Arc::from(sc.id.as_str()),
        pool,
        force_https: sc.force_https,
        hsts: header("hsts", &sc.hsts)?,
        max_body: sc.max_body_size.unwrap_or(d.max_body_size).0,
        connect_timeout: sc.connect_timeout.unwrap_or(d.connect_timeout).0,
        read_timeout,
        write_timeout: sc.write_timeout.unwrap_or(d.write_timeout).0,
        upgrade: sc.upgrade,
        upgrade_idle_timeout: sc.upgrade_idle_timeout.map_or(read_timeout, |t| t.0),
        real_ip_header,
        compression: sc.compression.unwrap_or(d.compression),
        upstream_host: header("upstream_host", &sc.upstream_host)?,
        mtls,
        plugins,
        metrics: SiteMetrics::new(&sc.id, cache.is_some()),
        files,
        cache,
    })
}

/// Compile the configuration. `prev` is the running runtime on a reload: pools whose
/// definition did not change are taken over with their health state.
pub fn build(
    cfg: &Config,
    config_path: Option<PathBuf>,
    prev: Option<&Runtime>,
) -> Result<Built, String> {
    if cfg.tls.watch_interval.0.is_zero()
        || std::time::Instant::now()
            .checked_add(cfg.tls.watch_interval.0)
            .is_none()
    {
        return Err("tls.watch_interval must be positive and fit the runtime clock".into());
    }
    let mut warnings = Vec::new();
    let mut watched: Vec<PathBuf> = config_path.into_iter().collect();

    let (mut certs, cert_files) =
        CertStore::build(&cfg.tls.certs, cfg.tls.default_cert.as_deref())?;
    watched.extend(cert_files);

    let mut pools = BTreeMap::new();
    for (name, pc) in &cfg.upstreams {
        if !valid_id(name) {
            return Err(format!(
                "upstream name {name:?} must be letters, digits, '.', '-' or '_'"
            ));
        }
        let old = prev.and_then(|p| p.pools.get(name));
        let pool = Pool::build(name, pc, old).map_err(|e| format!("upstream {name:?}: {e}"))?;
        pools.insert(name.clone(), pool);
    }

    let mut acme = cfg.tls.acme.as_ref().map(acme_plan).transpose()?;
    let crowdsec = cfg
        .crowdsec
        .as_ref()
        .map(|c| crowdsec::Plan::build(c, &mut watched).map(Arc::new))
        .transpose()?;
    let mut exact = HashMap::with_hasher(foldhash::fast::RandomState::default());
    let mut wildcard = Vec::new();
    let mut sites = Vec::new();
    let mut ids = HashSet::new();
    let mut trusts = HashMap::new();
    for sc in cfg.sites.iter().filter(|s| s.enabled) {
        if !ids.insert(sc.id.clone()) {
            return Err(format!("duplicate site id {:?}", sc.id));
        }
        let site = Arc::new(
            compile_site(
                sc,
                cfg,
                &mut pools,
                prev,
                crowdsec.as_ref(),
                &mut trusts,
                &mut watched,
            )
            .map_err(|e| format!("site {:?}: {e}", sc.id))?,
        );
        if site.mtls.is_some() && cfg.listen.https.is_empty() {
            warnings.push(format!(
                "site {:?}: mtls without listen.https refuses every request",
                sc.id
            ));
        }
        sites.push(site.clone());
        if sc.hosts.is_empty() {
            return Err(format!("site {:?} has no hosts", sc.id));
        }
        if sc.acme {
            acme_site(
                sc,
                cfg,
                acme.as_mut(),
                &mut certs,
                &mut watched,
                &mut warnings,
            )
            .map_err(|e| format!("site {:?}: {e}", sc.id))?;
        }
        for h in &sc.hosts {
            let h = normalize_host(h).map_err(|e| format!("site {:?}: {e}", sc.id))?;
            if let Some(cert) = &sc.cert {
                certs
                    .add_override(&h, cert)
                    .map_err(|e| format!("site {:?}: {e}", sc.id))?;
            }
            if sc.acme && !certs.covers(&h) {
                warnings.push(format!(
                    "site {:?}: waiting for an ACME certificate for {h}",
                    sc.id
                ));
            } else if !cfg.listen.https.is_empty()
                && (site.force_https || site.hsts.is_some())
                && !certs.covers(&h)
            {
                warnings.push(format!("site {:?}: no certificate covers {h}", sc.id));
            }
            let dup = if let Some(suffix) = h.strip_prefix('*') {
                if wildcard.iter().any(|(s, _): &(String, _)| s == suffix) {
                    true
                } else {
                    wildcard.push((suffix.to_string(), site.clone()));
                    false
                }
            } else {
                exact.insert(h.clone(), site.clone()).is_some()
            };
            if dup {
                return Err(format!("host {h} is listed by more than one site"));
            }
        }
    }
    wildcard.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    for name in cfg.upstreams.keys() {
        let used = sites.iter().any(|s| {
            s.pool
                .iter()
                .chain(s.plugins.pools())
                .any(|p| &*p.name == name.as_str())
        });
        if !used {
            warnings.push(format!("upstream {name:?} is not used by any enabled site"));
        }
    }

    let realip = RealIp::new(
        &cfg.real_ip.header,
        &cfg.real_ip.trusted,
        cfg.real_ip.recursive,
    )?;
    let default_page = match &cfg.default_site.page {
        Some(p) => {
            watched.push(PathBuf::from(p));
            Some(Bytes::from(
                std::fs::read(p).map_err(|e| format!("default_site.page {p}: {e}"))?,
            ))
        }
        None => None,
    };
    let acme_webroot = cfg.tls.acme_webroot.as_ref().map(PathBuf::from);
    if let Some(root) = &acme_webroot
        && !root.is_dir()
    {
        warnings.push(format!(
            "tls.acme_webroot {} is not a directory",
            root.display()
        ));
    }
    if cfg.listen.http.is_empty() && cfg.listen.https.is_empty() {
        return Err("listen: no http or https addresses".into());
    }
    let server_header = match &cfg.defaults.server_header {
        ServerHeader::Off => None,
        ServerHeader::Value(v) => Some(server_header(v)?),
    };

    Ok(Built {
        runtime: Runtime {
            router: Router { exact, wildcard },
            sites,
            pools,
            realip,
            default_page,
            default_metrics: SiteMetrics::new("-", false),
            acme_webroot,
            acme: acme.map(Arc::new),
            crowdsec,
            mtls: !trusts.is_empty(),
            cache: cfg.cache,
            upstream_idle_timeout: cfg.defaults.upstream_idle_timeout.0,
            client_keepalive_secs: cfg
                .defaults
                .keepalive_timeout
                .0
                .as_secs_f64()
                .ceil()
                .max(1.0) as u64,
            server_header,
            watched,
            loaded_at: SystemTime::now(),
        },
        certs,
        warnings,
    })
}

/// A `Server` value: printable ASCII as written, no more than a product name and comments.
fn server_header(v: &str) -> Result<HeaderValue, String> {
    let err = |why: &str| Err(format!("defaults.server_header {v:?} {why}"));
    if v.trim().is_empty() {
        return err("is empty; write off to send no Server header");
    }
    if v.trim() != v || v.len() > 64 || !v.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return err("must be 1 to 64 printable ASCII characters, without surrounding spaces");
    }
    Ok(HeaderValue::from_str(v).expect("printable ASCII"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;

    #[test]
    fn server_header_setting() {
        let rt = |defaults: &str| {
            let cfg = parse(&format!(
                "listen: {{ http: [\"127.0.0.1:0\"] }}\ndefaults: {{ {defaults} }}\n"
            ))
            .unwrap();
            build(&cfg, None, None).map(|b| b.runtime.server_header)
        };
        // On by default; `off` or `false` sends none.
        assert_eq!(rt("").unwrap().unwrap(), "conduit");
        assert_eq!(rt("server_header: off").unwrap(), None);
        assert_eq!(rt("server_header: false").unwrap(), None);
        assert_eq!(rt("server_header: edge").unwrap().unwrap(), "edge");
        assert_eq!(
            rt("server_header: \"conduit (edge)\"").unwrap().unwrap(),
            "conduit (edge)"
        );
        for bad in [
            "\"\"",
            "\" \"",
            "\" conduit\"",
            "\"conduit\\r\\nX-Evil: 1\"",
            "\"conduit\\u00e9\"",
            &format!("{:?}", "x".repeat(65)),
        ] {
            let e = rt(&format!("server_header: {bad}")).unwrap_err();
            assert!(e.contains("defaults.server_header"), "{bad}: {e}");
        }
    }

    #[test]
    fn pools_and_references() {
        let cfg = parse(
            r#"
listen: { http: ["127.0.0.1:0"] }
upstreams:
  app: { backends: ["http://127.0.0.1:3000", "http://127.0.0.1:3001"], balance: least_conn }
  spare: { backends: ["http://127.0.0.1:3002"] }
sites:
  - { id: a, hosts: [a.test], upstream: app }
  - { id: b, hosts: [b.test], upstream: "http://127.0.0.1:4000" }
"#,
        )
        .unwrap();
        let b = build(&cfg, None, None).unwrap();
        let rt = &b.runtime;
        assert_eq!(
            &*rt.router
                .lookup("a.test")
                .unwrap()
                .pool
                .as_ref()
                .unwrap()
                .name,
            "app"
        );
        assert_eq!(
            rt.router
                .lookup("a.test")
                .unwrap()
                .pool
                .as_ref()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            &*rt.router
                .lookup("b.test")
                .unwrap()
                .pool
                .as_ref()
                .unwrap()
                .name,
            "site:b"
        );
        assert_eq!(
            rt.pools.keys().collect::<Vec<_>>(),
            ["app", "site:b", "spare"]
        );
        assert!(
            b.warnings.iter().any(|w| w.contains("\"spare\"")),
            "{:?}",
            b.warnings
        );

        // A reload keeps unchanged pools, including inline ones.
        let again = build(&cfg, None, Some(rt)).unwrap().runtime;
        assert!(Arc::ptr_eq(&rt.pools["app"], &again.pools["app"]));
        assert!(Arc::ptr_eq(&rt.pools["site:b"], &again.pools["site:b"]));

        let bad = parse("listen: { http: [\"127.0.0.1:0\"] }\nsites:\n  - { id: a, hosts: [a], upstream: nope }\n").unwrap();
        let err = build(&bad, None, None).err().unwrap();
        assert!(err.contains("unknown upstream"), "{err}");
    }

    #[test]
    fn proxy_rule_pools() {
        let yaml = |extra: &str| {
            format!(
                r#"
listen: {{ http: ["127.0.0.1:0"] }}
upstreams:
  api: {{ backends: ["http://127.0.0.1:3000"] }}
sites:
  - id: a
    hosts: [a.test]
    plugins:
      - rules:
          - match: {{ path: {{ prefix: /api/ }} }}
            action: {{ proxy: {{ upstream: api, strip_prefix: /api }} }}
          - match: {{ path: {{ prefix: /beta/ }} }}
            action: {{ proxy: {{ upstream: "http://127.0.0.1:4000" }} }}
{extra}"#
            )
        };
        let files = &format!(
            "    files: {{ root: {} }}\n",
            std::env::temp_dir().display()
        );
        // A files-only site may route to pools of its own rules.
        let cfg = parse(&yaml(files)).unwrap();
        let b = build(&cfg, None, None).unwrap();
        let rt = &b.runtime;
        assert_eq!(
            rt.pools.keys().collect::<Vec<_>>(),
            ["api", "site:a:rule1.2"]
        );
        // Used by a rule, so no warning.
        assert!(
            !b.warnings.iter().any(|w| w.contains("\"api\"")),
            "{:?}",
            b.warnings
        );
        let pools: Vec<_> = rt
            .router
            .lookup("a.test")
            .unwrap()
            .plugins
            .pools()
            .iter()
            .map(|p| p.name.to_string())
            .collect();
        assert_eq!(pools, ["api", "site:a:rule1.2"]);
        // Reloads keep a rule's inline pool.
        let again = build(&cfg, None, Some(rt)).unwrap().runtime;
        assert!(Arc::ptr_eq(
            &rt.pools["site:a:rule1.2"],
            &again.pools["site:a:rule1.2"]
        ));

        // Without an upstream of its own, a proxy rule needs the site's.
        let own = format!(
            "{files}    plugins:\n      - rules:\n          - action: {{ proxy: {{ strip_prefix: /x }} }}\n"
        );
        let cfg = parse(&yaml("").replace(
            "    plugins:\n      - rules:\n",
            "    upstream: api\n    plugins:\n      - rules:\n",
        ))
        .unwrap();
        assert!(build(&cfg, None, None).is_ok());
        let bad = parse(&format!(
            "listen: {{ http: [\"127.0.0.1:0\"] }}\nsites:\n  - id: a\n    hosts: [a.test]\n{own}"
        ))
        .unwrap();
        let err = build(&bad, None, None).err().unwrap();
        assert!(err.contains("needs the site's upstream"), "{err}");
        let bad = parse(&yaml(files).replace("upstream: api,", "upstream: nope,")).unwrap();
        let err = build(&bad, None, None).err().unwrap();
        assert!(err.contains("unknown upstream \"nope\""), "{err}");
    }

    #[test]
    fn routing() {
        let cfg = parse(
            r#"
listen: { http: ["127.0.0.1:0"] }
sites:
  - id: a
    hosts: [App.Example.com]
    upstream: http://127.0.0.1:1
  - id: w
    hosts: ["*.example.com"]
    upstream: http://127.0.0.1:2
  - id: off
    enabled: false
    hosts: [off.example.com]
    upstream: http://127.0.0.1:3
"#,
        )
        .unwrap();
        let b = build(&cfg, None, None).unwrap();
        let r = &b.runtime.router;
        assert_eq!(r.lookup("app.example.com").unwrap().id, "a".into());
        assert_eq!(r.lookup("x.example.com").unwrap().id, "w".into());
        assert_eq!(r.lookup("a.b.example.com").unwrap().id, "w".into());
        assert_eq!(r.lookup("off.example.com").unwrap().id, "w".into());
        assert!(r.lookup("example.com").is_none());
        assert!(r.lookup("other.test").is_none());
    }

    #[test]
    fn acme_sites() {
        let dir = std::env::temp_dir().join(format!("conduit-acme-rt-{}", std::process::id()));
        let yaml = |site: &str, extra: &str| {
            format!(
                "listen: {{ http: [\"127.0.0.1:0\"], https: [\"127.0.0.1:0\"] }}\n\
                 tls: {{ acme: {{ storage: {:?}, directory: \"https://ca.test/dir\" }} }}\n{extra}\
                 sites:\n  - {{ id: a, upstream: \"http://127.0.0.1:1\", force_https: true, {site} }}\n",
                dir.to_string_lossy()
            )
        };
        let ok = yaml("hosts: [WWW.a.test, a.test.], acme: true", "");
        let b = build(&parse(&ok).unwrap(), None, None).unwrap();
        let plan = b.runtime.acme.as_ref().unwrap();
        let path = acme_dir(&dir, "https://ca.test/dir").join("a.pem");
        assert_eq!(plan.sites[0].hosts, ["a.test", "www.a.test"]);
        assert_eq!(plan.sites[0].path, path);
        assert!(b.runtime.watched.contains(&path));
        assert!(
            b.warnings
                .iter()
                .any(|w| w.contains("waiting for an ACME certificate")),
            "{:?}",
            b.warnings
        );

        // A stored certificate wins over SNI matching for the site's hosts.
        let (cert, key) = crate::tls::test_certs::self_signed(&["a.test", "www.a.test"]);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [cert, key].concat()).unwrap();
        let b = build(&parse(&ok).unwrap(), None, None).unwrap();
        assert_eq!(
            b.certs.select(Some("www.a.test")).map(|c| c.name.as_str()),
            Some("acme:a")
        );
        assert!(b.warnings.is_empty(), "{:?}", b.warnings);
        // A broken file is reported, never fatal: the service orders a new one.
        std::fs::write(&path, "garbage").unwrap();
        let b = build(&parse(&ok).unwrap(), None, None).unwrap();
        assert!(
            b.warnings.iter().any(|w| w.contains("not used")),
            "{:?}",
            b.warnings
        );

        let err = |site: &str, extra: &str| {
            build(&parse(&yaml(site, extra)).unwrap(), None, None)
                .err()
                .unwrap()
        };
        let e = err("hosts: [\"*.a.test\"], acme: true", "");
        assert!(e.contains("wildcard"), "{e}");
        let e = err("hosts: [a.test], acme: true, cert: x", "");
        assert!(e.contains("cannot both"), "{e}");
        let no_section = "listen: { http: [\"127.0.0.1:0\"] }\nsites:\n  - { id: a, hosts: [a.test], upstream: \"http://127.0.0.1:1\", acme: true }\n";
        let e = build(&parse(no_section).unwrap(), None, None)
            .err()
            .unwrap();
        assert!(e.contains("tls.acme"), "{e}");
        let https_only =
            yaml("hosts: [a.test], acme: true", "").replace("http: [\"127.0.0.1:0\"], ", "");
        let e = build(&parse(&https_only).unwrap(), None, None)
            .err()
            .unwrap();
        assert!(e.contains("listen.http"), "{e}");
        let plain =
            yaml("hosts: [a.test], acme: true", "").replace("https://ca.test", "http://ca.test");
        let e = build(&parse(&plain).unwrap(), None, None).err().unwrap();
        assert!(e.contains("https://"), "{e}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn acme_storage_per_directory() {
        let s = std::path::Path::new("/var/lib/conduit/acme");
        let prod = acme_dir(s, crate::config::LETSENCRYPT);
        let staging = acme_dir(s, crate::config::LETSENCRYPT_STAGING);
        assert!(
            prod.to_string_lossy()
                .starts_with("/var/lib/conduit/acme/acme-v02.api.letsencrypt.org-"),
            "{prod:?}"
        );
        assert_ne!(prod, staging);
        assert_eq!(prod, acme_dir(s, crate::config::LETSENCRYPT));
        let local = acme_dir(s, "https://localhost:14000/dir");
        assert!(
            local.to_string_lossy().contains("/localhost-14000-"),
            "{local:?}"
        );
    }

    #[test]
    fn duplicate_hosts_rejected() {
        let cfg = parse(
            r#"
listen: { http: ["127.0.0.1:0"] }
sites:
  - { id: a, hosts: [x.test], upstream: "http://127.0.0.1:1" }
  - { id: b, hosts: [X.test], upstream: "http://127.0.0.1:2" }
"#,
        )
        .unwrap();
        let err = build(&cfg, None, None).err().unwrap();
        assert!(err.contains("more than one site"), "{err}");
    }
}
