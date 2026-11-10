//! Certificates chosen per handshake from the SNI: site override, exact SAN, wildcard SAN,
//! then the default certificate.

use std::any::Any;
use std::collections::HashMap;
use std::ffi::{c_int, c_uchar, c_uint};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use foreign_types::ForeignTypeRef;
use http::HeaderValue;
use log::{error, warn};
use openssl::asn1::Asn1Time;
use openssl::error::ErrorStack;
use openssl::ex_data::Index;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{ClientHelloResponse, NameType, Ssl, SslContextBuilder, SslRef, SslVerifyMode};
use openssl::x509::store::{X509Store, X509StoreBuilder};
use openssl::x509::{X509, X509NameRef, X509VerifyResult};
use pingora::listeners::TlsAccept;
use pingora::tls::ext;

use crate::config::CertConfig;
use crate::proxy::Shared;

pub struct CertKey {
    pub name: String,
    leaf: X509,
    chain: Vec<X509>,
    key: PKey<Private>,
    pub dns_names: Vec<String>,
    pub not_after: String,
    /// notAfter as seconds since the Unix epoch.
    pub not_after_unix: i64,
}

impl CertKey {
    pub fn load(c: &CertConfig) -> Result<CertKey, String> {
        let ctx = |what: &str, e: &dyn std::fmt::Display| format!("cert {:?}: {what}: {e}", c.name);
        let pem = std::fs::read(&c.cert).map_err(|e| ctx(&format!("read {}", c.cert), &e))?;
        let mut stack = X509::stack_from_pem(&pem).map_err(|e| ctx("parse certificate", &e))?;
        if stack.is_empty() {
            return Err(ctx("parse certificate", &"no certificate in file"));
        }
        let leaf = stack.remove(0);
        let key_pem = std::fs::read(&c.key).map_err(|e| ctx(&format!("read {}", c.key), &e))?;
        let key = PKey::private_key_from_pem(&key_pem).map_err(|e| ctx("parse key", &e))?;
        let public = leaf.public_key().map_err(|e| ctx("read public key", &e))?;
        if !public.public_eq(&key) {
            return Err(ctx("key", &"does not match the certificate"));
        }
        let mut dns_names: Vec<String> = leaf
            .subject_alt_names()
            .map(|sans| {
                sans.iter()
                    .filter_map(|n| n.dnsname().map(str::to_ascii_lowercase))
                    .collect()
            })
            .unwrap_or_default();
        if dns_names.is_empty()
            && let Some(cn) = leaf
                .subject_name()
                .entries_by_nid(openssl::nid::Nid::COMMONNAME)
                .next()
                .and_then(|e| e.data().to_string().ok())
        {
            dns_names.push(cn.to_ascii_lowercase());
        }
        let not_after = leaf.not_after().to_string();
        let not_after_unix = Asn1Time::from_unix(0)
            .and_then(|epoch| epoch.diff(leaf.not_after()))
            .map_or(0, |d| i64::from(d.days) * 86_400 + i64::from(d.secs));
        if let Ok(now) = Asn1Time::days_from_now(0)
            && leaf.not_after() < now
        {
            warn!("certificate {:?} expired on {not_after}", c.name);
        }
        Ok(CertKey {
            name: c.name.clone(),
            leaf,
            chain: stack,
            key,
            dns_names,
            not_after,
            not_after_unix,
        })
    }
}

#[derive(Default)]
pub struct CertStore {
    by_name: HashMap<String, Arc<CertKey>>,
    /// Host -> certificate from a site's `cert:` override.
    overrides: HashMap<String, Arc<CertKey>>,
    /// Parent domain -> explicit site override for one-label wildcard SNI names.
    wildcard_overrides: HashMap<String, Arc<CertKey>>,
    exact: HashMap<String, Arc<CertKey>>,
    /// Parent domain (`example.com` for `*.example.com`) -> certificate.
    wildcard: HashMap<String, Arc<CertKey>>,
    default: Option<Arc<CertKey>>,
}

impl CertStore {
    pub fn build(
        certs: &[CertConfig],
        default: Option<&str>,
    ) -> Result<(CertStore, Vec<PathBuf>), String> {
        let mut store = CertStore::default();
        let mut watched = Vec::new();
        for c in certs {
            if store.by_name.contains_key(&c.name) {
                return Err(format!("duplicate certificate name {:?}", c.name));
            }
            let ck = Arc::new(CertKey::load(c)?);
            watched.push(PathBuf::from(&c.cert));
            watched.push(PathBuf::from(&c.key));
            for n in &ck.dns_names {
                if let Some(parent) = n.strip_prefix("*.") {
                    store
                        .wildcard
                        .entry(parent.to_string())
                        .or_insert_with(|| ck.clone());
                } else {
                    store.exact.entry(n.clone()).or_insert_with(|| ck.clone());
                }
            }
            store.by_name.insert(c.name.clone(), ck);
        }
        if let Some(d) = default {
            store.default = Some(store.by_name.get(d).cloned().ok_or_else(|| {
                format!("tls.default_cert {d:?} is not a configured certificate")
            })?);
        }
        Ok((store, watched))
    }

    pub fn add_override(&mut self, host: &str, cert: &str) -> Result<(), String> {
        let ck = self
            .by_name
            .get(cert)
            .cloned()
            .ok_or_else(|| format!("host {host:?} uses unknown certificate {cert:?}"))?;
        let host = host.to_ascii_lowercase();
        if let Some(parent) = host.strip_prefix("*.") {
            self.wildcard_overrides.insert(parent.to_owned(), ck);
        } else {
            self.overrides.insert(host, ck);
        }
        Ok(())
    }

    /// A certificate conduit manages itself (ACME), preferred for `hosts` over SNI-based
    /// selection. Returns the hosts it does not cover.
    pub fn add_managed(&mut self, ck: CertKey, hosts: &[String]) -> Result<Vec<String>, String> {
        if self.by_name.contains_key(&ck.name) {
            return Err(format!("duplicate certificate name {:?}", ck.name));
        }
        let ck = Arc::new(ck);
        let mut missing = Vec::new();
        for h in hosts {
            if ck.dns_names.iter().any(|n| n == h) {
                self.overrides.insert(h.clone(), ck.clone());
            } else {
                missing.push(h.clone());
            }
        }
        self.by_name.insert(ck.name.clone(), ck);
        Ok(missing)
    }

    /// Whether a certificate other than the default one matches this host.
    pub fn covers(&self, host: &str) -> bool {
        self.select_specific(host).is_some()
    }

    fn select_specific(&self, host: &str) -> Option<&CertKey> {
        // Configured wildcard patterns already name their parent; do not discard a label.
        if let Some(parent) = host.strip_prefix("*.") {
            return self
                .wildcard_overrides
                .get(parent)
                .or_else(|| self.wildcard.get(parent))
                .map(|c| &**c);
        }
        if let Some(c) = self.overrides.get(host) {
            return Some(c);
        }
        // A wildcard covers exactly one label.
        let parent = host.split_once('.').map(|(_, parent)| parent);
        parent
            .and_then(|p| self.wildcard_overrides.get(p))
            .or_else(|| self.exact.get(host))
            .or_else(|| parent.and_then(|p| self.wildcard.get(p)))
            .map(|c| &**c)
    }

    pub fn select(&self, sni: Option<&str>) -> Option<&CertKey> {
        let specific = sni.and_then(|s| {
            let s = s.trim_end_matches('.').to_ascii_lowercase();
            self.select_specific(&s)
        });
        specific.or(self.default.as_deref())
    }

    pub fn all(&self) -> impl Iterator<Item = &CertKey> {
        self.by_name.values().map(|c| &**c)
    }

    pub fn summary(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .by_name
            .values()
            .map(|c| {
                format!(
                    "{} [{}] expires {}",
                    c.name,
                    c.dns_names.join(", "),
                    c.not_after
                )
            })
            .collect();
        v.sort();
        v
    }
}

/// Picks the certificate during the handshake; the store is swapped on reload.
pub struct CertResolver {
    store: Arc<ArcSwap<CertStore>>,
}

impl CertResolver {
    pub fn new(store: Arc<ArcSwap<CertStore>>) -> Self {
        CertResolver { store }
    }
}

/// The CAs a site takes client certificates from (`mtls.ca`).
pub struct ClientTrust {
    /// SHA-256 of the CA file. It names these CAs in TLS sessions and connections, and stays
    /// the same across reloads while the file does.
    pub id: [u8; 32],
    store: X509Store,
    cas: Vec<X509>,
}

impl ClientTrust {
    pub fn load(path: &str) -> Result<ClientTrust, String> {
        let err = |e: &dyn std::fmt::Display| format!("mtls.ca {path}: {e}");
        let pem = std::fs::read(path).map_err(|e| err(&e))?;
        let cas = X509::stack_from_pem(&pem).map_err(|e| err(&e))?;
        if cas.is_empty() {
            return Err(err(&"no certificate in file"));
        }
        let mut store = X509StoreBuilder::new().map_err(|e| err(&e))?;
        for ca in &cas {
            store.add_cert(ca.clone()).map_err(|e| err(&e))?;
        }
        Ok(ClientTrust {
            id: openssl::sha::sha256(&pem),
            store: store.build(),
            cas,
        })
    }
}

/// What the handshake of a connection to an mTLS site showed of the client; Pingora keeps it
/// in the connection's `SslDigest`.
pub struct ClientCert {
    /// `ClientTrust::id` of the CAs the handshake asked for.
    pub trust: [u8; 32],
    pub status: ClientStatus,
}

pub enum ClientStatus {
    /// The client sent no certificate.
    Missing,
    /// Verified. The subject (RFC 4514) and the SHA-256 fingerprint (hex), ready for headers.
    Valid {
        subject: HeaderValue,
        fingerprint: HeaderValue,
    },
    /// Sent, but it failed verification.
    Invalid(String),
}

impl ClientCert {
    fn read(ssl: &SslRef, trust: [u8; 32]) -> ClientCert {
        let status = match ssl.peer_certificate() {
            None => ClientStatus::Missing,
            Some(cert) => match ssl.verify_result() {
                X509VerifyResult::OK => {
                    let fingerprint = cert
                        .digest(MessageDigest::sha256())
                        .map(|d| {
                            d.iter().fold(String::with_capacity(64), |mut s, b| {
                                let _ = write!(s, "{b:02x}");
                                s
                            })
                        })
                        .unwrap_or_default();
                    ClientStatus::Valid {
                        subject: HeaderValue::from_maybe_shared(distinguished_name(
                            cert.subject_name(),
                        ))
                        .unwrap_or_else(|_| HeaderValue::from_static("")),
                        fingerprint: HeaderValue::from_maybe_shared(fingerprint)
                            .unwrap_or_else(|_| HeaderValue::from_static("")),
                    }
                }
                e => ClientStatus::Invalid(e.error_string().to_string()),
            },
        };
        ClientCert { trust, status }
    }
}

/// A name as RFC 4514 writes it (`CN=alice,O=Example`, last RDN first), with control
/// characters escaped so that it fits in a header.
fn distinguished_name(name: &X509NameRef) -> bytes::Bytes {
    let entries: Vec<_> = name.entries().collect();
    let mut out = String::new();
    for (i, e) in entries.iter().rev().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match e.object().nid().short_name() {
            Ok(n) => out.push_str(n),
            Err(_) => out.push_str(&e.object().to_string()),
        }
        out.push('=');
        let value = e
            .data()
            .to_string()
            .unwrap_or_else(|_| String::from_utf8_lossy(e.data().as_slice()).into_owned());
        let last = value.chars().count().saturating_sub(1);
        for (j, c) in value.chars().enumerate() {
            match c {
                '"' | '+' | ',' | ';' | '<' | '>' | '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                '#' | ' ' if j == 0 => {
                    out.push('\\');
                    out.push(c);
                }
                ' ' if j == last => out.push_str("\\ "),
                c if c.is_control() => {
                    let mut buf = [0; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        let _ = write!(out, "\\{b:02x}");
                    }
                }
                c => out.push(c),
            }
        }
    }
    bytes::Bytes::from(out)
}

/// The host name in the body of a ClientHello `server_name` extension (RFC 6066, 3).
fn sni_from_extension(ext: &[u8]) -> Option<&str> {
    let (len, mut list) = ext.split_first_chunk::<2>()?;
    if usize::from(u16::from_be_bytes(*len)) != list.len() {
        return None;
    }
    while let Some((&[kind, a, b], rest)) = list.split_first_chunk::<3>() {
        let n = usize::from(u16::from_be_bytes([a, b]));
        let name = rest.get(..n)?;
        if kind == 0 {
            return std::str::from_utf8(name).ok();
        }
        list = &rest[n..];
    }
    None
}

static TRUST_INDEX: LazyLock<Index<Ssl, [u8; 32]>> =
    LazyLock::new(|| Ssl::new_ex_index().expect("an OpenSSL ex_data index"));

unsafe extern "C" {
    fn SSL_set_session_id_context(
        ssl: *mut openssl_sys::SSL,
        sid_ctx: *const c_uchar,
        len: c_uint,
    ) -> c_int;
}

fn client_hello_sni(ssl: &SslRef) -> Option<String> {
    let mut out: *const c_uchar = std::ptr::null();
    let mut len = 0;
    // SAFETY: only called from the ClientHello callback, while OpenSSL holds the ClientHello;
    // `out` points into it and is copied out before the callback returns.
    let ext = unsafe {
        if openssl_sys::SSL_client_hello_get0_ext(ssl.as_ptr(), 0, &mut out, &mut len) != 1
            || out.is_null()
        {
            return None;
        }
        std::slice::from_raw_parts(out, len)
    };
    sni_from_extension(ext).map(|s| s.trim_end_matches('.').to_ascii_lowercase())
}

/// Ask this handshake for a client certificate from `trust`.
fn ask_for_certificate(ssl: &mut SslRef, trust: &ClientTrust) -> Result<(), ErrorStack> {
    // OpenSSL resumes a session only in the context it was made in. With the CAs' hash as the
    // context, a session from a site without mTLS, from another site's CAs, or from before the
    // CA file changed is never resumed here: the client does a full handshake and its
    // certificate is verified again. A session made here keeps its verified certificate.
    // SAFETY: `ssl` is a live SSL; OpenSSL copies the 32 bytes.
    let ok = unsafe {
        SSL_set_session_id_context(ssl.as_ptr(), trust.id.as_ptr(), trust.id.len() as c_uint)
    };
    if ok != 1 {
        return Err(ErrorStack::get());
    }
    ext::ssl_set_verify_cert_store(ssl, &trust.store)?;
    for ca in &trust.cas {
        ssl.add_client_ca(ca)?;
    }
    // A missing or failing certificate does not fail the handshake: the request is refused
    // with a status and an access log line instead, and `optional` sites can take it.
    ssl.set_verify_callback(SslVerifyMode::PEER, |_, _| true);
    ssl.set_ex_data(*TRUST_INDEX, trust.id);
    Ok(())
}

/// Ask clients of mTLS sites for a certificate. This runs on the ClientHello, before OpenSSL
/// decides whether to resume a session, which is what keeps sessions apart per CA set.
pub fn ask_client_certs(ctx: &mut SslContextBuilder, shared: Arc<Shared>) {
    ctx.set_client_hello_callback(move |ssl, _alert| {
        let rt = shared.runtime.load();
        if !rt.mtls {
            return Ok(ClientHelloResponse::SUCCESS);
        }
        let Some(host) = client_hello_sni(ssl) else {
            return Ok(ClientHelloResponse::SUCCESS);
        };
        if let Some(m) = rt.router.lookup(&host).and_then(|s| s.mtls.as_ref()) {
            ask_for_certificate(ssl, &m.trust).inspect_err(|e| {
                error!("cannot ask {host} clients for a certificate: {e}");
            })?;
        }
        Ok(ClientHelloResponse::SUCCESS)
    });
}

#[async_trait]
impl TlsAccept for CertResolver {
    async fn handshake_complete_callback(
        &self,
        ssl: &SslRef,
    ) -> Option<Arc<dyn Any + Send + Sync>> {
        // Set on handshakes that asked for a certificate, resumed ones included.
        let trust = *ssl.ex_data(*TRUST_INDEX)?;
        Some(Arc::new(ClientCert::read(ssl, trust)))
    }

    async fn certificate_callback(&self, ssl: &mut SslRef) {
        let sni = ssl.servername(NameType::HOST_NAME).map(str::to_owned);
        let store = self.store.load();
        let Some(ck) = store.select(sni.as_deref()) else {
            // No certificate: the handshake fails, as it should for unknown names.
            return;
        };
        let result = ext::ssl_use_certificate(ssl, &ck.leaf)
            .and_then(|_| {
                ck.chain
                    .iter()
                    .try_for_each(|c| ext::ssl_add_chain_cert(ssl, c))
            })
            .and_then(|_| ext::ssl_use_private_key(ssl, &ck.key));
        if let Err(e) = result {
            error!(
                "cannot apply certificate {:?} for SNI {sni:?}: {e}",
                ck.name
            );
        }
    }
}

#[cfg(test)]
pub mod test_certs {
    //! Self-signed certificates for tests.
    use openssl::asn1::Asn1Time;
    use openssl::bn::{BigNum, MsbOption};
    use openssl::ec::{EcGroup, EcKey};
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::pkey::PKey;
    use openssl::x509::extension::SubjectAlternativeName;
    use openssl::x509::{X509Builder, X509NameBuilder};

    /// PEM (cert, key) for the given DNS names.
    pub fn self_signed(names: &[&str]) -> (Vec<u8>, Vec<u8>) {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_nid(Nid::COMMONNAME, names[0]).unwrap();
        let name = name.build();
        let mut b = X509Builder::new().unwrap();
        b.set_version(2).unwrap();
        let mut serial = BigNum::new().unwrap();
        serial.rand(64, MsbOption::MAYBE_ZERO, false).unwrap();
        b.set_serial_number(&serial.to_asn1_integer().unwrap())
            .unwrap();
        b.set_subject_name(&name).unwrap();
        b.set_issuer_name(&name).unwrap();
        b.set_pubkey(&key).unwrap();
        b.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        b.set_not_after(&Asn1Time::days_from_now(30).unwrap())
            .unwrap();
        let mut san = SubjectAlternativeName::new();
        for n in names {
            san.dns(n);
        }
        let ext = san.build(&b.x509v3_context(None, None)).unwrap();
        b.append_extension(ext).unwrap();
        b.sign(&key, MessageDigest::sha256()).unwrap();
        (
            b.build().to_pem().unwrap(),
            key.private_key_to_pem_pkcs8().unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pair(dir: &std::path::Path, name: &str, names: &[&str]) -> CertConfig {
        let (cert, key) = test_certs::self_signed(names);
        let c = dir.join(format!("{name}.crt"));
        let k = dir.join(format!("{name}.key"));
        std::fs::write(&c, cert).unwrap();
        std::fs::write(&k, key).unwrap();
        CertConfig {
            name: name.into(),
            cert: c.to_string_lossy().into(),
            key: k.to_string_lossy().into(),
        }
    }

    #[test]
    fn selection_order() {
        let dir = std::env::temp_dir().join(format!("conduit-tls-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wild = write_pair(&dir, "wild", &["*.example.com", "example.com"]);
        let exact = write_pair(&dir, "forgejo", &["forgejo.example.com"]);
        let other = write_pair(&dir, "other", &["other.test"]);
        let (mut store, watched) = CertStore::build(&[wild, exact, other], Some("other")).unwrap();
        assert_eq!(watched.len(), 6);
        store.add_override("special.example.com", "other").unwrap();

        let pick = |s: &str| store.select(Some(s)).map(|c| c.name.clone());
        assert_eq!(pick("forgejo.example.com").as_deref(), Some("forgejo"));
        assert_eq!(pick("FORGEJO.example.com.").as_deref(), Some("forgejo"));
        assert_eq!(pick("app.example.com").as_deref(), Some("wild"));
        assert_eq!(pick("example.com").as_deref(), Some("wild"));
        assert_eq!(pick("special.example.com").as_deref(), Some("other"));
        // a wildcard covers one label only; deeper names fall back to the default
        assert_eq!(pick("api.remote.example.com").as_deref(), Some("other"));
        assert_eq!(store.select(None).map(|c| c.name.as_str()), Some("other"));
        assert!(store.add_override("x", "missing").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mismatched_key_rejected() {
        let dir = std::env::temp_dir().join(format!("conduit-tls-mismatch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = write_pair(&dir, "a", &["a.test"]);
        let b = write_pair(&dir, "b", &["b.test"]);
        let mixed = CertConfig {
            name: "mixed".into(),
            cert: a.cert,
            key: b.key,
        };
        let err = CertKey::load(&mixed).err().unwrap();
        assert!(err.contains("does not match"), "{err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sni_extension() {
        let entry = |kind: u8, name: &[u8]| {
            let mut e = vec![kind];
            e.extend_from_slice(&(name.len() as u16).to_be_bytes());
            e.extend_from_slice(name);
            e
        };
        let list = |entries: &[Vec<u8>]| {
            let body: Vec<u8> = entries.concat();
            let mut l = (body.len() as u16).to_be_bytes().to_vec();
            l.extend(body);
            l
        };
        let one = list(&[entry(0, b"App.Example.com")]);
        assert_eq!(sni_from_extension(&one), Some("App.Example.com"));
        let other_first = list(&[entry(7, b"x"), entry(0, b"a.test")]);
        assert_eq!(sni_from_extension(&other_first), Some("a.test"));
        assert_eq!(sni_from_extension(&one[..one.len() - 1]), None);
        assert_eq!(sni_from_extension(&[]), None);
        assert_eq!(sni_from_extension(&[0, 4, 0, 0, 9, b'a']), None);
        assert_eq!(sni_from_extension(&list(&[entry(7, b"x")])), None);
    }

    #[test]
    fn subject_as_rfc4514() {
        use openssl::x509::X509NameBuilder;
        let mut n = X509NameBuilder::new().unwrap();
        n.append_entry_by_text("C", "ID").unwrap();
        n.append_entry_by_text("O", "Acme, Inc+Co").unwrap();
        n.append_entry_by_text("CN", " alice\n").unwrap();
        let n = n.build();
        assert_eq!(
            &distinguished_name(&n)[..],
            b"CN=\\ alice\\0a,O=Acme\\, Inc\\+Co,C=ID"
        );
        assert!(HeaderValue::from_maybe_shared(distinguished_name(&n)).is_ok());
    }

    #[test]
    fn client_trust_from_file() {
        let dir = std::env::temp_dir().join(format!("conduit-tls-trust-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ca.pem");
        let pem = test_certs::self_signed(&["ca"]).0;
        std::fs::write(&path, &pem).unwrap();
        let t = ClientTrust::load(path.to_str().unwrap()).unwrap();
        assert_eq!(t.id, openssl::sha::sha256(&pem));
        assert_eq!(t.cas.len(), 1);
        std::fs::write(&path, "").unwrap();
        let err = ClientTrust::load(path.to_str().unwrap()).err().unwrap();
        assert!(err.contains("no certificate"), "{err}");
        assert!(ClientTrust::load("/nonexistent/ca.pem").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
