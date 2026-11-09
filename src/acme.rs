//! Certificates from an ACME CA (Let's Encrypt by default) for sites with `acme: true`.
//!
//! A background service issues a certificate when a site has none or its host names changed,
//! and renews it inside the window the CA suggests (ARI, RFC 9773), or when a third of its
//! lifetime is left if the CA suggests none. The proxy answers HTTP-01 challenges from memory
//! on every host. Key and chain go to one PEM file per site, replaced atomically; the reloader
//! then swaps the certificate in, so connections are never dropped.
//!
//! Requests to the CA go through Pingora's HTTP connector and OpenSSL like everything else;
//! instant-acme only speaks the protocol.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use http_body::Body as _;
use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, BodyWrapper, BytesResponse,
    CertificateIdentifier, ChallengeType, HttpClient, Identifier, NewAccount, NewOrder,
    OrderStatus, RetryPolicy,
};
use log::{error, info, warn};
use openssl::asn1::Asn1Time;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::stack::Stack;
use openssl::x509::extension::SubjectAlternativeName;
use openssl::x509::{X509, X509NameBuilder, X509ReqBuilder};
use pingora::server::ShutdownWatch;
use pingora::services::background::BackgroundService;
use rustls_pki_types::Der;

use crate::httpc;
use crate::metrics;
use crate::proxy::Shared;
use crate::runtime::{AcmePlan, AcmeSite};

/// HTTP-01 answers of orders in progress: token -> key authorization.
static CHALLENGES: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

/// The key authorization for `/.well-known/acme-challenge/<token>`, while an order of this
/// process waits for it.
pub fn challenge_answer(token: &[u8]) -> Option<String> {
    let token = std::str::from_utf8(token).ok()?;
    CHALLENGES.lock().unwrap().get(token).cloned()
}

/// Tokens published for one order; withdrawn when the order ends, however it ends.
#[derive(Default)]
pub(crate) struct Published(Vec<String>);

impl Published {
    pub(crate) fn add(&mut self, token: &str, answer: &str) {
        CHALLENGES
            .lock()
            .unwrap()
            .insert(token.to_string(), answer.to_string());
        self.0.push(token.to_string());
    }
}

impl Drop for Published {
    fn drop(&mut self) {
        let mut map = CHALLENGES.lock().unwrap();
        for t in &self.0 {
            map.remove(t);
        }
    }
}

/// Per-site state for `/status`.
#[derive(Clone, Default)]
pub struct SiteStatus {
    pub not_after: Option<i64>,
    /// When the current certificate is due for renewal.
    pub renew_at: Option<i64>,
    /// Whether `renew_at` comes from the CA (ARI) rather than the lifetime.
    pub renew_window_from_ca: bool,
    pub last_error: Option<String>,
    /// Next try after a failure.
    pub retry_at: Option<i64>,
}

#[derive(Default)]
pub struct Status {
    pub sites: Mutex<BTreeMap<String, SiteStatus>>,
}

fn unix(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn at(unix: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(unix.max(0) as u64)
}

// ---------------------------------------------------------------------------------------
// HTTP transport over Pingora

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// ACME responses are small JSON documents and PEM chains.
const MAX_RESPONSE: usize = 1 << 20;

/// instant-acme's view of conduit's HTTP client; clones share the connector.
#[derive(Clone)]
struct Http(Arc<httpc::Client>);

impl Http {
    fn new(ca_file: Option<&str>) -> Http {
        Http(Arc::new(httpc::Client::new(
            ca_file,
            HTTP_TIMEOUT,
            MAX_RESPONSE,
            0,
        )))
    }

    async fn send(&self, req: Request<BodyWrapper<Bytes>>) -> pingora::Result<BytesResponse> {
        let (parts, mut wrapped) = req.into_parts();
        let mut body = Vec::new();
        while let Some(frame) =
            std::future::poll_fn(|cx| Pin::new(&mut wrapped).poll_frame(cx)).await
        {
            let Ok(frame) = frame;
            if let Ok(data) = frame.into_data() {
                body.extend_from_slice(&data);
            }
        }
        let headers: Vec<_> = parts
            .headers
            .iter()
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();
        // One request per connection: ACME traffic is a handful of requests a day.
        let reply = self
            .0
            .send(parts.method, &parts.uri, &headers, Bytes::from(body), false)
            .await?;
        let mut resp = Response::new(());
        *resp.status_mut() = reply.status;
        *resp.headers_mut() = reply.headers;
        let (parts, ()) = resp.into_parts();
        Ok(BytesResponse {
            parts,
            body: Box::new(reply.body),
        })
    }
}

impl HttpClient for Http {
    fn request(
        &self,
        req: Request<BodyWrapper<Bytes>>,
    ) -> Pin<Box<dyn Future<Output = Result<BytesResponse, instant_acme::Error>> + Send>> {
        let t = self.clone();
        Box::pin(async move { t.send(req).await.map_err(|e| instant_acme::Error::Other(e)) })
    }
}

// ---------------------------------------------------------------------------------------
// Storage

/// Write a private file atomically: a temporary file with mode 0600, then a rename.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path)?;
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Exclusive lock on the storage, so two processes (old and new during a graceful upgrade)
/// never order the same certificate. Released when dropped.
struct StorageLock(#[allow(dead_code)] std::fs::File);

fn try_lock(storage: &Path) -> std::io::Result<Option<StorageLock>> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(storage)?;
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(storage.join("lock"))?;
    // SAFETY: flock on a descriptor this function owns.
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r == 0 {
        return Ok(Some(StorageLock(f)));
    }
    let e = std::io::Error::last_os_error();
    if e.kind() == std::io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(e)
    }
}

/// What matters about a stored certificate.
struct Stored {
    /// SAN DNS names, lowercase and sorted.
    names: Vec<String>,
    not_before: i64,
    not_after: i64,
    /// For ARI; `None` without an Authority Key Identifier.
    id: Option<CertificateIdentifier<'static>>,
}

fn asn1_unix(t: &openssl::asn1::Asn1TimeRef) -> Option<i64> {
    let d = Asn1Time::from_unix(0).ok()?.diff(t).ok()?;
    Some(i64::from(d.days) * 86_400 + i64::from(d.secs))
}

/// ARI identifies a certificate by its AKI and the DER content of its serial number.
fn cert_id(leaf: &X509) -> Option<CertificateIdentifier<'static>> {
    let aki = leaf.authority_key_id()?.as_slice().to_vec();
    let mut serial = leaf.serial_number().to_bn().ok()?.to_vec();
    if serial.first().is_none_or(|b| b & 0x80 != 0) {
        serial.insert(0, 0);
    }
    Some(CertificateIdentifier::new(Der::from_slice(&aki), Der::from_slice(&serial)).into_owned())
}

fn inspect(pem: &[u8]) -> Option<Stored> {
    let leaf = X509::from_pem(pem).ok()?;
    let mut names: Vec<String> = leaf
        .subject_alt_names()?
        .iter()
        .filter_map(|n| n.dnsname().map(str::to_ascii_lowercase))
        .collect();
    names.sort();
    names.dedup();
    Some(Stored {
        names,
        not_before: asn1_unix(leaf.not_before())?,
        not_after: asn1_unix(leaf.not_after())?,
        id: cert_id(&leaf),
    })
}

/// Renewal time without the CA's advice: `renew_before` ahead of expiry, by default a third
/// of the lifetime, never more than half of it (so a fresh certificate is never due).
fn lifetime_renewal(c: &Stored, renew_before: Option<Duration>) -> i64 {
    let life = (c.not_after - c.not_before).max(2);
    let before = renew_before.map_or(life / 3, |d| (d.as_secs() as i64).min(life / 2));
    c.not_after - before
}

fn random_unit() -> f64 {
    let mut b = [0u8; 8];
    if openssl::rand::rand_bytes(&mut b).is_err() {
        return 0.5;
    }
    (u64::from_le_bytes(b) >> 11) as f64 / (1u64 << 53) as f64
}

// ---------------------------------------------------------------------------------------
// Orders

fn new_key() -> Result<PKey<Private>, String> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(|e| e.to_string())?;
    let ec = EcKey::generate(&group).map_err(|e| e.to_string())?;
    PKey::from_ec_key(ec).map_err(|e| e.to_string())
}

fn csr(key: &PKey<Private>, hosts: &[String]) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    let mut b = X509ReqBuilder::new()?;
    if let Some(first) = hosts.first().filter(|h| h.len() <= 64) {
        let mut name = X509NameBuilder::new()?;
        name.append_entry_by_nid(Nid::COMMONNAME, first)?;
        b.set_subject_name(&name.build())?;
    }
    b.set_pubkey(key)?;
    let mut san = SubjectAlternativeName::new();
    for h in hosts {
        san.dns(h);
    }
    let mut exts = Stack::new()?;
    exts.push(san.build(&b.x509v3_context(None))?)?;
    b.add_extensions(&exts)?;
    b.sign(key, MessageDigest::sha256())?;
    b.build().to_der()
}

/// Key and chain in one PEM file, after checking that they belong together and cover
/// exactly the requested names.
fn bundle(chain: &str, key: &PKey<Private>, hosts: &[String]) -> Result<Vec<u8>, String> {
    let certs = X509::stack_from_pem(chain.as_bytes()).map_err(|e| format!("chain: {e}"))?;
    let leaf = certs.first().ok_or("the CA sent no certificate")?;
    let public = leaf.public_key().map_err(|e| e.to_string())?;
    if !public.public_eq(key) {
        return Err("the certificate is not for the key we sent".into());
    }
    let stored = inspect(chain.as_bytes()).ok_or("unreadable certificate")?;
    if stored.names != hosts {
        return Err(format!(
            "the certificate names {:?}, not {hosts:?}",
            stored.names
        ));
    }
    let mut out = chain.trim_end().as_bytes().to_vec();
    out.push(b'\n');
    out.extend(key.private_key_to_pem_pkcs8().map_err(|e| e.to_string())?);
    Ok(out)
}

fn acme_err(what: &str) -> impl Fn(instant_acme::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

/// Detail of the first failed challenge of an invalid order.
async fn failure_detail(order: &mut instant_acme::Order) -> String {
    let mut auths = order.authorizations();
    while let Some(Ok(mut a)) = auths.next().await {
        let host = a.identifier().to_string();
        let Ok(state) = a.refresh().await else {
            continue;
        };
        if state.status == AuthorizationStatus::Valid {
            continue;
        }
        if let Some(p) = state.challenges.iter().find_map(|c| c.error.as_ref()) {
            return format!(
                "{host}: {}",
                p.detail
                    .as_deref()
                    .or(p.r#type.as_deref())
                    .unwrap_or("invalid")
            );
        }
        return format!("{host}: authorization {:?}", state.status);
    }
    "order invalid".into()
}

/// One order from creation to certificate: the PEM file to store.
async fn order(
    account: &Account,
    site: &AcmeSite,
    profile: Option<&str>,
    replaces: Option<CertificateIdentifier<'static>>,
) -> Result<Vec<u8>, String> {
    let ids: Vec<Identifier> = site
        .hosts
        .iter()
        .map(|h| Identifier::Dns(h.clone()))
        .collect();
    let mut new = NewOrder::new(&ids);
    if let Some(p) = profile {
        new = new.profile(p);
    }
    let mut order = match replaces {
        Some(r) => match account.new_order(&new.replaces(r)).await {
            Ok(o) => o,
            // The CA may refuse the link (already replaced, unknown); a plain order still works.
            Err(e) => {
                warn!(
                    "ACME site {}: order replacing the old certificate refused ({e}); ordering anew",
                    site.id
                );
                let mut new = NewOrder::new(&ids);
                if let Some(p) = profile {
                    new = new.profile(p);
                }
                account
                    .new_order(&new)
                    .await
                    .map_err(acme_err("new order"))?
            }
        },
        None => account
            .new_order(&new)
            .await
            .map_err(acme_err("new order"))?,
    };

    let mut published = Published::default();
    let mut auths = order.authorizations();
    while let Some(a) = auths.next().await {
        let mut a = a.map_err(acme_err("authorization"))?;
        match a.status {
            AuthorizationStatus::Valid => continue,
            AuthorizationStatus::Pending => {}
            other => return Err(format!("authorization for {} is {other:?}", a.identifier())),
        }
        let host = a.identifier().to_string();
        let mut ch = a
            .challenge(ChallengeType::Http01)
            .ok_or_else(|| format!("the CA offers no http-01 challenge for {host}"))?;
        published.add(&ch.token, ch.key_authorization().as_str());
        ch.set_ready().await.map_err(acme_err("challenge"))?;
    }
    let polling = RetryPolicy::new()
        .initial_delay(Duration::from_millis(500))
        .timeout(Duration::from_secs(120));
    let status = match order.poll_ready(&polling).await {
        Ok(s) => s,
        Err(instant_acme::Error::Api(_)) => OrderStatus::Invalid,
        Err(e) => return Err(format!("validation: {e}")),
    };
    if status != OrderStatus::Ready {
        return Err(format!(
            "validation failed: {}",
            failure_detail(&mut order).await
        ));
    }
    drop(published);

    let key = new_key()?;
    let der = csr(&key, &site.hosts).map_err(|e| format!("CSR: {e}"))?;
    order
        .finalize_csr(&der)
        .await
        .map_err(acme_err("finalize"))?;
    let chain = order
        .poll_certificate(&polling)
        .await
        .map_err(acme_err("certificate"))?;
    bundle(&chain, &key, &site.hosts)
}

// ---------------------------------------------------------------------------------------
// The service

/// Backoff after a failed order: 5 minutes, doubling, at most 6 hours. Let's Encrypt allows
/// 5 failed validations per host name and hour.
fn backoff(failures: u32) -> Duration {
    let mins = 5u64.saturating_mul(1 << failures.saturating_sub(1).min(7));
    Duration::from_secs((mins * 60).min(6 * 3600))
}

/// The CA's suggested renewal time for one certificate, fetched again after `refresh`.
struct Ari {
    id: String,
    renew_at: i64,
    refresh: SystemTime,
}

#[derive(Default)]
struct SiteState {
    failures: u32,
    retry_at: Option<SystemTime>,
    ari: Option<Ari>,
}

struct Connected {
    /// Directory URL and CA file the account was set up with.
    key: (String, Option<String>),
    account: Account,
    ari: bool,
}

#[derive(Default)]
struct Manager {
    account: Option<Connected>,
    sites: HashMap<Arc<str>, SiteState>,
}

enum Due {
    /// Order now, linking to the certificate it replaces when the CA's window was used.
    Now(&'static str, Option<CertificateIdentifier<'static>>),
    At(SystemTime),
}

impl Manager {
    async fn account(&mut self, plan: &AcmePlan) -> Result<&Connected, String> {
        let key = (plan.directory.clone(), plan.ca_file.clone());
        if self.account.as_ref().is_some_and(|c| c.key == key) {
            return Ok(self.account.as_ref().unwrap());
        }
        let http = Http::new(plan.ca_file.as_deref());
        let path = plan.dir.join("account.json");
        let account = match std::fs::read(&path) {
            Ok(text) => {
                let creds: AccountCredentials = serde_json::from_slice(&text)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Account::builder_with_http(Box::new(http))
                    .from_credentials(creds)
                    .await
                    .map_err(acme_err("ACME directory"))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let contact = plan.email.as_ref().map(|e| format!("mailto:{e}"));
                let contacts: Vec<&str> = contact.iter().map(String::as_str).collect();
                let (account, creds) = Account::builder_with_http(Box::new(http))
                    .create(
                        &NewAccount {
                            contact: &contacts,
                            terms_of_service_agreed: true,
                            only_return_existing: false,
                        },
                        plan.directory.clone(),
                        None,
                    )
                    .await
                    .map_err(acme_err("new account"))?;
                let json = serde_json::to_vec(&creds).map_err(|e| e.to_string())?;
                write_private(&path, &json).map_err(|e| format!("{}: {e}", path.display()))?;
                info!("ACME account created at {}", plan.directory);
                account
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        self.account = Some(Connected {
            key,
            account,
            ari: true,
        });
        Ok(self.account.as_ref().unwrap())
    }

    /// When the stored certificate of `site` needs replacing.
    async fn due(&mut self, plan: &AcmePlan, site: &AcmeSite, status: &mut SiteStatus) -> Due {
        let Some(c) = std::fs::read(&site.path).ok().and_then(|p| inspect(&p)) else {
            return Due::Now("no certificate yet", None);
        };
        status.not_after = Some(c.not_after);
        if c.names != site.hosts {
            return Due::Now("host names changed", None);
        }
        let now = SystemTime::now();
        let fallback = lifetime_renewal(&c, plan.renew_before);
        let mut renew_at = fallback;
        let mut from_ca = None;
        if let Some(id) = &c.id {
            let key = id.to_string();
            let cached = self.sites.get(&site.id).and_then(|s| s.ari.as_ref());
            if let Some(a) = cached.filter(|a| a.id == key && now < a.refresh) {
                renew_at = a.renew_at;
                from_ca = Some(id.clone());
            } else if let Ok(conn) = self.account(plan).await
                && conn.ari
            {
                match conn.account.renewal_info(id).await {
                    Ok((info, retry)) => {
                        let start = info.suggested_window.start.unix_timestamp();
                        let end = info.suggested_window.end.unix_timestamp().max(start);
                        let t = start + ((end - start) as f64 * random_unit()) as i64;
                        let refresh = now
                            + retry.clamp(Duration::from_secs(3600), Duration::from_secs(86_400));
                        self.sites.entry(site.id.clone()).or_default().ari = Some(Ari {
                            id: key,
                            renew_at: t,
                            refresh,
                        });
                        renew_at = t;
                        from_ca = Some(id.clone());
                    }
                    Err(instant_acme::Error::Unsupported(_)) => {
                        if let Some(c) = self.account.as_mut() {
                            c.ari = false;
                        }
                    }
                    // Not issued by this CA, or the CA is down: the lifetime decides.
                    Err(e) => {
                        info!("ACME site {}: no renewal window from the CA ({e})", site.id);
                        self.sites.entry(site.id.clone()).or_default().ari = Some(Ari {
                            id: key,
                            renew_at: fallback,
                            refresh: now + Duration::from_secs(6 * 3600),
                        });
                    }
                }
            }
        }
        status.renew_at = Some(renew_at);
        status.renew_window_from_ca = from_ca.is_some();
        if unix(now) >= renew_at {
            Due::Now("renewal due", from_ca)
        } else {
            Due::At(at(renew_at))
        }
    }

    /// Check every site, order what is due; returns how long to sleep.
    async fn tick(&mut self, plan: &AcmePlan, shared: &Shared) -> Duration {
        self.sites
            .retain(|id, _| plan.sites.iter().any(|s| &s.id == id));
        if self
            .account
            .as_ref()
            .is_some_and(|c| c.key.0 != plan.directory)
        {
            self.account = None;
        }
        let mut next = SystemTime::now() + Duration::from_secs(3600);
        let mut wrote = false;
        for site in &plan.sites {
            let mut status = SiteStatus::default();
            let retry_at = self.sites.get(&site.id).and_then(|s| s.retry_at);
            match self.due(plan, site, &mut status).await {
                Due::At(t) => next = next.min(t),
                Due::Now(_, _) if retry_at.is_some_and(|t| SystemTime::now() < t) => {
                    let t = retry_at.unwrap();
                    status.retry_at = Some(unix(t));
                    next = next.min(t);
                }
                Due::Now(why, replaces) => match self
                    .issue(plan, site, why, replaces, status.not_after)
                    .await
                {
                    Ok(Some(not_after)) => {
                        wrote = true;
                        self.sites.insert(site.id.clone(), SiteState::default());
                        status.not_after = Some(not_after);
                        status.renew_at = None;
                    }
                    // Another process holds the storage lock: look again shortly.
                    Ok(None) => next = next.min(SystemTime::now() + Duration::from_secs(60)),
                    Err(e) => {
                        metrics::ACME_ORDERS.with_label_values(&["failure"]).inc();
                        let st = self.sites.entry(site.id.clone()).or_default();
                        st.failures += 1;
                        let t = SystemTime::now() + backoff(st.failures);
                        st.retry_at = Some(t);
                        error!(
                            "ACME site {} ({}): {e}; next try in {}s",
                            site.id,
                            site.hosts.join(", "),
                            backoff(st.failures).as_secs()
                        );
                        status.last_error = Some(e);
                        status.retry_at = Some(unix(t));
                        next = next.min(t);
                    }
                },
            }
            if let Some(old) = shared.acme.sites.lock().unwrap().get(&*site.id)
                && status.last_error.is_none()
                && status.retry_at.is_some()
            {
                status.last_error = old.last_error.clone();
            }
            shared
                .acme
                .sites
                .lock()
                .unwrap()
                .insert(site.id.to_string(), status);
        }
        shared
            .acme
            .sites
            .lock()
            .unwrap()
            .retain(|id, _| plan.sites.iter().any(|s| &*s.id == id));
        if wrote {
            shared.reload_now.notify_one();
        }
        next.duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
            .max(Duration::from_secs(1))
    }

    /// Order and store a certificate. `Ok(None)`: another process holds the lock.
    async fn issue(
        &mut self,
        plan: &AcmePlan,
        site: &AcmeSite,
        why: &str,
        replaces: Option<CertificateIdentifier<'static>>,
        seen: Option<i64>,
    ) -> Result<Option<i64>, String> {
        let Some(_lock) =
            try_lock(&plan.storage).map_err(|e| format!("lock {}: {e}", plan.storage.display()))?
        else {
            return Ok(None);
        };
        // Another process (the other one during a graceful upgrade) may have stored a new
        // certificate since `seen` was read; the next check judges that one.
        if let Some(c) = std::fs::read(&site.path).ok().and_then(|p| inspect(&p))
            && c.names == site.hosts
            && Some(c.not_after) != seen
        {
            return Ok(Some(c.not_after));
        }
        info!(
            "ACME site {} ({}): {why}, ordering a certificate",
            site.id,
            site.hosts.join(", ")
        );
        let account = self.account(plan).await?.account.clone();
        let pem = order(&account, site, plan.profile.as_deref(), replaces).await?;
        write_private(&site.path, &pem).map_err(|e| format!("{}: {e}", site.path.display()))?;
        metrics::ACME_ORDERS.with_label_values(&["success"]).inc();
        let not_after = inspect(&pem).map_or(0, |c| c.not_after);
        info!(
            "ACME site {}: certificate stored in {}, valid until {}",
            site.id,
            site.path.display(),
            crate::timefmt::rfc3339(not_after.max(0) as u64)
        );
        Ok(Some(not_after))
    }
}

pub struct AcmeService {
    shared: Arc<Shared>,
}

impl AcmeService {
    pub fn new(shared: Arc<Shared>) -> AcmeService {
        AcmeService { shared }
    }
}

#[async_trait]
impl BackgroundService for AcmeService {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        let mut m = Manager::default();
        loop {
            let plan = self.shared.runtime.load().acme.clone();
            let wait = match plan {
                Some(plan) if !plan.sites.is_empty() => tokio::select! {
                    _ = shutdown.changed() => return,
                    w = m.tick(&plan, &self.shared) => w,
                },
                _ => Duration::from_secs(3600),
            };
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(wait) => {}
                _ = self.shared.acme_wake.notified() => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CertConfig;
    use crate::tls::CertKey;

    #[test]
    fn challenge_tokens_are_withdrawn() {
        {
            let mut p = Published::default();
            p.add("tok-acme-test", "tok-acme-test.thumb");
            assert_eq!(
                challenge_answer(b"tok-acme-test").as_deref(),
                Some("tok-acme-test.thumb")
            );
        }
        assert_eq!(challenge_answer(b"tok-acme-test"), None);
        assert_eq!(challenge_answer(b"\xff"), None);
    }

    #[test]
    fn renewal_from_lifetime() {
        let day = 86_400;
        let c = Stored {
            names: vec![],
            not_before: 0,
            not_after: 90 * day,
            id: None,
        };
        assert_eq!(lifetime_renewal(&c, None), 60 * day);
        let d = |days: u64| Some(Duration::from_secs(days * day as u64));
        assert_eq!(lifetime_renewal(&c, d(30)), 60 * day);
        // capped at half the lifetime: a 6-day certificate is not due when issued
        let short = Stored {
            not_after: 6 * day,
            ..c
        };
        assert_eq!(lifetime_renewal(&short, d(30)), 3 * day);
        assert_eq!(lifetime_renewal(&short, None), 4 * day);
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(300));
        assert_eq!(backoff(2), Duration::from_secs(600));
        assert_eq!(backoff(4), Duration::from_secs(2400));
        assert_eq!(backoff(30), Duration::from_secs(6 * 3600));
    }

    /// A CA-issued-like leaf: signed by a CA key, with an AKI and a serial whose top bit is
    /// set (so ARI needs the leading zero of the DER integer).
    fn issued(hosts: &[&str]) -> (String, PKey<Private>) {
        use openssl::bn::BigNum;
        use openssl::x509::X509Builder;
        use openssl::x509::extension::{AuthorityKeyIdentifier, SubjectKeyIdentifier};
        let ca_key = new_key().unwrap();
        let mut ca_name = X509NameBuilder::new().unwrap();
        ca_name
            .append_entry_by_nid(Nid::COMMONNAME, "test CA")
            .unwrap();
        let ca_name = ca_name.build();
        let mut ca = X509Builder::new().unwrap();
        ca.set_version(2).unwrap();
        ca.set_subject_name(&ca_name).unwrap();
        ca.set_issuer_name(&ca_name).unwrap();
        ca.set_pubkey(&ca_key).unwrap();
        ca.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        ca.set_not_after(&Asn1Time::days_from_now(365).unwrap())
            .unwrap();
        let ski = SubjectKeyIdentifier::new()
            .build(&ca.x509v3_context(None, None))
            .unwrap();
        ca.append_extension(ski).unwrap();
        ca.sign(&ca_key, MessageDigest::sha256()).unwrap();
        let ca = ca.build();

        let key = new_key().unwrap();
        let mut b = X509Builder::new().unwrap();
        b.set_version(2).unwrap();
        let serial = BigNum::from_hex_str("8badf00d").unwrap();
        b.set_serial_number(&serial.to_asn1_integer().unwrap())
            .unwrap();
        b.set_issuer_name(ca.subject_name()).unwrap();
        b.set_subject_name(&X509NameBuilder::new().unwrap().build())
            .unwrap();
        b.set_pubkey(&key).unwrap();
        b.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        b.set_not_after(&Asn1Time::days_from_now(90).unwrap())
            .unwrap();
        let mut san = SubjectAlternativeName::new();
        for h in hosts {
            san.dns(h);
        }
        let ext = san.build(&b.x509v3_context(Some(&ca), None)).unwrap();
        b.append_extension(ext).unwrap();
        let aki = AuthorityKeyIdentifier::new()
            .keyid(true)
            .build(&b.x509v3_context(Some(&ca), None))
            .unwrap();
        b.append_extension(aki).unwrap();
        b.sign(&ca_key, MessageDigest::sha256()).unwrap();
        let leaf = b.build();
        let chain = format!(
            "{}{}",
            String::from_utf8(leaf.to_pem().unwrap()).unwrap(),
            String::from_utf8(ca.to_pem().unwrap()).unwrap()
        );
        (chain, key)
    }

    #[test]
    fn stored_bundle_loads_and_identifies() {
        let hosts = vec!["a.test".to_string(), "www.a.test".to_string()];
        let (chain, key) = issued(&["www.a.test", "A.test"]);
        let pem = bundle(&chain, &key, &hosts).unwrap();

        let c = inspect(&pem).unwrap();
        assert_eq!(c.names, hosts);
        assert!((c.not_after - c.not_before - 90 * 86_400).abs() <= 2);
        // serial 0x8badf00d has its top bit set: DER content is 00 8b ad f0 0d
        let id = c.id.unwrap().to_string();
        assert!(id.ends_with(".AIut8A0"), "{id}");

        // The proxy reads key and chain from the same file.
        let dir = std::env::temp_dir().join(format!("conduit-acme-bundle-{}", std::process::id()));
        let path = dir.join("site.pem");
        write_private(&path, &pem).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let file = path.to_string_lossy().into_owned();
        let ck = CertKey::load(&CertConfig {
            name: "acme:site".into(),
            cert: file.clone(),
            key: file,
        })
        .unwrap();
        assert_eq!(ck.dns_names, ["www.a.test", "a.test"]);
        std::fs::remove_dir_all(&dir).ok();

        // A chain for other names, or for another key, is refused.
        let err = bundle(&chain, &key, &["a.test".to_string()]).unwrap_err();
        assert!(err.contains("names"), "{err}");
        let err = bundle(&chain, &new_key().unwrap(), &hosts).unwrap_err();
        assert!(err.contains("key"), "{err}");
    }

    #[test]
    fn csr_names_every_host() {
        let key = new_key().unwrap();
        let hosts = vec!["a.test".to_string(), "b.test".to_string()];
        let der = csr(&key, &hosts).unwrap();
        let req = openssl::x509::X509Req::from_der(&der).unwrap();
        assert!(req.verify(&key).unwrap());
        let text = String::from_utf8(req.to_text().unwrap()).unwrap();
        assert!(text.contains("DNS:a.test, DNS:b.test"), "{text}");
    }

    #[test]
    fn storage_lock_is_exclusive() {
        let dir = std::env::temp_dir().join(format!("conduit-acme-lock-{}", std::process::id()));
        let first = try_lock(&dir).unwrap();
        assert!(first.is_some());
        // flock locks belong to the open file description, so a second open conflicts.
        assert!(try_lock(&dir).unwrap().is_none());
        drop(first);
        assert!(try_lock(&dir).unwrap().is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
