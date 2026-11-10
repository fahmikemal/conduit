//! Copy into the disposable build's tests/audit_recheck.rs.
//! These tests call production Rust functions, without altering those functions.
use conduit::config::RateLimitConfig;
use conduit::limits::RateLimit;
use conduit::plugin::Plugin;
use conduit::rules::RequestView;
use conduit::units::Dur;
use std::sync::{Arc, Barrier};
use std::time::Duration;

#[test]
fn audit_recheck_f05_duration_overflow() {
    assert_eq!(conduit::units::parse_duration("60m").unwrap().as_secs(), 3600);
    let outcome = std::panic::catch_unwind(|| {
        conduit::units::parse_duration("9223372036854775808m")
    });
    // This test uses the dev profile with overflow checks, as recorded in the run log.
    assert!(outcome.is_err(), "expected unchecked multiplication to panic in dev profile");
    println!("F05: production parse_duration panics on overflowing input; ordinary control = 3600s");
}

#[test]
fn audit_recheck_f07_rate_admission_race() {
    const N: usize = 16;
    let c = RateLimitConfig {
        requests: 1,
        per: Dur(Duration::from_secs(3600)),
        key: "client_ip".into(),
        matcher: None,
        status: 429,
    };
    let mut reproduced = None;
    for trial in 0..200 {
        let limiter = Arc::new(RateLimit::new(&c, "audit-recheck-rate", 0).unwrap());
        let barrier = Arc::new(Barrier::new(N));
        let workers: Vec<_> = (0..N).map(|_| {
            let limiter = limiter.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let method = http::Method::GET;
                let headers = http::HeaderMap::new();
                let view = RequestView {
                    method: &method,
                    https: false,
                    host: "audit.test",
                    client: Some("192.0.2.1".parse().unwrap()),
                    path: b"/",
                    uri: b"/",
                    query: b"",
                    headers: &headers,
                };
                barrier.wait();
                usize::from(limiter.request(&view).is_none())
            })
        }).collect();
        let accepted: usize = workers.into_iter().map(|w| w.join().unwrap()).sum();
        if accepted > 1 {
            reproduced = Some((trial, accepted));
            break;
        }
    }
    assert!(reproduced.is_some(), "race not observed; this result alone does not disprove it");
    println!("F07: observed {:?} (trial, accepted) for configured 1/3600s; no test hook inside production code", reproduced.unwrap());
}

#[test]
fn audit_recheck_f09_variance_collision() {
    let mut response = pingora::http::ResponseHeader::build(200, None).unwrap();
    response.insert_header("content-encoding", "deflate").unwrap();
    response.insert_header("vary", "Accept-Encoding").unwrap();
    let now = std::time::SystemTime::now();
    let meta = pingora::cache::CacheMeta::new(now + Duration::from_secs(600), now, 0, 0, response);
    let request = |encoding: &str| {
        let mut r = pingora::http::RequestHeader::build("GET", b"/", None).unwrap();
        r.insert_header("accept-encoding", encoding).unwrap();
        r
    };
    let a = conduit::cache::variance(&meta, &request("deflate"));
    let b = conduit::cache::variance(&meta, &request("identity"));
    assert!(a.is_some());
    assert_eq!(a, b);
    assert_ne!(a, conduit::cache::variance(&meta, &request("gzip")));
    println!("F09: actual variance collides for deflate/identity; gzip control differs");
}

#[test]
fn audit_recheck_f08_wildcard_coverage_warning() {
    let d = std::env::temp_dir().join(format!("conduit-audit-f08-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let crt = d.join("wild.crt");
    let key = d.join("wild.key");
    let output = std::process::Command::new("openssl").args([
        "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
        "-nodes", "-days", "1", "-subj", "/CN=wild",
        "-addext", "subjectAltName=DNS:*.example.test", "-keyout",
    ]).arg(&key).arg("-out").arg(&crt).output().unwrap();
    assert!(output.status.success());
    let c = conduit::config::CertConfig {
        name: "wild".into(), cert: crt.to_string_lossy().into(), key: key.to_string_lossy().into(),
    };
    let (store, _) = conduit::tls::CertStore::build(&[c], None).unwrap();
    assert!(store.covers("app.example.test"));
    assert!(!store.covers("*.example.test"));
    let cfg = conduit::config::parse(&format!(
        "listen: {{ https: [\"127.0.0.1:0\"] }}\ntls:\n  certs: [{{ name: wild, cert: {}, key: {} }}]\nsites:\n  - {{ id: wild, hosts: [\"*.example.test\"], upstream: \"http://127.0.0.1:1\", force_https: true }}\n",
        crt.display(), key.display()
    )).unwrap();
    let built = conduit::runtime::build(&cfg, None, None).unwrap();
    assert!(built.warnings.iter().any(|w| w.contains("no certificate covers *.example.test")));
    println!("F08: actual runtime warning is {:?}, although the wildcard certificate is selected for app.example.test", built.warnings);
    std::fs::remove_dir_all(&d).unwrap();
}
