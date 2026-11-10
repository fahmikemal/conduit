<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/conduit-logo-horizontal-reverse.svg">
    <img alt="conduit" src="assets/brand/conduit-logo-horizontal-ember.svg" width="300">
  </picture>
</h1>

A reverse proxy built on [Pingora](https://github.com/cloudflare/pingora) for running many sites
behind one address: routing by host name, TLS with automatic certificates, WebSocket, and access
rules, from one YAML file and one static binary, with hot reload and zero-downtime upgrades.

The design follows Pingora's own model (`ProxyHttp` hooks, load balancer, background services)
rather than imitating nginx.

> **Status:** pre-1.0. The 10 November 2026 validation passed 111 unit tests and all 34
> end-to-end tests, including HTTP/2 upstream and the audit regressions. See the
> [validation evidence](docs/audit/HTTP2_UPSTREAM_VALIDATION.md). Earlier fuzzing, parity and
> 24-hour soak results apply to their documented builds; the current Krynox run is separate.
> Expect configuration changes before 1.0.

## Features

**Routing and TLS**
- Routing by host name (exact and `*.domain`), many hosts per site.
- TLS with certificate selection per SNI, HTTP/2 to clients, certificates reloaded when the files
  change.
- Built-in ACME (HTTP-01, ARI renewal windows); no certbot.
- Mutual TLS per site, with the client certificate passed to the upstream.
- Force HTTPS, HSTS, body size limits and timeouts per site.
- WebSocket and other protocol upgrades (`upgrade: websocket | any | off`). `any` covers
  non-WebSocket upgrades such as headscale/Tailscale.
- Compression of text responses the upstream left uncompressed.
- A `Server: conduit` header without a version, on by default. With `server_header: off`,
  conduit and Pingora add none; an upstream's own `Server` header passes through. Use the
  `headers` plugin to remove it too.

**Upstreams**
- Named pools with weighted backends; DNS names resolve to several backends.
- `round_robin`, `random`, `least_conn`, and consistent `hash` (client IP, URI, path, header,
  cookie).
- Active HTTP/TCP health checks and passive ejection. Retries go to other backends.
- Pools and their health state survive reloads.
- HTTP/1.1 upstreams by default; optional HTTP/2 over TLS or cleartext (`h2c://`),
  TLS negotiation with HTTP/1.1 fallback, and concurrent HTTP/2 streams for gRPC.

**Per-site plugins**, run in order
- `rules`: match method, path, query, header or scheme. Actions: deny, redirect, respond, serve
  a file, or `proxy` to another upstream with `strip_prefix`, `add_prefix` or a regex
  `rewrite`. Paths are normalized first, so encodings cannot slip past a rule.
- `block_exploits`: blocks common attack payloads (traversal, secret files, SQLi, XSS, PHP,
  Log4Shell, OGNL, SSRF to cloud metadata, scanners) and is tuned to let ordinary traffic
  through. Replayed against 1.1 million requests from a production access log, it blocks no
  legitimate request (one false positive found that way was fixed). One
  Aho-Corasick pass plus linear-time regexes, ~0.2–0.7 µs per request.
- `rate_limit` and `concurrency`, keyed by the real client IP, host or header.
- `basic_auth` (bcrypt, htpasswd files) and `forward_auth` (Authelia, Authentik,
  oauth2-proxy).
- `ip_filter`, `headers`, `body_replace` (streaming `sub_filter`).
- `crowdsec`: a CrowdSec bouncer (Local API stream) and optional AppSec checks, built in.

**Serving and caching**
- Static file server with ranges, conditional requests, precompressed files, SPA fallback, and
  `openat2(RESOLVE_BENEATH)` so nothing outside the root can be served.
- HTTP response cache (RFC 9111) on `pingora-cache`, with cache locking, stale-while-revalidate,
  stale-if-error and purge over the admin API.

**Operations**
- Reload on SIGHUP or file change without dropping connections. Graceful binary upgrades hand
  the listening sockets over (`systemctl reload`).
- Prometheus `/metrics`, a JSON `/status`, and `/healthz` on a separate admin listener.
- A JSON access log, one line per request, with upstream timings and request IDs, plus a
  CrowdSec parser for it.
- `conduit check` validates a config. Unknown keys are errors.

## Performance

Historical measurement, 8 November 2026: 2 CPUs against nginx with a tuned config (PGO
release build, idle machine). HTTP/2 here describes the client connection; upstreams were
HTTP/1.1. The final HTTP/2-upstream change has its own [A/B and soak results](docs/audit/HTTP2_UPSTREAM_VALIDATION.md#performa).

| Scenario | Requests/s vs nginx | CPU per request vs nginx |
|---|---:|---:|
| HTTP/2 | 1.29–1.32× | 0.80–0.82× |
| New TLS handshake per request | 1.10× | 0.86–0.89× |
| HTTPS keep-alive (HTTP/1.1) | 0.77–0.78× | 1.34–1.37× |
| HTTP/1.1 keep-alive | 0.76–0.81× | 1.31–1.38× |

Idle memory is about 26 MB with 100 sites. At half of peak load, p99 latency rises by about
0.5 ms. Methods, more numbers, and a comparison with HAProxy, Envoy, Caddy, Traefik and Apache
are in the [technical documentation](docs/technical.md#benchmark).

## Getting started

Requirements: Linux, Rust 1.88 or newer as declared by the manifest, a C toolchain, `cmake`,
`perl`, `patch` and `make`. The final validation used Rust 1.99.1; MSRV was not revalidated.
OpenSSL and zlib-ng are built from source and linked statically.

```sh
git clone --recurse-submodules https://github.com/fahmikemal/conduit.git
cd conduit
bash scripts/prepare-vendor.sh
cargo build --release
./target/release/conduit --help
```

A small configuration:

```yaml
listen:
  http: ["0.0.0.0:80"]
  https: ["0.0.0.0:443"]

admin:
  listen: "127.0.0.1:9180"

tls:
  acme:
    email: admin@example.com

sites:
  - id: app
    hosts: [app.example.com, www.example.com]
    upstream: http://127.0.0.1:3000
    acme: true
    force_https: true
    plugins:
      - block_exploits: {}
      - rate_limit: { requests: 20, per: 1m, match: { path: { prefix: /login } } }

  - id: api
    hosts: [api.example.com]
    upstream:
      backends: ["http://10.0.0.11:8080", "http://10.0.0.12:8080"]
      balance: least_conn
      health_check: { type: http, path: /healthz }
    acme: true
```

```sh
conduit check -c conduit.yaml        # validate
conduit run -c conduit.yaml          # foreground; -d daemonizes, -u takes over from a running process
```

Every option is described in [`conduit.example.yaml`](conduit.example.yaml), and in depth, with
examples, in the [configuration reference](docs/configuration.md). A systemd unit and
a logrotate file are in [`deploy/`](deploy/), and the CrowdSec parser is in
[`deploy/crowdsec/`](deploy/crowdsec/).

### HTTP/2 upstream

HTTP/1.1 remains the upstream default. To connect to a cleartext HTTP/2 service:

```yaml
listen:
  http: ["127.0.0.1:8080"]
sites:
  - id: internal
    hosts: [internal.example.test]
    upstream: h2c://127.0.0.1:50051
```

For TLS backends, set `http_version: "2"` on the named or inline pool to require HTTP/2,
or `http_version: auto` to allow TLS ALPN to choose HTTP/2 or HTTP/1.1. Each H2 connection
allows up to `http2_max_streams: 128`, bounded further by the backend's SETTINGS.
Native gRPC clients also need HTTP/2 over TLS to conduit; the client listener does not offer
h2c. See [upstream pools](docs/configuration.md#4-upstream-pools) for the protocol matrix,
health checks, reloads and WebSocket behavior.

## Development

```sh
mise run test           # unit and end-to-end tests
mise run lint           # rustfmt and clippy -D warnings
mise run test-acme      # ACME against Pebble (Docker)
mise run test-crowdsec  # against a real CrowdSec (Docker)
mise run bench          # conduit vs nginx, with a regression gate (Docker)
mise run release        # profile-guided release build for glibc 2.35 (Docker)
fuzz/run.sh [SECONDS]   # all cargo-fuzz targets (nightly)
```

The end-to-end tests run the real binary. They cover reloads under load, certificate rotation,
graceful upgrades with open WebSockets, upstream flapping, slow and malformed clients, file
containment, cache variants, concurrent admission, HTTP/2 multiplexing and gRPC streams.

`mise run test` runs the default suite. ACME, CrowdSec, slowloris and the 120-second H2 soak
are ignored by default and must be selected explicitly. For a native Cargo checkout, prepare
the vendor patch first, including after resetting the submodule:

```sh
bash scripts/prepare-vendor.sh
cargo test --locked --all-targets
cargo test --locked --lib -- --ignored --nocapture
cargo test --locked --test e2e upstream_http2_ -- --nocapture
cargo test --locked --test e2e slow_request_head_is_cut_off -- --ignored --nocapture
OHA_BIN=/path/to/oha cargo test --locked --test e2e upstream_http2_load_stability -- --ignored --nocapture
```

The two explicit unit measurements are included in the 111 total; ACME, CrowdSec,
slowloris and H2 soak bring the 30 default E2E passes to 34. Full commands and logs are in
[the HTTP/2 validation report](docs/audit/HTTP2_UPSTREAM_VALIDATION.md).

## Pingora fork

conduit builds against Pingora 0.9.1 with a small set of patches. The fork is a git submodule
in [`vendor/pingora`](vendor/README.md) (branch `conduit`, one commit per patch) and is wired in
through `[patch.crates-io]`. Together with conduit-side work, the patches cut CPU per HTTP/1.1
request by about a quarter. One bounds the time for a whole request header (slowloris), one
makes the `Server` header of Pingora's own error responses configurable, and one backports an
unreleased upstream fix that ends brotli streams properly. HTTP/2 additionally requires the
checked-in [`vendor/pingora-h2-reuse.patch`](vendor/pingora-h2-reuse.patch), applied by
`scripts/prepare-vendor.sh` over the pinned submodule. It prevents duplicate H2 pool entries
that retain idle sockets. A modified submodule after preparation is expected. Each patch, its measured effect, and
how each one stands against upstream are listed in [`vendor/README.md`](vendor/README.md).

## Documentation

- [Configuration reference](docs/configuration.md): every option with its default, how a
  request is handled step by step, match conditions, the rules engine and every plugin in
  depth, files, cache, TLS, headers, the access log, metrics, and a guide for nginx users.
- [Technical documentation](docs/technical.md): every feature in detail, test
  results, benchmarks and their method, profiling notes, and the roadmap.
- [Management and Web UI roadmap](docs/roadmap.md): planned optional Hallmark-designed UI,
  headless default, SQLite/auth, Authentik/ZITADEL OIDC, conflict-safe configuration changes
  and production release criteria. These management features are not implemented yet.
- [`conduit.example.yaml`](conduit.example.yaml): every configuration option.
- [`vendor/README.md`](vendor/README.md): the pinned Pingora fork, required local H2 pool
  backport, build preparation and historical upstream survey.
- [Audit index](docs/audit/README.md): current closure status and the historical investigation.
- [HTTP/2 validation](docs/audit/HTTP2_UPSTREAM_VALIDATION.md): the final 111 unit / 34 E2E
  results, release benchmark, local H2 soak and source/binary hashes.
- [Graphify report](graphify-out/GRAPH_REPORT.md): the shared code/documentation map;
  [interactive graph](graphify-out/graph.html).

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for
attribution, including Pingora (Copyright Cloudflare, Inc., Apache-2.0).
