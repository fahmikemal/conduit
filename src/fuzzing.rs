//! Entry points for the cargo-fuzz targets in `fuzz/`. Each takes arbitrary bytes, must not
//! panic, and checks properties the code promises. Not an API; kept in the library so the
//! targets reach crate-private functions.

use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr};
use std::sync::LazyLock;
use std::time::{Duration, UNIX_EPOCH};

use http::{HeaderName, HeaderValue, header};
use pingora::http::RequestHeader;

use crate::accesslog::Entry;
use crate::config::{self, Config, UpstreamRef};
use crate::files::{self, Accepted};
use crate::plugin::Step;
use crate::proxy::{self, Conn};
use crate::runtime::{self, Runtime};
use crate::{auth, pathnorm};

/// The YAML config parser, then compiling what it accepts.
pub fn config(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(cfg) = config::parse(text) else {
        return;
    };
    // Compiling reads the files the config names and resolves backend names: keep it to
    // inputs that touch neither special files nor DNS.
    let special = ["/dev", "/proc", "/sys", "/run"]
        .iter()
        .any(|p| text.contains(p));
    if !special && ip_backends_only(&cfg) {
        let _ = runtime::build(&cfg, None, None);
    }
}

fn ip_backends_only(cfg: &Config) -> bool {
    let literal = |url: &str| {
        let rest = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))
            .unwrap_or(url)
            .trim_end_matches('/');
        rest.parse::<SocketAddr>().is_ok()
            || rest.parse::<IpAddr>().is_ok()
            || rest
                .strip_prefix('[')
                .and_then(|r| r.strip_suffix(']'))
                .is_some_and(|r| r.parse::<IpAddr>().is_ok())
    };
    let pools = cfg
        .upstreams
        .values()
        .chain(cfg.sites.iter().filter_map(|s| match &s.upstream {
            Some(UpstreamRef::Inline(p)) => Some(&**p),
            _ => None,
        }));
    let mut ok = true;
    for p in pools {
        ok &= p.backends.iter().all(|b| literal(&b.url));
    }
    ok && cfg.sites.iter().all(|s| match &s.upstream {
        Some(UpstreamRef::Named(n)) => cfg.upstreams.contains_key(n) || literal(n),
        _ => true,
    })
}

/// Path normalization: the result is a clean absolute path.
pub fn path(data: &[u8]) {
    let Ok(p) = pathnorm::normalize(data) else {
        return;
    };
    assert_eq!(p.first(), Some(&b'/'), "{p:?}");
    assert!(!p.contains(&0));
    let inner = p.strip_suffix(b"/").unwrap_or(&p);
    for seg in inner.split(|b| *b == b'/').skip(1) {
        assert!(
            !seg.is_empty() && seg != b"." && seg != b"..",
            "segment {seg:?} left in {p:?} from {data:?}"
        );
    }
}

/// A `proxy` rule's upstream target, from any path a request may have, is a valid origin-form
/// target: a '/', then path characters and escapes, then the query as it came.
pub fn route(data: &[u8]) {
    use crate::config::{ProxyConfig, RewriteConfig};
    use crate::route::Route;
    static ROUTES: LazyLock<Vec<Route>> = LazyLock::new(|| {
        let mk = |strip: Option<&str>, add: Option<&str>, rw: Option<(&str, &str)>| {
            let c = ProxyConfig {
                upstream: None,
                strip_prefix: strip.map(String::from),
                add_prefix: add.map(String::from),
                rewrite: rw.map(|(regex, to)| RewriteConfig {
                    regex: regex.into(),
                    to: to.into(),
                }),
            };
            Route::compile("fuzz", &c, None).expect("valid route")
        };
        vec![
            mk(Some("/api"), None, None),
            mk(Some("/a/"), Some("/v 2/"), None),
            mk(None, Some("/"), None),
            mk(None, None, Some(("^/(?P<x>[^/]*)/(.*)$", "/$2/${x}/ü"))),
            mk(None, None, Some(("[aeiou]+", ""))),
        ]
    });
    let (raw_path, query) = match data.iter().position(|b| *b == b'?') {
        Some(i) => (&data[..i], Some(&data[i + 1..])),
        None => (data, None),
    };
    let Ok(path) = pathnorm::normalize(raw_path) else {
        return;
    };
    for r in ROUTES.iter() {
        let t = r.target(&path, query);
        let (p, q) = match t.iter().position(|b| *b == b'?') {
            Some(i) => (&t[..i], Some(&t[i + 1..])),
            None => (&t[..], None),
        };
        assert_eq!(p.first(), Some(&b'/'), "{t:?}");
        assert_eq!(q, query, "{t:?}");
        let mut i = 0;
        while i < p.len() {
            let b = p[i];
            if b == b'%' {
                assert!(
                    p.len() > i + 2 && p[i + 1].is_ascii_hexdigit() && p[i + 2].is_ascii_hexdigit(),
                    "{t:?}"
                );
                i += 3;
                continue;
            }
            assert!(
                b.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&b),
                "{b:?} in {t:?}"
            );
            i += 1;
        }
        // A query Pingora refuses never arrives (the request is answered 400); with one it
        // took, the target sent upstream is taken too.
        let arrives = query.is_none_or(|q| {
            RequestHeader::build("GET", &[b"/?".as_slice(), q].concat(), None).is_ok()
        });
        if arrives {
            let mut up = RequestHeader::build("GET", b"/", None).expect("request");
            up.set_raw_path(&t).expect("Pingora takes the target");
        }
    }
}

/// `block_exploits` on any input taken as a query, a path, a user agent and a header: no
/// panic, and the needles that pick the rules to run never hide a match of a rule.
pub fn block_exploits(data: &[u8]) {
    crate::exploits::fuzz_check(data);
}

/// `body_replace` streaming: however the body is cut into chunks, the output is what the
/// whole body gives.
///
/// Input: a count byte, then that many `from` and `to` strings (a length byte each, then the
/// bytes), a byte choosing chunk sizes, then the body.
pub fn body_replace(data: &[u8]) {
    use crate::config::{BodyReplaceConfig, ReplacePair};
    use crate::replace::{BodyReplace, Stream};
    use std::sync::Arc;
    let Some((&count, mut rest)) = data.split_first() else {
        return;
    };
    let mut string = || -> Option<String> {
        let (&n, after) = rest.split_first()?;
        let n = (n as usize % 9).min(after.len());
        let (s, after) = after.split_at(n);
        rest = after;
        String::from_utf8(s.to_vec()).ok()
    };
    let mut pairs = Vec::new();
    for _ in 0..count % 4 + 1 {
        let (Some(from), Some(to)) = (string(), string()) else {
            return;
        };
        if from.is_empty() {
            return;
        }
        pairs.push(ReplacePair { from, to });
    }
    let Some((&sizes, body)) = rest.split_first() else {
        return;
    };
    let r = Arc::new(
        BodyReplace::new(&BodyReplaceConfig {
            replace: pairs,
            types: vec!["*".into()],
            matcher: None,
        })
        .expect("valid body_replace"),
    );
    let whole = Stream::new(r.clone()).feed(body, true);
    let mut s = Stream::new(r);
    let mut got = Vec::new();
    let (mut at, mut k) = (0, 0u32);
    while at < body.len() {
        // 1 to 7 bytes, varying.
        let n = (1 + ((sizes as u32).rotate_left(k) % 7) as usize).min(body.len() - at);
        k += 3;
        got.extend(s.feed(&body[at..at + n], false));
        at += n;
    }
    got.extend(s.feed(b"", true));
    assert_eq!(got, whole, "{body:?}");
}

/// The hand-written access log JSON matches serde's, stays on one line, and parses.
pub fn access_log(data: &[u8]) {
    let mut parts = data.split(|b| *b == 0xfe);
    let mut next = || String::from_utf8_lossy(parts.next().unwrap_or_default());
    let (request_id, host, method, uri, ua, referer, upstream, error) = (
        next(),
        next(),
        next(),
        next(),
        next(),
        next(),
        next(),
        next(),
    );
    let n = |i: usize| data.get(i).copied().unwrap_or(0) as u64;
    let micros = n(0) << 32 | n(1) << 24 | n(2) << 16 | n(3) << 8 | n(4);
    let client = match n(5) % 3 {
        0 => None,
        1 => Some(IpAddr::from([
            n(6) as u8,
            n(7) as u8,
            n(8) as u8,
            n(9) as u8,
        ])),
        _ => Some(IpAddr::from([
            n(6) as u16 * 257,
            0,
            0,
            0,
            0,
            0,
            n(7) as u16,
            n(8) as u16,
        ])),
    };
    let e = Entry {
        ts: UNIX_EPOCH + Duration::from_micros(micros.wrapping_mul(1_000_003)),
        request_id: &request_id,
        site: (n(10) & 1 == 1).then_some(&*host),
        host: &host,
        client,
        method: &method,
        scheme: if n(11) & 1 == 1 { "https" } else { "http" },
        uri: Cow::Borrowed(&uri),
        proto: "HTTP/1.1",
        status: (n(12) << 8 | n(13)) as u16,
        bytes_sent: micros as usize,
        bytes_received: micros.rotate_left(17),
        duration_ms: Duration::from_nanos(micros.wrapping_mul(7919)),
        upstream: (n(14) & 1 == 1).then_some(&*upstream),
        upstream_status: (n(14) & 2 == 2).then_some(n(15) as u16 * 3),
        upstream_connect_ms: (n(14) & 4 == 4).then(|| Duration::from_nanos(micros)),
        upstream_header_ms: (n(14) & 8 == 8).then(|| Duration::from_micros(micros)),
        retries: n(16) as u32,
        tls: (n(14) & 16 == 16).then_some("TLSv1.3"),
        user_agent: Cow::Borrowed(&ua),
        referer: Cow::Borrowed(&referer),
        error: (n(14) & 32 == 32).then_some(&*error),
        blocked_by: (n(14) & 64 == 64).then_some("crowdsec"),
        cache: (n(14) & 128 == 128).then_some("hit"),
    };
    let mut out = Vec::new();
    e.write_json(&mut out);
    let line = String::from_utf8(out).expect("access log line is UTF-8");
    assert!(!line.contains('\n'), "{line}");
    assert_eq!(line, serde_json::to_string(&e).unwrap());
}

const ROUTES: &str = r#"
listen:
  http: ["127.0.0.1:8080"]
real_ip:
  header: X-Forwarded-For
  trusted: [127.0.0.0/8, "::1/128"]
  recursive: true
upstreams:
  web:
    backends: [http://10.0.0.1:80, http://10.0.0.2:80]
    balance: hash
    hash_key: "header:x-user"
  sticky:
    backends: [http://10.0.0.3:80, { url: "http://10.0.0.4:80", weight: 3 }]
    balance: hash
    hash_key: "cookie:session"
sites:
  - id: app
    hosts: [app.example.com, "*.app.example.com"]
    upstream: web
    force_https: true
    max_body_size: 1m
  - id: git
    hosts: [git.example.com]
    upstream: http://10.0.0.20:3000
    plugins:
      - rules:
          - match:
              method: [GET, HEAD]
              headers: { user-agent: { regex: "(?i)mozilla" } }
            action: { redirect: { status: 302, to: "https://git.example.org{request_uri}" } }
          - match: { path: { prefix: /old/ } }
            action: { redirect: { to: "https://{host}{path}?moved=1" } }
  - id: pb
    hosts: [pb.example.com]
    upstream:
      backends: [http://10.0.0.26:8090]
      balance: hash
      hash_key: client_ip
    plugins:
      - block_exploits:
          skip_params: [q]
          except: [{ match: { path: { prefix: /search } }, categories: [xss, sqli] }]
      - ip_filter: { deny: [192.0.2.0/24] }
      - rate_limit: { requests: 1000000, per: 1s }
      - concurrency: { max: 1000000 }
      - rules:
          - match: { path: { regex: "^/_(/|$)" } }
            action: { deny: 403 }
          - match: { path: { exact: /wp-content/debug.log } }
            action: { deny: 404 }
          - match: { query: { regex: "(?i)debug=1" } }
            action: { respond: { status: 400, headers: { Content-Type: text/plain }, body: "no\n" } }
          - match: { headers: { x-middleware-subrequest: { present: true } } }
            action: { deny: 403 }
  - id: sso
    hosts: [sso.example.com]
    upstream:
      backends: [http://10.0.0.28:3000]
      balance: hash
      hash_key: uri
    plugins:
      - forward_auth: { url: "http://127.0.0.1:9091/auth", copy_headers: [Remote-User] }
  - id: shop
    hosts: [shop.example.com]
    upstream: sticky
    plugins:
      - rules:
          - match: { path: { prefix: /api/ } }
            action: { proxy: { upstream: web, strip_prefix: /api, add_prefix: /v2 } }
          - match: { headers: { x-beta: { present: true } } }
            action: { proxy: { upstream: "http://10.0.0.30:80", rewrite: { regex: "^/([^/]*)(.*)$", to: "/beta${2}/${1}" } } }
          - match: { path: { prefix: /admin } }
            action: { deny: 403 }
      - block_exploits: { mode: log, categories: [traversal, dotfiles, sqli, xss, php, rce, ssrf, scanners, spam] }
      - body_replace:
          match: { path: { prefix: /api/ } }
          replace: [{ from: "http://10.0.0.1", to: "https://shop.example.com" }]
"#;

static ROUTER: LazyLock<Runtime> = LazyLock::new(|| {
    let cfg = config::parse(ROUTES).expect("fuzz routes");
    runtime::build(&cfg, None, None)
        .expect("fuzz runtime")
        .runtime
});

/// A request head as a client could send it, through everything conduit reads from it
/// before the upstream: host, routing, real IP, plugins, rules, hashing, header parsers.
///
/// Input: one flag byte (TLS, peer, HTTP version), then `METHOD target` and header lines
/// separated by `\n`.
pub fn request(data: &[u8]) {
    let Some((&flags, rest)) = data.split_first() else {
        return;
    };
    let mut lines = rest.split(|b| *b == b'\n');
    let start = lines.next().unwrap_or_default();
    let (method, target) = match start.iter().position(|b| *b == b' ') {
        Some(i) => (&start[..i], &start[i + 1..]),
        None => (&b"GET"[..], start),
    };
    let Ok(method) = http::Method::from_bytes(method) else {
        return;
    };
    let Ok(mut req) = RequestHeader::build(method, target, None) else {
        return;
    };
    req.set_version(match flags >> 4 & 3 {
        0 => http::Version::HTTP_10,
        1 => http::Version::HTTP_2,
        _ => http::Version::HTTP_11,
    });
    for line in lines {
        let Some(i) = line.iter().position(|b| *b == b':') else {
            continue;
        };
        let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(&line[..i]),
            HeaderValue::from_bytes(line[i + 1..].trim_ascii()),
        ) else {
            continue;
        };
        let _ = req.append_header(name, value);
    }

    let rt = &*ROUTER;
    let https = flags & 1 == 1;
    let peer: IpAddr = match flags >> 1 & 3 {
        0 => [127, 0, 0, 1].into(),
        1 => [192, 0, 2, 9].into(),
        2 => std::net::Ipv6Addr::LOCALHOST.into(),
        _ => [198, 51, 100, 7].into(),
    };

    let host = proxy::request_host(&req);
    assert!(
        host.bytes()
            .all(|b| !b.is_ascii_uppercase() && b.is_ascii_graphic()),
        "{host:?}"
    );
    let site = rt.router.lookup(&host);
    let name = site
        .and_then(|s| s.real_ip_header.as_ref())
        .unwrap_or(&rt.realip.header);
    let (client, trusted) = rt.realip.resolve(peer, req.headers.get(name));
    if !trusted {
        assert_eq!(client, peer);
    }
    let _ = proxy::incoming_request_id(&req);
    let _ = proxy::client_scheme(https, trusted, &req);
    let _ = proxy::proto(&req);
    let _ = proxy::header_str(&req, header::USER_AGENT);
    let uri = proxy::request_uri(&req);
    let pathless = uri.first() != Some(&b'/') && uri != b"*";

    let conn = Conn {
        https,
        host: &host,
        client: Some(client),
        client_cert: None,
    };
    let mut fx = crate::plugin::Effects::default();
    match proxy::decide(rt, site.map(|s| &**s), &req, &conn, &mut fx) {
        Step::Answer(local) => {
            assert!((100..600).contains(&local.status), "{}", local.status);
            assert!(
                !pathless || local.status == 400,
                "{uri:?}: {}",
                local.status
            );
        }
        Step::Pass | Step::Check(_) => {
            assert!(!pathless, "{uri:?} went through");
            // The plugins already normalized this path, so a route's target exists.
            if let Some(t) = proxy::routed_target(&fx, &req) {
                let t = t.expect("normalized once already");
                assert_eq!(t.first(), Some(&b'/'), "{t:?}");
                // The request itself was taken, query included.
                let mut up = RequestHeader::build("GET", b"/", None).expect("request");
                up.set_raw_path(&t).expect("Pingora takes the target");
            }
            if let Some(pool) = fx.route.as_ref().and_then(|r| r.pool.as_ref()) {
                let _ = pool.select(&pool.key(&req, Some(client)), &[]);
            }
        }
    }
    drop(fx);

    if let Some(pool) = site.and_then(|s| s.pool.as_ref()) {
        let key = pool.key(&req, Some(client));
        let _ = pool.select(&key, &[]);
    }

    let _ = Accepted::from_request(&req);
    let _ = auth::credentials(&req.headers);
    if let Some(v) = req.headers.get(header::RANGE) {
        let len = u64::from(flags) * 977;
        if let Some(Some((start, end))) = files::parse_range(v.as_bytes(), len) {
            assert!(start <= end && end < len, "{start}-{end} of {len}");
        }
    }
    for name in [header::IF_NONE_MATCH, header::IF_MATCH] {
        if let Some(v) = req.headers.get(name) {
            let _ = files::etag_listed(v.as_bytes(), b"\"5f3-6a1b2c\"", true);
            let _ = files::etag_listed(v.as_bytes(), b"W/\"5f3-6a1b2c\"", false);
        }
    }
}
