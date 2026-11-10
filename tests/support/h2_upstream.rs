use super::*;
use bytes::Bytes;
use openssl::ssl::{AlpnError, Ssl, SslAcceptor};
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

fn test_binary() -> std::ffi::OsString {
    std::env::var_os("CONDUIT_HTTP2_TEST_BIN")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_conduit").into())
}

#[derive(Default)]
struct Stats {
    connections: AtomicUsize,
    h1: AtomicUsize,
    h2: AtomicUsize,
    health: AtomicUsize,
    cached: AtomicUsize,
    parallel: AtomicUsize,
    active: Mutex<HashMap<usize, usize>>,
}

struct Active(Arc<Stats>, usize);
impl Active {
    fn new(stats: Arc<Stats>, connection: usize) -> Self {
        let mut active = stats.active.lock().unwrap();
        let n = active.entry(connection).or_default();
        *n += 1;
        stats.parallel.fetch_max(*n, Ordering::SeqCst);
        drop(active);
        Self(stats, connection)
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        *self.0.active.lock().unwrap().get_mut(&self.1).unwrap() -= 1;
    }
}

struct Origin {
    port: u16,
    stats: Arc<Stats>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Origin {
    fn drop(&mut self) {
        self.stop.take().unwrap().send(()).ok();
        self.thread.take().unwrap().join().unwrap();
    }
}

/// One real server accepts H1 and H2, distinguishing them by ALPN or prior knowledge.
/// `tls = Some(false)` intentionally omits ALPN to exercise strict-H2 refusal.
fn origin(dir: &Path, tls: Option<bool>) -> Origin {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let acceptor = tls.map(|advertise_h2| {
        let (cert, key) = self_signed(dir, &format!("origin-{port}"), &["origin.test"]);
        let mut b = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        b.set_certificate_file(cert, openssl::ssl::SslFiletype::PEM)
            .unwrap();
        b.set_private_key_file(key, openssl::ssl::SslFiletype::PEM)
            .unwrap();
        if advertise_h2 {
            b.set_alpn_select_callback(|_, client| {
                openssl::ssl::select_next_proto(b"\x02h2\x08http/1.1", client)
                    .ok_or(AlpnError::NOACK)
            });
        }
        b.build()
    });
    let stats = Arc::new(Stats::default());
    let state = stats.clone();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let thread = thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                loop {
                    let socket = tokio::select! {
                        _ = &mut stopped => break,
                        result = listener.accept() => result.unwrap().0,
                    };
                    let acceptor = acceptor.clone();
                    let stats = state.clone();
                    let id = stats.connections.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        let (io, h2): (Box<dyn Io>, bool) = if let Some(acceptor) = acceptor {
                            let ssl = Ssl::new(acceptor.context()).unwrap();
                            let mut io = tokio_openssl::SslStream::new(ssl, socket).unwrap();
                            if Pin::new(&mut io).accept().await.is_err() {
                                return;
                            }
                            let h2 = io.ssl().selected_alpn_protocol() == Some(b"h2");
                            (Box::new(io), h2)
                        } else {
                            let mut preface = [0; 24];
                            let Ok(n) = socket.peek(&mut preface).await else {
                                return;
                            };
                            let h2 = preface[..n].starts_with(b"PRI");
                            (Box::new(socket), h2)
                        };
                        if h2 {
                            serve_h2(io, stats, id).await;
                        } else {
                            serve_h1(io, stats).await;
                        }
                    });
                }
            });
    });
    Origin {
        port,
        stats,
        stop: Some(stop),
        thread: Some(thread),
    }
}

async fn serve_h1(io: Box<dyn Io>, stats: Arc<Stats>) {
    let mut io = tokio::io::BufReader::new(io);
    loop {
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if io.read_line(&mut line).await.unwrap_or(0) == 0 {
                return;
            }
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        stats.h1.fetch_add(1, Ordering::SeqCst);
        if head.to_ascii_lowercase().contains("upgrade: echo") {
            io.get_mut().write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: echo\r\nConnection: Upgrade\r\n\r\n").await.unwrap();
            let mut buf = [0; 1024];
            loop {
                let n = io.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                if io.get_mut().write_all(&buf[..n]).await.is_err() {
                    return;
                }
            }
        }
        let path = head.split_whitespace().nth(1).unwrap();
        let body = format!("h1:{path}");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Upstream-Version: 1.1\r\n\r\n{body}",
            body.len()
        );
        if io.get_mut().write_all(response.as_bytes()).await.is_err() {
            return;
        }
        if head.to_ascii_lowercase().contains("connection: close") {
            return;
        }
    }
}

async fn serve_h2(io: Box<dyn Io>, stats: Arc<Stats>, id: usize) {
    let Ok(mut connection) = h2::server::handshake(io).await else {
        return;
    };
    while let Some(Ok((mut request, mut respond))) = connection.accept().await {
        let stats = stats.clone();
        stats.h2.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            let _active = Active::new(stats.clone(), id);
            let path = request.uri().path().to_string();
            let grpc = path.starts_with("/grpc/");
            if path == "/delay" {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if path == "/health" {
                stats.health.fetch_add(1, Ordering::SeqCst);
            }
            if path == "/cached" {
                stats.cached.fetch_add(1, Ordering::SeqCst);
            }
            let mut response = http::Response::builder()
                .status(200)
                .header("x-upstream-version", "2")
                .header(
                    "x-origin-authority",
                    request.uri().authority().unwrap().as_str(),
                )
                .header("x-origin-scheme", request.uri().scheme_str().unwrap())
                .header(
                    "content-type",
                    if grpc {
                        "application/grpc"
                    } else {
                        "text/plain"
                    },
                )
                .header("cache-control", "public, max-age=600");
            if !grpc {
                response = response.header("content-length", format!("h2:{path}").len());
            }
            let response = response.body(()).unwrap();
            let Ok(mut stream) = respond.send_response(response, false) else {
                return;
            };
            while let Some(Ok(data)) = request.body_mut().data().await {
                request
                    .body_mut()
                    .flow_control()
                    .release_capacity(data.len())
                    .unwrap();
                if grpc && stream.send_data(data, false).is_err() {
                    return;
                }
            }
            if grpc {
                let mut trailers = http::HeaderMap::new();
                trailers.insert(
                    "grpc-status",
                    http::HeaderValue::from_static(if path.ends_with("/Denied") {
                        "7"
                    } else {
                        "0"
                    }),
                );
                trailers.insert("x-completed", http::HeaderValue::from_static("yes"));
                stream.send_trailers(trailers).ok();
            } else {
                stream
                    .send_data(Bytes::from(format!("h2:{path}")), true)
                    .ok();
            }
        });
    }
}

fn proxy_config(dir: &Path, pool: &str, sites: &str, tls: bool) -> (Proxy, u16, u16, PathBuf) {
    let (port, admin) = (free_port(), free_port());
    let listen = if tls {
        let (cert, key) = self_signed(dir, "frontend", &["front.test"]);
        format!(
            "listen: {{ https: [\"127.0.0.1:{port}\"] }}\ntls: {{ certs: [{{ name: front, cert: {}, key: {} }}] }}",
            cert.display(),
            key.display()
        )
    } else {
        format!("listen: {{ http: [\"127.0.0.1:{port}\"] }}")
    };
    let cfg = dir.join("conduit.yaml");
    std::fs::write(&cfg, format!("server: {{ threads: 1, pid_file: {d}/pid, upgrade_sock: {d}/sock, shutdown_grace: 0s }}\n{listen}\nadmin: {{ listen: \"127.0.0.1:{admin}\" }}\nupstreams:\n{pool}\nsites:\n{sites}\n",d=dir.display())).unwrap();
    let proxy = Proxy {
        child: Command::new(test_binary())
            .args(["run", "-c"])
            .arg(&cfg)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(dir.join("stderr")).unwrap())
            .spawn()
            .unwrap(),
        dir: dir.to_path_buf(),
    };
    wait_for_port(port);
    (proxy, port, admin, cfg)
}

#[test]
fn upstream_http2_cleartext_reload_and_multiplexing() {
    let dir = test_dir("upstream-h2c");
    let origin = origin(&dir, None);
    let (proxy, port, admin, cfg) = proxy_config(
        &dir,
        &format!(
            "  app: {{ backends: [http://127.0.0.1:{}], http_version: '1.1' }}",
            origin.port
        ),
        &format!(
            "  - {{ id: app, hosts: [front.test], upstream: app }}\n  - {{ id: short, hosts: [short.test], upstream: h2c://127.0.0.1:{} }}",
            origin.port
        ),
        false,
    );
    assert_eq!(http(port, "front.test", "/", &[]).body, "h1:/");
    assert_eq!(http(port, "short.test", "/", &[]).body, "h2:/");
    for (before, after, body) in [("'1.1'", "2", "h2:/"), ("2", "'1.1'", "h1:/")] {
        let text = std::fs::read_to_string(&cfg).unwrap();
        std::fs::write(
            &cfg,
            text.replace(
                &format!("http_version: {before}"),
                &format!("http_version: {after}"),
            ),
        )
        .unwrap();
        signal(proxy.child.id(), "-HUP");
        wait_until("upstream protocol reload", Duration::from_secs(5), || {
            let status = status_json(admin);
            status["upstreams"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == "app")
                .unwrap()["backends"][0]["http_version"]
                == after.trim_matches('\'')
        });
        assert_eq!(http(port, "front.test", "/", &[]).body, body);
    }
    // A warmed H2 connection must carry overlapping streams, not one socket per request.
    http(port, "short.test", "/", &[]);
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                http(port, "short.test", "/delay", &[])
            })
        })
        .collect();
    for worker in workers {
        assert_eq!(worker.join().unwrap().body, "h2:/delay");
    }
    assert!(origin.stats.parallel.load(Ordering::SeqCst) >= 4);
}

#[test]
fn upstream_http2_tls_negotiation_strict_and_upgrade() {
    let dir = test_dir("upstream-h2-tls");
    let h2 = origin(&dir, Some(true));
    let h1 = origin(&dir, Some(false));
    let pool = format!(
        "  both: {{ backends: [https://127.0.0.1:{}], verify_tls: false, http_version: auto }}\n  strict: {{ backends: [https://127.0.0.1:{}], verify_tls: false, http_version: '2' }}\n  fallback: {{ backends: [https://127.0.0.1:{}], verify_tls: false, http_version: auto }}\n  refused: {{ backends: [https://127.0.0.1:{}], verify_tls: false, http_version: 2 }}\n  default: {{ backends: [https://127.0.0.1:{}], verify_tls: false }}",
        h2.port, h2.port, h1.port, h1.port, h2.port
    );
    let sites = "  - { id: auto, hosts: [auto.test], upstream: both, upgrade: any }\n  - { id: strict, hosts: [strict.test], upstream: strict }\n  - { id: fallback, hosts: [fallback.test], upstream: fallback }\n  - { id: refused, hosts: [refused.test], upstream: refused }\n  - { id: default, hosts: [default.test], upstream: default }";
    let (_proxy, port, _, _) = proxy_config(&dir, &pool, sites, false);
    assert_eq!(http(port, "default.test", "/", &[]).body, "h1:/");
    let auto = http(port, "auto.test", "/", &[]);
    assert_eq!(auto.body, "h2:/");
    assert_eq!(auto.header("x-origin-scheme"), Some("https"));
    assert_eq!(auto.header("x-origin-authority"), Some("auto.test"));
    assert_eq!(http(port, "strict.test", "/", &[]).body, "h2:/");
    assert_eq!(http(port, "fallback.test", "/", &[]).body, "h1:/");
    let before = h1.stats.h1.load(Ordering::SeqCst);
    assert_eq!(http(port, "refused.test", "/", &[]).status, 502);
    assert_eq!(
        h1.stats.h1.load(Ordering::SeqCst),
        before,
        "required H2 must not send an H1 request"
    );
    let (mut stream, head) = open_upgrade(
        port,
        "GET /socket HTTP/1.1\r\nHost: auto.test\r\nUpgrade: echo\r\nConnection: Upgrade\r\n\r\n",
    );
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    stream.write_all(b"upgrade-echo").unwrap();
    let mut echo = [0; 12];
    stream.read_exact(&mut echo).unwrap();
    assert_eq!(&echo, b"upgrade-echo");
    assert_eq!(
        http(
            port,
            "strict.test",
            "/",
            &[("Upgrade", "websocket"), ("Connection", "Upgrade")]
        )
        .status,
        400
    );
}

#[test]
fn upstream_http2_health_cache_and_validation() {
    let dir = test_dir("upstream-h2-health");
    let origin = origin(&dir, None);
    let pool = format!(
        "  app: {{ backends: [h2c://127.0.0.1:{}], health_check: {{ path: /health, host: '::1', interval: 100ms, timeout: 1s, rise: 1 }} }}",
        origin.port
    );
    let (_proxy, port, admin, cfg) = proxy_config(
        &dir,
        &pool,
        "  - { id: app, hosts: [front.test], upstream: app, cache: true }",
        false,
    );
    wait_until("H2 health probe", Duration::from_secs(5), || {
        origin.stats.health.load(Ordering::SeqCst) > 0
    });
    assert!(
        status_json(admin)["upstreams"][0]["backends"][0]["healthy"]
            .as_bool()
            .unwrap()
    );
    let response = http(port, "front.test", "/cached", &[]);
    assert_eq!(response.body, "h2:/cached");
    assert_eq!(response.header("x-origin-scheme"), Some("http"));
    assert_eq!(response.header("x-origin-authority"), Some("front.test"));
    assert_eq!(
        http(port, "front.test", "/cached", &[]).header("cache-status"),
        Some("conduit; hit")
    );
    assert_eq!(origin.stats.cached.load(Ordering::SeqCst), 1);
    let valid = std::fs::read_to_string(&cfg).unwrap();
    for bad in [
        "http_version: 3",
        "http2_max_streams: 0",
        "http_version: '1.1'",
    ] {
        let text = valid.replace("health_check:", &format!("{bad}, health_check:"));
        std::fs::write(&cfg, text).unwrap();
        let output = Command::new(test_binary())
            .args(["check", "-c"])
            .arg(&cfg)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{bad}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    std::fs::write(cfg, valid).unwrap();
}

#[test]
fn upstream_http2_grpc_bidirectional_stream_and_trailers() {
    let dir = test_dir("upstream-h2-grpc");
    let origin = origin(&dir, None);
    let (proxy, port, admin, _) = proxy_config(
        &dir,
        &format!(
            "  grpc: {{ backends: [h2c://127.0.0.1:{}], http2_max_streams: 16 }}",
            origin.port
        ),
        "  - { id: grpc, hosts: [front.test], upstream: grpc }",
        true,
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .unwrap();
                let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
                builder.set_verify(SslVerifyMode::NONE);
                builder.set_alpn_protos(b"\x02h2").unwrap();
                let ssl = builder
                    .build()
                    .configure()
                    .unwrap()
                    .verify_hostname(false)
                    .into_ssl("front.test")
                    .unwrap();
                let mut tls = tokio_openssl::SslStream::new(ssl, tcp).unwrap();
                Pin::new(&mut tls).connect().await.unwrap();
                assert_eq!(tls.ssl().selected_alpn_protocol(), Some(b"h2".as_slice()));
                let (mut sender, connection) = h2::client::handshake(tls).await.unwrap();
                let driver = tokio::spawn(connection);
                let req = http::Request::builder()
                    .method("POST")
                    .uri(format!("https://front.test:{port}/grpc/Echo"))
                    .header("content-type", "application/grpc")
                    .header("te", "trailers")
                    .body(())
                    .unwrap();
                let (response, mut send) = sender.send_request(req, false).unwrap();
                let first = Bytes::from_static(b"\0\0\0\0\x05first");
                send.send_data(first.clone(), false).unwrap();
                let response = response.await.unwrap();
                assert_eq!(response.status(), 200);
                assert_eq!(response.headers()["x-upstream-version"], "2");
                assert_eq!(response.headers()["x-origin-scheme"], "http");
                let mut body = response.into_body();
                let echoed = body.data().await.unwrap().unwrap();
                assert_eq!(echoed, first, "response must arrive before request EOF");
                body.flow_control().release_capacity(echoed.len()).unwrap();
                let reloads = || {
                    metric(
                        &http(admin, "localhost", "/metrics", &[]).body,
                        "conduit_config_reloads_total",
                        &["result=\"success\""],
                    )
                    .unwrap()
                };
                let before = reloads();
                signal(proxy.child.id(), "-HUP");
                wait_until("reload during gRPC stream", Duration::from_secs(5), || {
                    reloads() > before
                });
                let mut frame = vec![0];
                frame.extend_from_slice(&(2_097_152u32).to_be_bytes());
                frame.resize(2_097_157, b'z');
                let last = Bytes::from(frame);
                send.send_data(last.clone(), true).unwrap();
                let mut received = Vec::new();
                while let Some(data) = body.data().await {
                    let data = data.unwrap();
                    body.flow_control().release_capacity(data.len()).unwrap();
                    received.extend_from_slice(&data);
                }
                assert_eq!(received, last);
                let trailers = body.trailers().await.unwrap().unwrap();
                assert_eq!(trailers["grpc-status"], "0");
                assert_eq!(trailers["x-completed"], "yes");
                let req = http::Request::builder()
                    .method("POST")
                    .uri(format!("https://front.test:{port}/grpc/Denied"))
                    .header("content-type", "application/grpc")
                    .body(())
                    .unwrap();
                let (response, _) = sender.send_request(req, true).unwrap();
                let response = response.await.unwrap();
                assert_eq!(response.status(), 200);
                let mut body = response.into_body();
                assert!(body.data().await.is_none());
                assert_eq!(body.trailers().await.unwrap().unwrap()["grpc-status"], "7");
                let req = http::Request::builder()
                    .method("POST")
                    .uri(format!("https://front.test:{port}/grpc/Cancel"))
                    .header("content-type", "application/grpc")
                    .body(())
                    .unwrap();
                let (response, mut stream) = sender.send_request(req, false).unwrap();
                let response = response.await.unwrap();
                stream.send_reset(h2::Reason::CANCEL);
                drop(response);
                let req = http::Request::builder()
                    .uri(format!("https://front.test:{port}/after-cancel"))
                    .body(())
                    .unwrap();
                let (response, _) = sender.send_request(req, true).unwrap();
                let response = response.await.unwrap();
                assert_eq!(response.status(), 200);
                let mut body = response.into_body();
                let mut bytes = Vec::new();
                while let Some(data) = body.data().await {
                    let data = data.unwrap();
                    body.flow_control().release_capacity(data.len()).unwrap();
                    bytes.extend_from_slice(&data);
                }
                assert_eq!(bytes, b"h2:/after-cancel");
                drop(sender);
                driver.abort();
            })
            .await
            .expect("bidirectional gRPC proxy stalled");
        });
}

#[test]
fn upstream_http2_stream_limit_and_reload() {
    let dir = test_dir("upstream-h2-stream-limit");
    let origin = origin(&dir, None);
    let (proxy, port, admin, cfg) = proxy_config(
        &dir,
        &format!(
            "  app: {{ backends: [h2c://127.0.0.1:{}], http2_max_streams: 1 }}",
            origin.port
        ),
        "  - { id: app, hosts: [front.test], upstream: app }",
        false,
    );
    let burst = || {
        http(port, "front.test", "/", &[]);
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let workers: Vec<_> = (0..16)
            .map(|_| {
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    http(port, "front.test", "/delay", &[])
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap().body, "h2:/delay");
        }
    };
    burst();
    assert_eq!(origin.stats.parallel.load(Ordering::SeqCst), 1);
    let reloads = || {
        metric(
            &http(admin, "localhost", "/metrics", &[]).body,
            "conduit_config_reloads_total",
            &["result=\"success\""],
        )
        .unwrap()
    };
    let before = reloads();
    std::fs::write(
        &cfg,
        std::fs::read_to_string(&cfg)
            .unwrap()
            .replace("http2_max_streams: 1", "http2_max_streams: 4"),
    )
    .unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until("stream-limit reload", Duration::from_secs(5), || {
        reloads() > before
    });
    burst();
    let parallel = origin.stats.parallel.load(Ordering::SeqCst);
    assert!(
        (2..=4).contains(&parallel),
        "per-connection concurrency was {parallel}"
    );
}

#[test]
#[ignore = "two-minute HTTP/2 load/reload/idle-connection stability run; needs oha"]
fn upstream_http2_load_stability() {
    let dir = test_dir("upstream-h2-stability");
    let origin = origin(&dir, Some(true));
    let (proxy, port, admin, cfg) = proxy_config(
        &dir,
        &format!(
            "  app: {{ backends: [https://127.0.0.1:{}], verify_tls: false, http_version: 2, http2_max_streams: 32, health_check: {{ path: /health, interval: 250ms, timeout: 1s, rise: 1 }} }}",
            origin.port
        ),
        "  - { id: app, hosts: [front.test], upstream: app }",
        false,
    );
    let text = format!(
        "{}\ndefaults: {{ upstream_idle_timeout: 1s }}\n",
        std::fs::read_to_string(&cfg).unwrap()
    );
    std::fs::write(&cfg, &text).unwrap();
    signal(proxy.child.id(), "-HUP");
    wait_until("short idle timeout loaded", Duration::from_secs(5), || {
        status_json(admin)["config"]["last_reload"]["ok"] == true
            && origin.stats.health.load(Ordering::SeqCst) > 0
    });
    let fds = || {
        std::fs::read_dir(format!("/proc/{}/fd", proxy.child.id()))
            .unwrap()
            .count()
    };
    let rss = || {
        let text = std::fs::read_to_string(format!("/proc/{}/status", proxy.child.id())).unwrap();
        text.lines()
            .find(|l| l.starts_with("VmRSS:"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<usize>()
            .unwrap()
    };
    let baseline_fds = fds();
    let oha = std::env::var_os("OHA_BIN").unwrap_or_else(|| "oha".into());
    let output = dir.join("load.json");
    let mut load = Command::new(oha)
        .args([
            "--no-tui",
            "--output-format",
            "json",
            "--disable-compression",
            "-z",
            "120s",
            "-w",
            "-c",
            "32",
            "-q",
            "1500",
            "--latency-correction",
            "--host",
            "front.test",
            "-o",
        ])
        .arg(&output)
        .arg(format!("http://127.0.0.1:{port}/"))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut next_reload = Duration::from_secs(5);
    let mut reloads = 0;
    let mut samples = Vec::new();
    while start.elapsed() < Duration::from_secs(120) {
        assert!(
            load.try_wait().unwrap().is_none(),
            "load generator exited early"
        );
        samples.push(serde_json::json!({"seconds": start.elapsed().as_secs(), "rss_kib": rss(), "fds": fds()}));
        if start.elapsed() >= next_reload {
            reloads += 1;
            let cap = if reloads % 2 == 0 { 32 } else { 64 };
            std::fs::write(
                &cfg,
                text.replace(
                    "http2_max_streams: 32",
                    &format!("http2_max_streams: {cap}"),
                ),
            )
            .unwrap();
            signal(proxy.child.id(), "-HUP");
            next_reload += Duration::from_secs(5);
        }
        assert!(status_json(admin)["config"]["last_reload"]["ok"] == true);
        thread::sleep(Duration::from_secs(1));
    }
    assert!(load.wait().unwrap().success());
    let results: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(
        results["errorDistribution"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        0
    );
    assert!(
        results["statusCodeDistribution"]
            .as_object()
            .unwrap()
            .keys()
            .all(|code| code == "200")
    );
    let requests = results["statusCodeDistribution"]["200"].as_u64().unwrap();
    assert!(requests >= 150_000, "only {requests} requests completed");
    thread::sleep(Duration::from_secs(3));
    let final_fds = fds();
    assert!(
        final_fds <= baseline_fds + 8,
        "idle sockets retained: before {baseline_fds}, after {final_fds}"
    );
    let warmed: Vec<_> = samples
        .iter()
        .filter(|s| s["seconds"].as_u64().unwrap() >= 10)
        .collect();
    let initial_rss = warmed[0]["rss_kib"].as_u64().unwrap();
    let final_rss = warmed.last().unwrap()["rss_kib"].as_u64().unwrap();
    assert!(
        final_rss < initial_rss + 32 * 1024,
        "RSS grew from {initial_rss} to {final_rss} KiB"
    );
    let metrics = http(admin, "localhost", "/metrics", &[]).body;
    assert_eq!(
        metric(
            &metrics,
            "conduit_config_reloads_total",
            &["result=\"failure\""]
        )
        .unwrap_or(0.0),
        0.0
    );
    assert!(
        metric(
            &metrics,
            "conduit_config_reloads_total",
            &["result=\"success\""]
        )
        .unwrap()
            >= reloads as f64
    );
    let report = serde_json::json!({"passed": true, "target_seconds": 120, "elapsed_seconds": start.elapsed().as_secs_f64(), "requests": requests, "reloads_sent": reloads, "baseline_fds": baseline_fds, "final_fds": final_fds, "initial_rss_kib": initial_rss, "final_rss_kib": final_rss, "load": results, "samples": samples});
    if let Some(path) = std::env::var_os("CONDUIT_H2_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    eprintln!(
        "H2 stability: {requests} requests, {reloads} reloads, FDs {baseline_fds} -> {final_fds}, RSS {initial_rss} -> {final_rss} KiB"
    );
}
