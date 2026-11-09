<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/conduit-logo-horizontal-reverse.svg">
    <img alt="conduit" src="assets/brand/conduit-logo-horizontal-ember.svg" width="300">
  </picture>
</h1>

A reverse proxy built on [Pingora](https://github.com/cloudflare/pingora), for small hosts that
run many sites behind one address. It does what an Nginx Proxy Manager setup is usually used
for (routing by host, TLS with automatic certificates, WebSocket, access rules) from one YAML
file and one static binary, with hot reload and zero-downtime upgrades.

The design follows Pingora's own model (`ProxyHttp` hooks, load balancer, background services)
rather than imitating nginx.

> **Status:** pre-1.0. Feature work is done and the code is tested hard locally (unit,
> end-to-end, fuzzing, parity against NPM, a 24-hour soak). It has not run production traffic
> yet. Expect configuration changes before 1.0.

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
- An optional `Server` header without a version. Off by default, and then no `Server` header is
  sent at all, not even Pingora's own.

**Upstreams**
- Named pools with weighted backends; DNS names resolve to several backends.
- `round_robin`, `random`, `least_conn`, and consistent `hash` (client IP, URI, path, header,
  cookie).
- Active HTTP/TCP health checks and passive ejection. Retries go to other backends.
- Pools and their health state survive reloads.

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
- `conduit import-npm` converts an Nginx Proxy Manager installation into a conduit config.

## Performance

Measured on 2 CPUs against nginx with a tuned config (PGO release build, idle machine):

| Scenario | Requests/s vs nginx | CPU per request vs nginx |
|---|---:|---:|
| HTTP/2 | 1.29–1.32× | 0.80–0.82× |
| New TLS handshake per request | 1.10× | 0.86–0.89× |
| HTTPS keep-alive (HTTP/1.1) | 0.77–0.78× | 1.34–1.37× |
| HTTP/1.1 keep-alive | 0.76–0.81× | 1.31–1.38× |

Idle memory is about 26 MB with 100 sites. At half of peak load, p99 latency rises by about
0.5 ms. Methods, more numbers, and a comparison with HAProxy, Envoy, Caddy, Traefik and Apache
are in the [technical documentation](docs/teknis.md#benchmark).

## Getting started

Requirements: Linux, Rust 1.88 or newer (developed on current stable), a C toolchain, `cmake`, `perl` and `make`. OpenSSL
and zlib-ng are built from source and linked statically.

```sh
git clone --recurse-submodules https://github.com/fahmikemal/conduit.git
cd conduit
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
conduit import-npm --npm-root ./npm-copy --paths-root /opt/npm > conduit.yaml   # from a copy of /opt/npm
```

Every option is described in [`conduit.example.yaml`](conduit.example.yaml). A systemd unit and
a logrotate file are in [`deploy/`](deploy/), and the CrowdSec parser is in
[`deploy/crowdsec/`](deploy/crowdsec/).

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
graceful upgrades with open WebSockets, upstream flapping, slow and malformed clients, and
running out of file descriptors.

## Pingora fork

conduit builds against Pingora 0.9.1 with a small set of patches. The fork is a git submodule
in [`vendor/pingora`](vendor/README.md) (branch `conduit`, one commit per patch) and is wired in
through `[patch.crates-io]`. Together with conduit-side work, the patches cut CPU per HTTP/1.1 request by about a
quarter. One bounds
the time for a whole request header (slowloris), and one makes the `Server` header of Pingora's
own error responses configurable. Each patch and its measured effect is listed in
[`vendor/README.md`](vendor/README.md).

## Documentation

- [Technical documentation](docs/teknis.md) (Indonesian): every feature in detail, test
  results, benchmarks and their method, profiling notes, and the roadmap.
- [`conduit.example.yaml`](conduit.example.yaml): every configuration option.
- [`vendor/README.md`](vendor/README.md) (Indonesian): the Pingora patches.

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for
attribution, including Pingora (Copyright Cloudflare, Inc., Apache-2.0).
