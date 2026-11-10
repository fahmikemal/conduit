# conduit: technical documentation

Full documentation: features, test results, benchmarks, commands, and the roadmap. The
[README](../README.md) has a short overview. Every configuration option, the rules engine and
each plugin are described in the [configuration reference](configuration.md).

A reverse proxy built on [Pingora](https://github.com/cloudflare/pingora) (0.9.1, with a few
performance patches in [`vendor/`](../vendor/README.md)), meant for production use in front of
many sites. The design follows Pingora's own model (`ProxyHttp` hooks, load balancer, background
services), not an imitation of nginx.

**Status: pre-1.0.** Phases 1 and 2 and the historical phase 3 checklist are complete.
The 10 November revision adds the audit fixes and HTTP/2 upstream, with the full local
validation described below. Routing, auth, ACME, CrowdSec, static files, response caching,
WebSocket upgrades, per-path proxy routing and streaming body replacement remain supported.
Configuration and API details can change before 1.0.

Implementation reference: `03cae2b`, 10 November 2026. The completed HTTP/2 upstream work
and audit fixes are summarized in [current local validation](#current-local-validation).
The older phase, fuzzing, parity and 24-hour soak results below retain their original build
scope. They do not validate the separate Krynox run or imply this revision was deployed.

## Features

**Routing and TLS**
- Routing by host (exact and `*.domain`); one site can have many hosts.
- TLS with certificate selection per SNI (site override → exact SAN → wildcard → default),
  HTTP/2 to clients, certificates reloaded when their files change.
- Upstream pools default to HTTP/1.1 and can select required HTTP/2, TLS auto negotiation,
  or cleartext HTTP/2 prior knowledge (`h2c://`). HTTP/2 connections multiplex streams;
  gRPC streaming and response trailers are forwarded. See the configuration reference for
  `http_version` and `http2_max_streams`.
- Force HTTPS, HSTS, body size limits, timeouts per site, a default page for unknown hosts, an
  ACME webroot.
- HTTP/1.1 clients that sit idle between requests are disconnected after `keepalive_timeout`
  (default 75 seconds, as in nginx). A client's `Keep-Alive: timeout=` is not followed; without
  this, Pingora keeps idle connections open forever. A request header must be complete within
  60 seconds of its first byte, so slowloris clients cannot hold connections (Pingora patch
  `7ffe1c4`). Headers up to 1 MiB and 256 lines are accepted; anything larger is refused with a
  4xx. A target without a path (`GET ?x`) is answered with 400, as nginx does, because Pingora
  passes it through as is.
- Level-1 compression (the first algorithm in `Accept-Encoding`: gzip, br, or zstd) for whole
  text responses with status 200/403/404 that the upstream left uncompressed. 206 responses are
  never compressed, because their byte ranges refer to the original content. Brotli streams
  are finished properly (upstream backport `be20dfc`; Pingora 0.9.1 sent brotli without its
  final block).
- A `Server` header (`defaults.server_header`, default `conduit`), sent exactly as written,
  without a version number, on every response: proxied responses (replacing the upstream's
  `Server`), local answers (403, 404, redirects), 502/504 error pages, and responses Pingora
  writes itself (400 for a broken header, 405 for `CONNECT`; Pingora patch `499f89d`). Any other
  value can be set, for example `server_header: edge`. With `server_header: off` (or `false`),
  conduit and Pingora add no `Server` header, including on those local error responses;
  an upstream's own `Server` passes through unchanged. A site can drop it with
  `headers: { response: { remove: [server] } }`. It changes on reload. For Wappalyzer to detect
  conduit, a `Server: ^conduit$` fingerprint still has to be added to their database.

**WebSocket and protocol upgrades** (`upgrade:` and `upgrade_idle_timeout:` per site)
- `upgrade: websocket` (default): only WebSocket handshakes are passed on, with hop-by-hop
  headers normalized by Pingora. Other upgrades reach the upstream as ordinary requests.
- `upgrade: any`: every HTTP/1.1 `Upgrade` is passed on with its handshake headers as they are
  (minus `Keep-Alive`, `TE`, `Proxy-Connection`, as nginx does). This is what headscale needs:
  Tailscale clients use `Upgrade: tailscale-control-protocol`, DERP uses `Upgrade: DERP`. It
  matches the usual nginx setup (`proxy_set_header Upgrade $http_upgrade` and `Connection`).
- `upgrade: off`: no upgrades; the upstream sees an ordinary request.
- The stream after a 101 is not a request body, so `max_body_size` does not apply to it. Only
  a handshake that actually completed counts: an `Upgrade` header the upstream refused does not
  lift the body limit.
- An upgraded connection is closed after `upgrade_idle_timeout` without data in either
  direction (default: the site's `read_timeout`, 90 seconds). Without this setting Pingora uses
  a fixed 60 seconds on the client side.
- Upgrade requests are never cached. A reload (SIGHUP) does not drop open connections (tested).
  During a graceful binary upgrade the old process keeps accepting connections for about
  5 seconds (a Pingora constant), then stops after `shutdown_grace` + `shutdown_timeout`. Upgraded
  connections still open are closed with it and clients reconnect to the new process. Each one
  is still logged once in the access log, with status 101 and `"error":"Cancelled"` (tested).
- One access log line per connection, written when the connection closes, with status 101, the
  duration of the whole session, and the bytes the client sent. The `concurrency` plugin holds
  its slot while the connection is open; `rate_limit` counts only the handshake.
- Extended CONNECT/WebSocket over HTTP/2 (RFC 8441) is not implemented by this conduit build.
  Forwarded HTTP/1 upgrades use HTTP/1.1 upstream: `auto` selects H1, required H2 rejects the
  upgrade with 400. `upgrade: off` still removes the upgrade before that policy is applied.

**mTLS per site** (`mtls: { ca, optional }`)
- The site asks for a client certificate signed by the CA in `ca` (a PEM file, reloaded when it
  changes). No certificate or an invalid one: 403 with the reason (e.g. "certificate has
  expired"), logged as `"blocked_by": "mtls"`. `optional: true` lets clients without a
  certificate through.
- The upstream receives `X-Client-Verify` (`SUCCESS`/`NONE`), `X-Client-Cert-Subject`
  (RFC 4514) and `X-Client-Cert-Fingerprint` (SHA-256). The same headers from the client are
  always removed.
- The certificate request is set in the ClientHello callback, before OpenSSL decides on
  resumption. The session ID context is the SHA-256 of the CA file, so a TLS session can only be
  resumed under the same CA: sessions from a site without mTLS, from another CA, or from before
  the CA file was replaced are never reused (the client does a full handshake and is verified
  again). Valid mTLS sessions can still be resumed, on TLS 1.2 and 1.3.
- A connection made for a different host (HTTP/2 connection reuse, SNI ≠ Host) is answered with
  421, so the browser opens a connection of its own for that host. Sites sharing a CA file accept
  each other.

**Built-in ACME** (`tls.acme` and `acme: true` per site), no certbot
- One certificate per site for all its hosts. Issued automatically at start, and again when the
  site's hosts change.
- Renewed in the window the CA suggests (ARI, RFC 9773). If the CA gives no window, it renews
  when a third of the lifetime is left. `renew_before` can change that, up to half the
  lifetime.
- HTTP-01 challenges are answered by conduit itself from memory, on all hosts, before the HTTPS
  redirect.
- Key and chain are stored in one PEM file per site (mode 0600) and replaced atomically. A new
  certificate is put in place right away through a reload, without dropping connections.
- A restart uses the stored certificates and does not order again. A lock file keeps the old
  and new process from ordering at the same time during a graceful upgrade.
- When an order fails, it shows in the log, in `/status`, and in the metric
  `conduit_acme_orders_total{result="failure"}`. The next attempt backs off: 5 minutes, doubling,
  up to 6 hours (Let's Encrypt allows 5 failed validations per host per hour).
- Requests to the CA go through the same Pingora HTTP connector and OpenSSL as the proxy. The
  `instant-acme` library is used only for the protocol, so there is no second TLS stack.
- Limits: wildcards need DNS-01, which is not supported yet, so use certificate files for them.
  Port 80 (`listen.http`) must be reachable by the CA.

**Built-in CrowdSec** (the `crowdsec:` section and the `crowdsec` plugin per site), no separate
bouncer
- L7 bouncer: decisions are pulled from the Local API in stream mode (the full list at start,
  then changes every `update_interval`, default 10 seconds) and kept in memory. Checking a
  client takes a few hash lookups (about 26 ns with 100,000 decisions), with no network call per
  request.
- It checks the real client IP (`real_ip`), so behind Cloudflare it blocks the visitor. A host
  firewall bouncer cannot do this, because its TCP peer is Cloudflare.
- Addresses and ranges (IPv4/IPv6) are stored as they are; ranges are not expanded into
  addresses. One address can have several decisions, each with its own expiry. `ban` wins over
  `captcha`; captcha is not supported yet, so it is treated like a ban (the standard CrowdSec
  fallback).
- If the LAPI cannot be reached, existing decisions stay in force until they expire. After a
  failed pull, the next pull fetches the full list, because missed changes cannot be requested
  again.
- AppSec (WAF) per site (`crowdsec: { appsec: true }`): request headers and bodies of known
  length (≤ `body_limit`, at most 64 KiB) are sent to the AppSec component before the request is
  forwarded. The body is read once and then forwarded whole to the upstream through Pingora's
  replay buffer. Larger or chunked bodies are checked by their headers only. If AppSec does not
  answer within `timeout` (default 200 ms), the request is let through or refused according to
  `failure_action`.
- Refused clients get a 403 with an HTML page (replaceable with `ban_page`). The access log
  records `"blocked_by": "crowdsec"` or `"appsec"` (others: `"rate_limit"`, `"concurrency"`,
  `"mtls"`, `"basic_auth"`, `"forward_auth"`, `"block_exploits:<category>"`).
- A CrowdSec parser for conduit's JSON access log is in [`deploy/crowdsec/`](../deploy/crowdsec/).
  It fills the same fields as the nginx parser, so the hub's stock `http-*` scenarios work as
  they are.

**Upstream pools** (named `upstreams:`, or inline in a site)
- Many weighted backends. A DNS name that resolves to several IPs becomes several backends.
- Algorithms: `round_robin`, `random`, `least_conn` (in-flight divided by weight), and `hash`
  (consistent/Ketama) keyed by `client_ip`, `uri`, `path`, `header:<name>` or `cookie:<name>`.
- Active HTTP or TCP health checks (interval, timeout, `rise`/`fall`, expected status, TLS/SNI
  per backend).
- Passive ejection: a backend is taken out for a while after N consecutive errors on real
  traffic. If every backend is unhealthy, the pool goes into "panic mode" and keeps trying rather
  than answering 502 right away.
- Retries to another backend when a connect fails (`retries`, never to the same backend twice).
- Upstream timeouts (connect, TLS handshake, read, write) are answered with 504; other failures
  (refused, connection dropped, broken response) with 502, as in nginx.
- Pools whose definition did not change carry over on reload together with their state
  (healthy, ejected, in-flight).

**File server** (`files:` per site), like Caddy's `file_server`
- A site can serve files from disk only (no `upstream`), or a mix: `match` decides which requests
  come from disk and the rest go to the upstream. `pass_thru: true` forwards missing files to the
  upstream instead of answering 404. `strip_prefix` drops a path prefix before the lookup.
- `index` (default `index.html`, `index.txt`), `try_files` for single-page apps
  (`["{path}", "/index.html"]`), `browse` for directory listings (HTML, escaped, at most 10,000
  entries), `hide` for names that must never be served.
- Nothing outside `root` can be served: files are opened with `openat2(RESOLVE_BENEATH)`, so the
  kernel refuses symlinks that lead outside the root; the fallback walks pinned directory
  descriptors with `O_NOFOLLOW` and rejects all symlinks, including in-root links (it logs
  `files: openat2 unavailable`). systemd's `RestrictSUIDSGID=yes` makes
  openat2 fail with ENOSYS, so the unit in `deploy/` leaves it out. Dotfiles are hidden except
  `.well-known`. `..` above the root is refused during normalization (400). Only regular files are
  served, not FIFOs or devices (opened with `O_NONBLOCK`, so they cannot hang a thread).
- `ETag` and `Last-Modified`, conditional requests in RFC 9110 order (`If-Match` →
  `If-Unmodified-Since` → `If-None-Match` → `If-Modified-Since`: 304/412), a single byte range
  (206, 416 with `Content-Range: bytes */len`, `If-Range`), HEAD without a body.
- Canonical URLs: a directory without `/` and a file with `/` are redirected with 308, keeping
  the query. `Location` always starts with a single `/`, so `//host` cannot become an open
  redirect.
- `precompressed: [br, zstd, gzip]` sends `foo.css.br` and so on when the client accepts it,
  with `Vary: Accept-Encoding`. Other text files are compressed on the way out (whole responses
  only).
- Small files (≤128 KiB) are read once and sent whole; large files stream in 128 KiB pieces from
  the blocking pool, so request threads never wait on the disk. Content-Type comes from an
  extension table.
- A site without an upstream answers methods other than GET/HEAD with 405 and
  `Allow: GET, HEAD`.

**Response cache** (`cache:` per site, off by default; shared memory in the `cache:` section)
- Uses Pingora's HTTP cache (`pingora-cache`): in-memory storage with LRU eviction by size
  (`max_size`, default 64 MB); objects over `max_object_size` (8 MB) are passed through without
  being stored. Cache lock: concurrent misses for the same URL wait for one fetch from the
  upstream (test: 5 concurrent requests → 1 upstream request).
- Shared-cache rules per RFC 9111: only GET/HEAD without a body. `private`/`no-store` are not
  stored. Requests with `Authorization` are stored only with `public`/`s-maxage`. `Vary` is
  honored.
- conduit itself never stores responses with `Set-Cookie`, 206 responses, or `Vary: *`.
- `default_ttl` gives a freshness lifetime to responses that do not state one, only for statuses
  cacheable by default (200, 301, 404, …).
- Key = site, scheme, host, path and query, plus the identity conduit sends to the upstream:
  headers from `forward_auth`/`basic_auth`, the mTLS certificate fingerprint, and
  `X-Forwarded-Host` from trusted proxies. Different users never share an entry, and clients
  cannot poison the cache through forwarded headers.
- Responses the upstream compressed vary automatically on the normalized `Accept-Encoding`.
  Clients that do not accept gzip never receive a gzip entry.
- `stale-while-revalidate`: the stale copy is sent at once and refreshed in the background.
- `stale-if-error` (or `stale_if_error` per site): the stale copy stands in when the upstream
  fails.
- Revalidation uses `ETag`/`Last-Modified` (a 304 from the upstream refreshes the entry).
- A `Cache-Status` header (RFC 9211) on every response of a cached site, e.g. `conduit; hit`,
  `conduit; fwd=miss; stored`, `conduit; hit; detail=stale`.
- The result is also in the access log (`"cache"`) and in the metric
  `conduit_cache_requests_total{result}`.
- Purge through the admin listener: `POST /cache/purge?site=<id>[&prefix=/path]` (JSON with the
  number of entries). On reload, entries of removed sites or of sites whose `cache:` was taken
  away are dropped. A restart and a graceful upgrade start with an empty cache.
- Note: Cloudflare still marks Pingora's cache API as experimental. The upstream decides what
  may be shared: pages that differ per cookie must send `private`.

**Plugins** (a chain per site, run in order)
- `rules`: match method/path/uri/query/scheme/header, then `deny`, `redirect`, `respond` or
  `file`. Paths are normalized (`/%5F/`, `//_`, `/a/../_` all become `/_/`), so encodings cannot
  slip past a rule.
- The `proxy` action in `rules`: the request is forwarded rather than answered. It can go to
  another upstream (a pool name, a URL, or a pool definition; default: the site's upstream) and
  with a changed path: `strip_prefix`, `add_prefix`, or `rewrite: { regex, to }` (`${1}`,
  `${name}`). This is the equivalent of `location` + `proxy_pass` and `rewrite` in nginx. Path
  changes are computed from the normalized path (so `/%61pi/` still matches
  `strip_prefix: /api`) and sent re-encoded with the original query. As in nginx, `%2F` reaches
  the upstream as `/`; requests whose path is not changed are forwarded as they are. Plugins
  after the rule still run, and routed requests are not served by `files`. Inline pools in a
  rule (`site:<id>:rule<p>.<r>`) get health checks and metrics and keep their state on reload.
  Cache entries are kept apart per pool and per routed target.
- `body_replace`: replace strings in the upstream's response body (the equivalent of
  `sub_filter`) while streaming, without buffering the whole body. At most the length of the
  longest `from` minus one byte is held back between chunks, so a pattern split across a chunk
  boundary is still found. Only for `types` (default `text/html`). The upstream is asked for the
  whole body uncompressed (`Accept-Encoding`, `Range`, `If-Range` are not forwarded); responses
  that still arrive compressed, 206 responses, and local answers are left alone. Compression to
  the client happens afterwards. Changed responses lose `Content-Length`, `ETag`, and
  `Last-Modified`, as in nginx.
- `block_exploits`: refuse requests that carry common attack payloads (what a `block-exploits`
  include does in nginx), with patterns built so ordinary requests pass: OAuth
  `redirect_uri=http://...`, a search for "union station", repository files under `etc/passwd`,
  `.env.example` in forgejo, `/.well-known/`. What is checked: the query (decoded the way an
  application reads it, `+` as a space; decoded once more if escapes remain; also a version with
  SQL comments removed), the normalized path, `User-Agent`, and other headers only for header
  payloads (Log4Shell, Shellshock, OGNL). The body is not read (that is AppSec's job).
  Categories: `traversal`, `dotfiles` (`/.env`, `/.git/`, `/.aws/` ... only at the root), `sqli`
  (UNION SELECT with a column list, `or 1=1` tautologies checked for equal sides, stacked
  queries, time-based, error-based), `xss` (script tags, event handlers in tags, `javascript:`
  URLs followed by code), `php` (stream wrappers, PHP-CGI, superglobals, object injection,
  phpunit), `rce` (Log4Shell, Shellshock, Spring4Shell, OGNL, template injection, shell commands,
  the Next.js `x-middleware-subrequest` header), `ssrf` (cloud metadata addresses), `scanners`
  (sqlmap, nikto, nuclei, etc., plus the user agents of the common nginx `block-exploits.conf`
  without download managers and TurnitinBot), and `spam` (that file's words, off unless named).
  Every rule has literal keywords; one Aho-Corasick pass per input selects the rules to run, then
  the regexes run (the `regex` crate, linear time, safe from ReDoS). Cost: about 0.2 µs for a
  request without a query, 0.5–0.7 µs with an ordinary query. `mode: log` only counts and logs.
  Every detection is logged as a warning (at most 20 per second) with its rule id; `disable`
  turns off given rules, `skip_params` exempts query parameters (search boxes), and `except`
  exempts categories for matching requests. The access log records
  `"blocked_by": "block_exploits:<category>"`, the metric is
  `conduit_block_exploits_total{site, category, action}`.
- `headers`: set/remove request headers to the upstream and response headers.
- `ip_filter`: allow/deny CIDRs by real client IP.
- `crowdsec`: refuse clients with a CrowdSec decision, and optionally check requests with
  AppSec.
- `rate_limit`: at most N requests per key within the `per` window (a sliding estimate over two
  windows); the rest get 429 + `Retry-After`. Keys: `client_ip` (IPv6 counted per /64), `host`
  or `header:<name>`; with `match`, only matching requests are counted and limited.
- `concurrency`: at most N requests of one key in flight at once; the excess is refused at once,
  not queued.
- `rate_limit` and `concurrency` use the `pingora-limits` count-min sketch (64 KiB per plugin, a
  fixed size whatever the number of clients). Their counts survive a reload as long as the
  settings and the plugin's position do not change, and they can overestimate but never
  underestimate. The limits are deliberately at L7 (requests per real client), not at L4 through
  `ConnectionFilter`: behind Cloudflare every connection comes from a Cloudflare IP, so a per-IP
  limit at L4 would throttle Cloudflare, not the attacker.
- `basic_auth`: username/password with bcrypt hashes (`htpasswd -nB`, `caddy hash-password`),
  from `users:` or an htpasswd file (reloaded when it changes), optionally only for requests that
  match `match`. bcrypt runs in the blocking pool (at most one per CPU), not on request threads;
  verified passwords are remembered until the next reload, so later requests skip hashing.
  Unknown users are still verified against another hash, so the timing is the same.
- `forward_auth`: ask an auth service (Authelia, Authentik, oauth2-proxy) with a GET carrying the
  client's headers (including cookies) and `X-Forwarded-Method/-Uri/-Host/-Proto/-For`. 2xx:
  allowed, and the `copy_headers` are copied to the upstream request (removed when absent from
  the answer, so the client cannot forge them). Anything other than 2xx: the auth service's
  answer goes to the client as it is (login redirect, several `Set-Cookie`). No answer: 502. The
  request body is not touched. Connections to the auth service are kept alive.
- Plugins that have to wait (bcrypt, the auth service) still run in written order: the chain
  stops at that plugin, waits, then continues. `rules` after `basic_auth` are reached only with
  the right password. The other plugins stay synchronous with no extra allocation.

**Observability** (the `admin.listen` listener, no authentication, keep it on loopback)
- Prometheus `/metrics`: requests per site and status class, histograms of total duration and
  upstream header time, response bytes, in-flight, requests/failures/up state/ejections per
  backend, retries per pool, reload results, certificate expiry, ACME orders, the number of
  CrowdSec decisions, LAPI pull results, refused requests (decision/AppSec), AppSec verdicts,
  requests refused by rate limit/concurrency per site (`conduit_limit_refused_total`),
  `basic_auth` and `forward_auth` results per site
  (`conduit_auth_requests_total{result=allow|deny|error}`), cache results per site
  (`conduit_cache_requests_total{result=hit|stale|revalidated|miss|uncacheable|bypass}`), cache
  contents (`conduit_cache_objects`, `conduit_cache_size_bytes`, `conduit_cache_evicted_objects`),
  idle upstream connections that could not be reused (`conduit_upstream_reuse_lost_total`, see
  [Threads and runtime](#threads-and-runtime)), dropped log lines, and process metrics
  (`process_*`, CPU, RSS, file descriptors of the running process).
- `/status` JSON: config, time and error of the last reload, sites (upstream, `files` root,
  cache, plugins, mTLS mode), pools and the state of each backend, certificates, ACME per site,
  CrowdSec (number of decisions, last pull, last error), cache contents, access log path.
  `/healthz` for liveness.
- `POST /cache/purge?site=<id>[&prefix=/path]` drops a site's cache entries (all of them, or those
  whose path+query starts with `prefix`, as the client sent it).
- A JSON access log, one line per request: request ID, site, client, status, bytes, duration,
  backend, upstream status, connect time and upstream header time, number of retries, TLS
  version. Each worker writes to its own buffer, and one thread collects those buffers every
  20 ms and writes them to the file. Within one such window, lines are grouped per worker, so
  sort by `ts`. When a buffer is full, lines are dropped and counted, and traffic does not slow
  down. It follows logrotate by itself.
- A request ID (`X-Request-Id`, 24 hex digits) in the response, to the upstream, and in the log.
  An ID from the client is accepted only from trusted peers.

**Operations**
- Reloads without dropped connections: SIGHUP or a file change for config and certificates;
  `systemctl reload` (Pingora upgrade, socket handover) for a new binary or listen addresses.
- A renewal caught halfway (new certificate, key still old) is refused and the old certificate
  keeps being served. File stamps are taken before reading, so a key that arrives later triggers
  the next reload.
- `conduit check` validates a config. Unknown keys are refused, so typos fail in `check`.

## Phase 1 targets

| Target | Result | Status |
|---|---|---|
| Idle RSS < 30 MB with 100 sites | 25.6–26.6 MB (PGO build) | ✓ |
| 0 failed requests during reloads, upgrades, and certificate rotation | 0 failed (tests below) | ✓ |
| Added p99 < 1 ms at 50% of conduit's HTTP peak | PGO build after the third round of Pingora patches, idle machine, median of 3 rounds: +0.51–0.57 ms (3 runs at 38,400–39,200 req/s). After the first round +0.60–0.81 ms at 34,700–36,400 req/s; before the patches +0.55–0.64 ms at about 28,400 req/s | ✓ |
| HTTP/1.1 and HTTPS keep-alive throughput ≥0.75× tuned nginx, or CPU per request ≤1.4× (changed on 2026-11-08 from "on par with nginx") | HTTP/1.1 0.76–0.81× (CPU 1.31–1.40×), HTTPS 0.77–0.78× (CPU 1.34–1.37×); HTTP/2 and TLS handshakes ahead of nginx (1.28–1.32× and 1.09–1.10×). See [HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput) | ✓ |

### HTTP/1.1 and HTTPS throughput

The last phase 1 target, closed on 2026-11-08. Numbers from a PGO release build on an idle
machine, against tuned nginx in the same run (`scripts/bench.sh dist/conduit`, 2026-11-08).
"Stock" is 10 runs with stock Pingora, "P1" 5 runs after the first round of patches, "P3" 3 runs
after the third round:

| Scenario | req/s conduit / nginx: stock | P1 | P3 | CPU per request conduit / nginx: stock | P1 | P3 | Status |
|---|---:|---:|---:|---:|---:|---:|---|
| HTTP/2 | 1.05–1.12× | 1.15–1.22× | 1.29–1.32× | 0.97–1.00× | 0.88–0.90× | 0.80–0.82× | ✓ |
| New TLS handshake per request | 1.06–1.09× | 1.06–1.08× | 1.10× | 0.87–0.89× | 0.87–0.90× | 0.86–0.89× | ✓ |
| HTTPS keep-alive (HTTP/1.1) | 0.64–0.65× | 0.65–0.71× | 0.77–0.78× | 1.68–1.74× | 1.37–1.56× | 1.34–1.37× | ✓ (≥0.75×) |
| HTTP/1.1 keep-alive | 0.60–0.63× | 0.67–0.70× | 0.76–0.81× | 1.73–1.89× | 1.56–1.61× | 1.31–1.38× | ✓ (≥0.75×) |

Before PGO, HTTP/1.1 keep-alive was at 0.51–0.55×.

The gap is inside Pingora, not in conduit's code. From the [CPU profile](#cpu-profile) before the
patches: conduit's code was about 13% of worker CPU time, Pingora and tokio about 51%, syscalls
36%. What conduit could do on its side is already in use: PGO (+14–16% req/s),
`work_stealing: false`, a 16 KiB write buffer (+11%). Decision of 2026-11-07: stay on Pingora,
do not write an HTTP/1 stack of our own.

**Pingora patches (done 2026-11-08).** Pingora 0.9.1 is forked into `vendor/pingora` (branch
`conduit`, one commit per patch) and wired in through `[patch.crates-io]`. Details, numbers per
patch, and how to update are in [`vendor/README.md`](../vendor/README.md). In short:

| Patch | Effect (A/B, plain build) |
|---|---|
| Idle upstream connections are kept without a tokio task per connection (before: a spawned task, a timer, a oneshot, and an `Arc<Mutex>` on every request) | CPU −9% HTTP, −7% HTTPS |
| The connection reuse check uses the address recorded at connect time, without `getpeername()` | 1 syscall per request fewer |
| Compression MIME check by prefix, without a regex; hop-by-hop headers checked in one pass; `insert_header` with an already parsed name (`CasedHeaderName`) does not parse it again | CPU −3–4% |
| Header name case kept as a style plus exceptions, not a second `HeaderMap` | CPU 0 to −1% |
| A batch of response tasks is flushed once: a compressed response goes out in 1 `send()`, not 3 (header, body, `0\r\n\r\n`) | gzip: req/s +23%, CPU −13.5%, p99 −19% |

Plus one change in conduit itself: upstream connections are pooled per worker thread (see
[Threads and runtime](#threads-and-runtime)), HTTP +4% req/s and CPU −4%.

New PGO release against the old PGO release (`scripts/ab.sh`, 7 rounds): HTTP +12% req/s and
−12% CPU, HTTPS +9% and −13% CPU, HTTP/2 +9% and −11% CPU, saturated p99 −16 to −26%; TLS
handshakes unchanged (their cost is in the cryptography). Idle RSS unchanged (25.6 MB).

Second round (HTTP/1.1 patches, afternoon of 2026-11-08), new PGO release against that morning's
PGO release (`scripts/ab.sh`, 7 rounds, proxy on 2 physical cores):

| Scenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, saturated | +6.3% | −8.4% | −3.2% |
| HTTP with gzip, saturated | +26.8% | ±0 | −17.2% |
| HTTPS keep-alive, saturated | +8.2% | −7.3% | −3.6% |
| HTTP/2, saturated | +3.0% | +7.0% | −3.0% |
| New TLS handshake | +3.7% | −3.1% | −1.6% |

In `scripts/bench.sh` the ratios against tuned nginx went up: HTTP 0.66× → 0.71–0.72×, HTTPS
0.70× → 0.72–0.74×, HTTP/2 1.20× → 1.22–1.23×. The gate failed on one number: saturated HTTP p99
of 1.85 ms (two runs) against a 1.30 ms baseline, while the old binary in the same run had
1.19 ms. Its p50 actually went down (0.96 → 0.79 ms): the distribution got wider. In
`scripts/ab.sh` with the same CPU layout that saturated p99 difference was +16% with overlapping
per-round ranges, and at fixed rates (31k and 43k req/s, about 50% and 70% of peak) p99 was the
same within noise. The baseline was not re-recorded.

Third round (evening of 2026-11-08): candidates of about 1–2% from the profile. Per step, plain
build (`scripts/ab.sh`, 7 rounds):

| Change | HTTP | HTTPS | HTTP/2 |
|---|---|---|---|
| conduit's access log rewritten: JSON lines are built straight into a buffer owned by each worker thread (no `serde_json` and no channel), and a writer thread collects all buffers every 20 ms | req/s +1.8%, CPU −2.8% | +2.5%, −2.2% | ±0, −1.8% |
| The upstream response header is handed over to the proxy rather than copied (`bb3859d`) | +1.4%, −1.4% | +1.7%, −2.2% | −1.8%, −0.9% |
| An HTTP/1 task channel of its own, replacing two `tokio::sync::mpsc` per request (`c4cce19`; two A/B runs in a row: a `Mutex` version, then a spinlock) | +0.7%, ±0 | +4.1%, −2.3% | +0.8%, −4.7% |

The JSON access log writer was tested against `serde_json` with 20,000 random entries (including
control characters, quotes, and IPv6), so the format is byte for byte the same. `scripts/ab.sh`
now also checks that the number of access log lines equals the number of requests and that none
were dropped.

The request header clone was left alone. The client's original header is still used after
`upstream_request_filter` changes the copy: for retries to another backend, the access log, and
the cache key. Removing that clone would change the `ProxyHttp` contract, and its share in the
profile is under 1%.

New PGO release against the second-round PGO release (`scripts/ab.sh`, 7 rounds, 0 access log
lines lost):

| Scenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, saturated | +9.7% | +2.6% | −9.8% |
| HTTP with gzip, saturated | +6.7% | −8.8% | −7.6% |
| HTTPS keep-alive, saturated | +6.8% | −11.1% | −7.4% |
| HTTP/2, saturated | +6.0% | −5.9% | −6.2% |
| New TLS handshake | −2.2% (overlapping ranges) | +3.3% | −1.2% |

The HTTP req/s ranges per round do not overlap (113–119k against 104–109k).

In `scripts/bench.sh` (3 runs) the ratios against tuned nginx rose again: HTTP 0.71–0.72× →
0.76–0.81×, HTTPS 0.72–0.74× → 0.77–0.78×, HTTP/2 1.22–1.23× → 1.29–1.32×. 0 access log lines
lost (about 3 million lines per run). The gate failed in all three runs on the p99 of the 70%
scenario: 2.18–3.75 ms against a 1.65 ms baseline. This is not a regression. The rate of the 70%
scenario was derived from conduit's peak in that run, and the peak rose by about 10%, so the
scenario now ran at 53,800–54,900 req/s instead of about 49,000. At that rate the test machine
itself is close to saturation: nginx in the same run also added +1.39 to +5.97 ms of p99. At the
same rate, the new release beats the old one (`scripts/ab.sh SCENARIOS=rate`, 5 rounds): p99
−13.7% at 49,188 req/s and −20.6% at 54,901 req/s, CPU −8% and −9%. One run also recorded a
saturated HTTPS p99 of 2.52 ms (the other two runs 1.48–1.49 ms, and the A/B above −11%). Since
then `scripts/bench.sh` runs the 50% and 70% scenarios at the rate from the baseline, not from
that run's peak, so a faster release is not automatically tested under a heavier load.

Stage A (late evening of 2026-11-08), small changes pointed out by the line-level profile:
headers to the upstream without `to_string()` for the client address and with a static scheme
value, the request ID counter taken in blocks by each worker (not one shared atomic per request),
host lookup with foldhash, and the Pingora patch `0a6aee4` (HTTP modules looked up without a
shared index). A/B plain build, 7 rounds: HTTP +1.1% req/s (CPU ±0), gzip +2.1% (CPU −1.9%),
HTTPS +1.2% (−2.7%), HTTP/2 +3.4% (−2.9%). In `scripts/bench.sh` differences this small are lost
in the noise: HTTP 0.78×, HTTPS 0.77×, HTTP/2 1.28× (gate passed). What is left of the cost in
conduit's code is spread over many small spots; the next big step is in Pingora.

Why it is not on par yet: after the third round, about 44% of conduit's worker CPU is in syscalls
and about 56% in user space. If the kernel cost per request equals nginx's (an estimate, not
measured), conduit uses about 1.7× nginx's user-space CPU (about 2.5× before this round). The rest
of the cost is spread thin with no single spot above about 1.5% ([CPU profile](#cpu-profile)):
allocations, memcpy of large async futures between `await`s, and Pingora's proxy glue code. The
next step is no longer a small patch but a change in the shape of Pingora's request flow
(smaller futures, fewer allocations per request).

The original target was "on par with tuned nginx". Decision of 2026-11-08: the target became
≥0.75× tuned nginx or CPU per request ≤1.4× (either is enough), without breaking the p99 and idle
RSS targets above, and phase 1 was closed. The reasons:
- The other targets are met.
- At ordinary traffic this gap limits nothing: sites well under 1,000 req/s, while conduit on
  2 CPUs handles about 78,000 req/s HTTP and about 58,000 req/s HTTPS.
- The next step is no longer a small patch: an HTTP/1 fast path in the Pingora fork (estimated
  0.9–0.95×, a large fork diff that is hard to rebase), and for 1.0× likely an HTTP/1 stack of
  our own, which goes against the decision of 2026-11-07.

That fast path stays open as optional work in case parity with nginx is ever needed.

## Current local validation

The final implementation in `03cae2b` passed **111 application unit tests and 34 E2E tests**:
109 default unit tests plus two explicit measurement tests; 30 default E2E tests plus ACME,
CrowdSec, slowloris and HTTP/2 soak run explicitly. The HTTP/2 cases cover h2c, TLS ALPN,
strict fallback refusal, concurrent streams, stream-limit reloads, health probes, cache,
gRPC bidirectional streaming, a 2 MiB frame, trailers, cancellation and reload mid-stream.
Five functional H2 cases were repeated against the final release binary. One additional
native Pingora regression passed for the H2 pool reuse backport; the full vendor suite was
not repeated for this revision. Clippy and rustfmt passed.

| Evidence | Scope and result |
|---|---|
| [Audit closure](audit/FIX_VALIDATION.md) | Eight security regressions and the final release PoC close the validated findings; F04 retains documented TTL/purge behavior. |
| [HTTP/2 report](audit/HTTP2_UPSTREAM_VALIDATION.md) | Five H2 functional cases, one H2 soak and the complete application suite; source/binary hashes and raw logs. |
| Default-path release A/B | Five alternating rounds, 6,000,000 requests, zero errors; all use H1 upstream. Median throughput change: HTTP +1.5%, HTTPS +9.3%, client H2 +2.5%. Shared-host and affinity limits prevent a universal speedup claim. |
| Final release H2 soak | 120 seconds, 1,500 req/s offered load, 179,999 successful requests, 23 reloads, FDs 40 → 39 after drain; RSS 28,152 → 29,596 KiB. |
| Earlier mixed local soak | Before the H2 feature: 600 seconds of traffic, 432,000 requests and 23,960 long-lived WebSocket echoes, zero observed errors. |

The local 120-second run is bounded stability evidence. The Krynox 24-hour run mentioned by
the operator remains a separate run; no deployment, restart or live verification is claimed.
See the [audit index](audit/README.md) for the distinction between historical and current
results and the [configuration reference](configuration.md#upstream-protocol-behavior) for
HTTP/1.1, required H2, auto negotiation, frontend TLS and upgrade constraints.

## Test results (local)

This detailed catalog includes historical phase 1–3 runs. Counts such as 86 unit / 9 E2E
in the fork row describe that earlier run. The totals and raw evidence for the final build
are in [current local validation](#current-local-validation). Historical fuzz/production-log
replay campaigns and the older 24-hour soak were not repeated during the HTTP/2 change.

| | |
|---|---|
| Unit tests, final validation 2026-11-10 | 109 default + 2 explicit measurements = 111 passed; importer tests were removed with that feature |
| `Server` header (`server_header` e2e) | Default (option not set): proxied responses (replacing `Server: echo-upstream`), 404 for an unknown host, 403 from `ip_filter`, 502 for a dead upstream, 400 for a broken header, and 405 for `CONNECT` all carry exactly one `Server: conduit`; a site with a `headers` remove has no `Server`. After a reload with `server_header: off`: the upstream's `Server` passes through, and the five local responses above have no `Server` (not `Pingora`). A value with CRLF is refused on reload and the old config keeps running. Unit tests: unset → `conduit`, `off` and `false` → none, another name as written; empty, surrounding spaces, non-ASCII, longer than 64 characters → refused |
| `block_exploits` on real traffic (2026-11-09) | 1,139,833 requests from a production nginx access log (17 Apr–9 Nov 2026, 51 proxy hosts; read read-only, client IPs hashed before storing, the copy deleted afterwards) replayed into the engine with default settings. 25,117 (2.2%) would be blocked, and every one was checked: secret file probes (`/.env*`, `/.git/config`, `/.aws/`, `.npmrc`, etc.; 22,554, from 2,313 clients), traversal (`../../`, `..;/`, `/proc/self/environ`), phpunit `eval-stdin.php`, Struts OGNL, Shellshock, pearcmd, cloud metadata through proxy parameters, `?=phpinfo()`, the zgrab UA, and Dolibarr `file=../conf/conf.php`. One false positive was found and fixed: TurnitinBot (a legitimate crawler, from NPM's UA list; the download managers on that list were dropped too). One missed attack was added: one level up to a config file (Dolibarr, 145 requests to 7 hosts). After the fixes, the only differences from the previous run are those two. Blocked by NPM but let through by conduit: `=http://` (on purpose, see parity) and `/./`; the rest come from Next.js header rules in advanced configs, which conduit handles with `rules` |
| `block_exploits` (unit + `block_exploits` e2e) | 163 attack payloads (sqlmap, nuclei, PayloadsAllTheThings, Log4Shell and its obfuscations, Spring4Shell, Confluence/Struts OGNL, PHP-CGI CVE-2024-4577, phpunit, ThinkPHP, pearcmd, reverse shells, cloud metadata, scanner UAs, Dolibarr `../conf/conf.php`) must hit the right rule; 74 ordinary requests that look like attacks must pass (OAuth `redirect_uri=http://`, S3 presigned URLs, registry digests, repository files `etc/passwd` and `.env.example`, code search for `select * from`, `time.sleep(5)`, `<button onclick=`, "JavaScript: The Good Parts", "O'Reilly and Sons", `order=desc`, Grafana variables `${env:raw}`, WebDAV `.git/`, `/.well-known/`, Uptime Kuma/curl/python-requests/TurnitinBot/GetRight UAs, `../` to an ordinary file). Results with and without the Aho-Corasick keywords are the same for all of these inputs. E2E: attacks → 403 and never reach the upstream, ordinary requests arrive whole, `except` and `skip_params`, `log` mode lets through and counts, metrics per category, `blocked_by` in the access log, warning lines with the rule id. Cost (`cost`, release): about 0.22 µs without a query, 0.5 µs with an ordinary query, 0.6 µs for an OAuth URL, 19 µs for a 5 KB query |
| Routes and `body_replace` (`routes_and_body_replace`) | `strip_prefix` to a named pool, including encoded paths (`/%61pi/./v1/../v2` → `/v2`) and re-encoded spaces, regex `rewrite` keeping the query, `add_prefix` to an inline URL chosen by a header, requests without a route forwarded as they are; a `deny` rule before a route still wins, and plugins after the route still run. `body_replace`: a pattern split over 3 upstream chunks is still replaced, the upstream receives no `Accept-Encoding`/`Range`, `ETag`/`Last-Modified`/`Content-Length` are removed, a response with `Content-Length` becomes chunked, an HTTP/1.0 client receives the body until the connection closes, JSON and gzip bodies are left alone. Unit tests: every way of cutting the body (two pieces anywhere, and byte by byte) gives the same result as the whole body |
| End-to-end (real binary) | proxying and headers, request IDs, SNI, HTTP/2, plugins, 502, LB spread over 2 backends, health checks marking a backend down, retries + passive ejection, metrics, admin endpoints, reload keeping pool state, failed reload shown in `/status`, JSON access log, rate limiting (429 + `Retry-After` only on the matching path and per client, counts surviving a reload) and concurrency (a third request refused at once while two are running, other clients unaffected), request and response bodies (0 B to 3 MB up and 20 MB down, `Content-Length` and chunked with uneven pieces, an upstream that reads slowly, one keep-alive connection for all of it, 8 parallel clients; content checked with SHA-256) |
| mTLS (real handshakes) | no certificate, another CA, and an expired certificate → 403 with the reason; a valid certificate → the upstream receives subject and fingerprint, forged headers from the client are removed; `optional`; SNI ≠ Host → 421, sites with the same CA accept each other; HTTP/2 through curl; **TLS 1.2 and 1.3 resumption**: sessions of the same site are resumed with their certificate, sessions from a site without mTLS or with another CA are not resumed, sessions with a bad certificate are still refused; CA file replaced → old sessions are not resumed and the new CA applies at once. Passed 5 out of 5 times |
| Auth | `basic_auth` only on the matching path, rules after it not reached without a password, wrong password and unknown user → 401, later verifications from the cache; `forward_auth`: no session → login redirect with 2 intact `Set-Cookie`, the `X-Forwarded-*` headers the auth service receives are correct and cannot be forged by the client, `copy_headers` copied or removed, POST body arrives whole, auth service down → 502; metrics and `blocked_by`. Passed 5 out of 5 times |
| Certificate rotation and reload under load | continuous HTTP and HTTPS traffic (about 6,000 + 600 requests) during: a certbot-style rotation (rename, key first), a certificate written in place with the key following 1.5 seconds later (the halfway reload is refused, the old certificate keeps being served), and a new site through SIGHUP. 0 failed, both new certificates visible, TLS connections opened before the rotation keep working. Passed 5 out of 5 times |
| Graceful upgrade | thousands of requests during the socket handover, 0 failed; `/status` shows the new PID |
| Graceful upgrade with open WebSockets (`upgrade_with_open_websockets`) | 5 WebSockets opened to the old process, then `run -u` + SIGQUIT: the old WebSockets keep exchanging data during the handover and the grace period; about 5.5 seconds after SIGQUIT new connections and `/status` are served by the new process; the old process exits 7–12 seconds after SIGQUIT (Pingora's 5 seconds + `shutdown_grace` + `shutdown_timeout`) and drops the WebSockets it still holds; each of them still has an access log line (101, `"error":"Cancelled"`); WebSockets on the new process keep going |
| Idle, slow, and broken clients (`idle_and_malformed_clients`, `slow_request_head_is_cut_off`) | idle keep-alive connections closed per `keepalive_timeout` even when the client asks for `Keep-Alive: timeout=600`; a 64 KiB header → 200; a 2 MiB header, 300 header lines, and garbage → 4xx without hanging; a header sent one byte every few seconds is cut off after 62 seconds (`--ignored`, takes about 1 minute) |
| Running out of fds (`file_descriptors_run_out`) | conduit with `ulimit -n 128` flooded with 250 connections: no panic, no spinning (CPU < 500 ms over 3 seconds while accept fails with `EMFILE`), and serving again once connections close |
| Fuzzing (`fuzz/`, cargo-fuzz + libFuzzer) | 7 targets (`config`, `path`, `access_log`, `request`, `route`, `body_replace`, `block_exploits`; see [Commands](#commands)). After `block_exploits` was added (also hooked into the `request` harness), 10 minutes per affected target: 1.26 million `block_exploits` inputs (232 seeds from an attack corpus), 4.0 million requests, 0.72 million configs, 0 crashes and 0 differences between the fast path and the path without keywords. 20 minutes per target after the fixes: 1.4 million configs, 101 million paths, 17 million access log lines, 8.9 million requests, 0 crashes. After the `proxy` action and `body_replace` were added, another 10 minutes per target (config, path, access_log: 0.59 / 41 / 6.5 million), then 10 minutes with route and `body_replace` in the `request` harness: 5.2 million requests, 4.5 million route targets, 2.8 million bodies (two rounds), 0 crashes. One finding in `route` turned out to be a wrong harness assumption: a query containing `<` is already refused by Pingora (400) before it reaches routing; that input is now a seed. An earlier finding is fixed and has a unit test: a target without a path (`target_without_path_is_400`) |
| Soak test, 24 hours (`scripts/soak/`, 2026-11-08/09) | On a 2 vCPU / 2 GB test VM, with a build from 2026-11-08 (before the 504, `keepalive_timeout` and request header deadline fixes): 56.2 million requests (HTTP/1.1 small and 256 KiB gzip, POST, HTTPS, HTTP/2, a new TLS handshake per request; about 650 req/s), 3.45 million echoes on 40 long-lived WebSockets and 172,450 short WebSockets, during 288 reloads, 286 backend outages of 30 seconds (TCP resets, including on open connections), 48 access log rotations, 24 certificate rotations and 6 graceful upgrades (each served by the new process, the old one gone). Hourly medians over the whole run: RSS 45–48 MB, 167 fds, 12 threads, 7 upstream connections; peak 55 MB and 177 fds. Per process (at most 4 hours between upgrades), RSS at the end was at most 0.3 MB above its value 15 minutes in (peaks up to 52 MB came back down), except the last one, which stepped from 47 to 53 MB within 30 seconds at 1 h 40 min and ended at 55 MB; the run ended before it showed whether that comes back down too. 0 panics, 0 dead processes, 0 access log lines dropped, 56.3 million access log lines. 120 requests failed (0.0002%), all explained: 37 POSTs and 1 GET answered 502 when the backend was reset in the middle of a request (a POST whose body was sent is not retried; nginx does the same); 80 HTTP/2 requests, 16 at 5 of the 6 upgrades (2 connections × 8 streams): 78 are hyper's "operation was canceled", requests oha never sent after the old process's GOAWAY and does not retry, 2 are "http2 error" (the old process keeps serving the streams in flight; browsers open a new connection); 2 requests lost in the keep-alive race when the old process closes idle connections. The 40 long-lived WebSockets were dropped by each old process as it exited (240, all reconnected; none outside the 30 seconds after an upgrade). p99 per stream 0.9–4.6 ms (median over the 5-minute windows), worst 6.1 ms |
| Upstream flapping under load (`upstream_flapping`) | about 100,000 GET and POST requests from 6 clients (keep-alive and new connections) to a pool of 3 backends, while: one backend is down for 2.5 seconds, one backend hangs for 2.5 seconds, all three are down for 1.5 seconds, then one backend goes down and up 20 times every 100 ms. Every failed request must be explained: 502 only for requests in flight when a backend died, or while all backends were down (failing at once, under 100 ms, not piling up); 504 only for the hanging backend (4–6 requests before the health check takes it out). A GET on an upstream connection that turned out to be dead is retried automatically, a POST is not (not safe to repeat). No mixed-up answers, no client connection dropped without an answer, and no panics. Afterwards every backend takes traffic again, `in_flight` returns to 0, the fd count returns to where it started, and every request is logged once in the access log. Passed 4 out of 4 times |
| Parity with nginx (local, Docker) | nginx (the Nginx Proxy Manager 2.14.0 image, the same version as production) loads a copy of the generated configs of 51 proxy hosts, conduit an equivalent config of the same hosts including translations of 9 advanced configs; both point at a fake upstream (`scripts/parity/stub.py`) that answers with the request it received, and `scripts/parity.py` compares the two. 189 requests per proxy: `/` over HTTP and HTTPS with browser and non-browser UAs on every host, plus every advanced rule (a browser-only forgejo redirect, CORS + `OPTIONS` 204, a Next.js header block, `debug.log` 404, `mobile-config.json`, an S3 console, a PocketBase admin block including `%5F` and upper case), paths with `..`/spaces/encodings, POST, WebSocket and headscale upgrades, unknown hosts. Status, `Location`, `Content-Type`, CORS/cache/HSTS headers, and the target/method/headers that reach the upstream are the same. Deliberate differences: conduit adds `X-Forwarded-Host` and `X-Request-Id`, keeps upstream connections alive (nginx sends `Connection: close`), does not send `X-Forwarded-Scheme`, does not trust `X-Forwarded-Proto` from arbitrary clients (nginx passes it on), and compresses JSON for clients that ask for gzip. With `block_exploits` on the 38 hosts that used nginx's exploit rules: 197 requests per proxy, and the only status differences are the deliberate ones. Blocked by nginx but passed by conduit: `redirect_uri=http://...`, `<div>description</div>` (nginx's `<.*script.*>` pattern), `concat(`. Passed by nginx but blocked by conduit: `' union select 1` (nginx's pattern needs `(`), `/.env`, `/..;/`, Log4Shell in a header, the sqlmap UA, `' or '1'='1`. Attacks nginx blocks (`union select (1)`, `<script>`, `../../etc/passwd`) are blocked by both |
| ACME with Pebble (Let's Encrypt's test CA, `mise run test-acme`) | issued about 1 second after start; a restart uses the stored certificate without a new order; a certificate close to expiry is renewed in the middle of about 285 HTTPS requests with 0 failed; a name the CA refuses shows in `/status` and is retried later without disturbing other sites. Passed 5 out of 5 times (including after the last phase 2 changes) |
| Real CrowdSec v1.8.2 in Docker (`mise run test-crowdsec`) | a `cscli` decision applies in conduit about 1 second later (address, range, captcha → refused), deleted → allowed again; AppSec refuses `/.env` and a body with a forbidden pattern, a clean body reaches the upstream whole, sites without AppSec are unaffected; CrowdSec paused: AppSec lets through after the timeout, existing decisions stay in force, pulls recover by themselves; **full loop**: 20 probing 404 requests in the access log → the `deploy/crowdsec` parser → the `http-probing` scenario → the IP blocked by conduit about 2 seconds after the last probe. Passed 5 out of 5 times (including after the last phase 2 changes) |
| File server | index, `ETag`/`Last-Modified` → 304, `If-Match` → 412, range 206 without compression, 416 with `Content-Range`, stale `If-Range` → 200, a 300 KB file streamed whole, gzip for whole text files, the 300 KB file compressed with `br`, `zstd`, and `gzip` decoded whole by strict decoders (without `be20dfc`: `br: Unexpected EOF`), HEAD, precompressed `.br` + `Vary`, canonical 308 redirects with the query (`//sub` → `/sub/`, never to another host), dotfile/`%2e`/symlink out of the root → 404, `..` above the root → 400, POST to a site without an upstream → 405, SPA `try_files`, files mixed with an upstream with `strip_prefix` and `pass_thru`, `/status` |
| Response cache | miss → hit (`Cache-Status`), query and site in the key, `X-Forwarded-Host`/`-Proto` from a trusted peer in the key, `Set-Cookie`/`private`/no freshness information/`Authorization` not stored, POST → bypass, `default_ttl`, `Vary`, objects > `max_object_size` passed through whole without being stored, cache lock (5 concurrent → 1 to the upstream), `stale-while-revalidate` (stale, then refreshed in the background), `stale-if-error` while the upstream is down, purge by prefix and per site through the admin listener (and validation of its parameters), metrics, `/status`, access log |
| WebSocket and upgrades | WebSocket 101 with `Sec-WebSocket-*` reaching the upstream, 8 KiB back and forth on a site with `max_body_size: 1k`; a headscale-style upgrade (`tailscale-control-protocol`, `X-Tailscale-Handshake`) on `upgrade: any` arrives whole without `Keep-Alive`/`TE`, and in the default mode arrives as an ordinary request; `upgrade: off` refuses WebSocket; a fake `Upgrade` with a 4 KiB body is still 413; quiet connections closed after about 2 seconds per `upgrade_idle_timeout` or `read_timeout`, and kept open while data flows; a site with caching; a reload does not drop open connections; access log 101 with bytes and no error. Also checked by mutation: without the fixes, the test fails. Passed 4 out of 4 times |
| Pingora fork (`vendor/pingora`) | unit tests of `pingora-http` (58 + 3 doctests), `pingora-proxy` (65), and `pingora-core` (628, with OpenSSL), including new tests for the idle pool, the deadline for the whole request header (slowloris), the reuse check, the MIME check, hop-by-hop, `CasedHeaderName`, Pingora's own `Server` header, the response header handover (9 response shapes, bytes and keep-alive compared with the old path), and the HTTP/1 task channel (order, waiting for a slot, end of stream, receiver gone, 10,000 values across threads). All conduit tests above (86 unit, 9 e2e, ACME, CrowdSec) were repeated once with the fork and passed |
| clippy `-D warnings` | clean |

Follow-up daemon readiness smoke test (2026-11-10): the rebuilt PGO binary completed
255,136 requests with two daemonized graceful upgrades, 60 reloads, 27 backend outages,
nine access log rotations and five certificate rotations. Both upgrades were served by the
new PID and the old process exited. No panics, dead-process samples, dropped access log
lines or lost upstream reuses; peak RSS 49.1 MB. Eight POST requests returned 502 when the
backend was reset; 32 HTTP/2 operations were canceled across the two upgrades. All 80
long-lived WebSocket disconnects were during upgrades and reconnected; 836 short WebSockets
completed without failure. This smoke run validates the harness and daemon handover; it
does not replace the 24-hour run.

## Benchmark

The PGO/nginx comparison below records the 8 November build. HTTP/2 denotes the client
connection, with HTTP/1.1 upstreams. It is separate from the final before/after H2-feature
benchmark in [current local validation](#current-local-validation); backend H2 peak versus
H1 peak has not been compared.

`scripts/bench.sh`: the proxy is limited to 2 CPUs, the upstream and the load generator run on other
CPUs, 2 KB bodies, 64 connections, 50,000 requests per scenario, access log on in both proxies.
nginx runs twice: tuned (upstream keep-alive), and nginx without upstream keep-alive, proxying
through variables with one upstream connection per request, as generated nginx configs often do.

One run (evening of 2026-11-08, PGO release after the third round of Pingora patches, idle
machine; the middle run of 3). Ranges are in
[HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput). The gate baseline is still from
the first-round release.

| Scenario | Proxy | req/s | p99 | CPU per 10k req | Peak RSS |
|---|---|---:|---:|---:|---:|
| HTTP keep-alive | nginx without upstream keep-alive | 35,772 | 1.96 ms | 0.34 s | 19 MB |
| | tuned nginx | 98,917 | 0.77 ms | 0.15 s | 19 MB |
| | **conduit** | **77,006** | 1.16 ms | 0.20 s | 30 MB |
| HTTPS (HTTP/1.1) | tuned nginx | 74,518 | 0.95 ms | 0.21 s | 29 MB |
| | **conduit** | **57,982** | 2.52 ms¹ | 0.28 s | 33 MB |
| HTTP/2 | tuned nginx | 37,956 | 1.84 ms | 0.41 s | 29 MB |
| | **conduit** | **49,784** | 1.99 ms | 0.33 s | 36 MB |
| New TLS handshake per request | tuned nginx | 3,277 | 20.65 ms | 5.76 s | 30 MB |
| | **conduit** | **3,601** | 33.82 ms | 4.96 s | 37 MB |

¹ The other two runs: 1.48–1.49 ms.

Idle RSS with 100 sites, 2 hosts per site, plugins on every site, and 10 pools with health
checks: **25.6–26.6 MB** with the PGO release build (1 site: 21.4–22.6 MB), including the idle
ACME service. The upper numbers are from a re-measurement on 2026-11-08 with the latest release
binary (ten fork patches, stage A, the new access log). Build without PGO: 26.8–27.3 MB. The
serde-saphyr YAML parser adds about 0.8 MB compared with serde_yaml (measured with the same
config and comparison binary).

With CrowdSec (20 sites, a fake LAPI holding 70,200 decisions, 15.9 MB of JSON): idle RSS
19.6 MB without CrowdSec, **27.4 MB** with 70,200 decisions. Right after a full pull it briefly
reached 44 MB (the JSON response in memory), then went down by itself.

How to read these numbers:
- conduit is **about 2.2× nginx without upstream keep-alive** (req/s 2.15–2.18×, lower p99 and CPU),
  **faster than tuned nginx on HTTP/2** (1.29–1.32× req/s, CPU 0.80–0.82×), and **faster on TLS
  handshakes** (1.10× req/s, CPU 0.86–0.89×).
- On small HTTP/1.1 and HTTPS keep-alive requests, conduit reaches 0.76–0.81× of tuned nginx's
  throughput. The samply profile (see below) shows this gap is almost entirely inside Pingora.
- At ordinary traffic (far below 1,000 req/s) this overhead does not matter: both proxies add
  under 1 ms per request.

### Comparison with other reverse proxies

`scripts/compare.py` (`mise run compare`) runs nginx, Caddy, Apache httpd, HAProxy, Traefik,
Envoy, and conduit under the same load and setup:
- Every proxy runs in a container with the same 2 CPUs.
- All proxies use the same upstream, 2 KB bodies, 64 connections, 50,000 requests per run.
- 3 rounds with the proxy order rotated; the tables use the median.
- CPU and memory come from the container's cgroup. conduit also runs in a container
  (`debian:trixie-slim` with `dist/conduit`), so it is measured the same way.

The tables below were taken before the second round of HTTP/1.1 patches (see
[HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput)) and have not been re-run.

Every proxy is configured as it would be for production:
- 2 workers or threads.
- Keep-alive to the upstream. Checked by the number of new connections the upstream accepted: at
  most 74 per run (tuned nginx) for 50,000 requests. Nginx without upstream keep-alive opens one
  connection per request.
- TLS with the same ECDSA P-256 certificate, and HTTP/2 over TLS.
- `X-Forwarded-For` and `X-Forwarded-Proto` to the upstream.
- An access log line for every request, written to a file. Checked by counting lines: complete
  for every proxy, except HAProxy, which lost 0.2% of lines (HAProxy drops log lines when busy),
  and Apache, which still held 19 lines in its buffer when they were counted.

Versions: nginx 1.28, Caddy 2.11.9, Apache 2.4.69 (event MPM), HAProxy 3.4.7, Traefik 3.7.17,
Envoy 1.40.2 (BoringSSL), and conduit's PGO release with the Pingora patches. Measured on
2026-11-08 on an idle machine, 2 runs. The tables are from the second run. Ratios against nginx
in the first run differ by at most 0.02.

req/s, with the ratio against tuned nginx in parentheses:

| Proxy | HTTP/1.1 keep-alive | HTTPS keep-alive | HTTP/2 | New TLS connection per request |
|---|---:|---:|---:|---:|
| tuned nginx | 101,079 | 74,070 | 37,906 | 3,313 |
| HAProxy | 75,048 (0.74×) | 56,834 (0.77×) | **54,092 (1.43×)** | 2,932 (0.88×) |
| **conduit** | **69,776 (0.69×)** | **51,323 (0.69×)** | **43,983 (1.16×)** | **3,510 (1.06×)** |
| nginx without upstream keep-alive | 35,199 (0.35×) | | | |
| Apache httpd | 33,689 (0.33×) | 25,426 (0.34×) | 13,686 (0.36×) | 2,183 (0.66×) |
| Envoy | 33,389 (0.33×) | 30,344 (0.41×) | 26,912 (0.71×) | **5,178 (1.56×)** |
| Caddy | 18,457 (0.18×) | 16,457 (0.22×) | 12,298 (0.32×) | 3,271 (0.99×) |
| Traefik | 17,596 (0.17×) | 15,182 (0.20×) | 11,139 (0.29×) | 3,283 (0.99×) |

CPU per 10,000 requests, and memory:

| Proxy | HTTP/1.1 | HTTPS | HTTP/2 | New TLS | Added p99 at 8,800 req/s | Idle memory | Peak memory |
|---|---:|---:|---:|---:|---:|---:|---:|
| tuned nginx | 0.15 s | 0.20 s | 0.41 s | 5.68 s | +0.09 ms | 9.1 MB | 10–13 MB |
| HAProxy | 0.21 s | 0.28 s | 0.30 s | 6.40 s | +0.08 ms | 19.6 MB | 22–26 MB |
| **conduit** | 0.23 s | 0.30 s | 0.35 s | 5.19 s | +0.12 ms | 9.1 MB | 23–26 MB |
| nginx without upstream keep-alive | 0.35 s | | | | +0.44 ms | 8.7 MB | 9 MB |
| Apache httpd | 0.53 s | 0.70 s | 1.34 s | 8.66 s | +0.39 ms | 6.0 MB | 38–47 MB |
| Envoy | 0.54 s | 0.58 s | 0.66 s | 3.53 s | +0.44 ms | 14.3 MB | 21 MB |
| Caddy | 1.00 s | 1.11 s | 1.47 s | 5.75 s | +2.85 ms | 9.7 MB | 28–29 MB |
| Traefik | 1.06 s | 1.22 s | 1.64 s | 5.73 s | +3.24 ms | 13.5 MB | 44–47 MB |

"Added p99" is measured at a fixed rate of 8,800 req/s for 10 seconds, 50% of the HTTP peak of
the slowest proxy (Traefik), so no proxy is saturated. The number is the p99 through the proxy
minus the p99 straight to the upstream (0.28 ms). Memory here is the cgroup's anonymous memory
(heap and stack, without binary pages), so it is smaller than the RSS elsewhere in this
document.

How to read this:
- conduit is in the fastest group with nginx and HAProxy. On HTTP/1.1 conduit is about 2× Apache
  and Envoy, and about 3.8× Caddy and Traefik.
- HTTP/1.1 and HTTPS: nginx is fastest, HAProxy at 0.74–0.77×, conduit at 0.69×. The cause of
  conduit's gap is discussed in [HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput).
- HTTP/2: HAProxy is fastest (1.43× nginx), conduit second (1.16×), nginx third.
- New TLS connections: Envoy is far ahead (1.56×), probably thanks to BoringSSL; this was not
  investigated further. conduit 1.06×; nginx, Caddy, and Traefik about equal; HAProxy 0.88×;
  Apache 0.66×.
- Latency under moderate load: nginx, HAProxy, and conduit add ≤0.12 ms of p99. Apache, Envoy,
  and nginx without upstream keep-alive about 0.4 ms. Caddy and Traefik (Go) add about 3 ms.
- Idle memory is small for every proxy (6–20 MB). Under load, Apache and Traefik grow the most.
- Limits: this is a synthetic load (2 KB bodies, localhost, 2 CPUs). The configurations are
  reasonable for production but not tuned to the limit for each proxy. For example, Caddy and
  Traefik use their default JSON log format that records every header, and Caddy's log rotation
  is turned off like the other proxies'. At ordinary traffic, far below 1,000 req/s, all of
  these proxies are fast enough.

### Latency under load

A fixed rate for 10 seconds (`oha -q`, `--latency-correction`): latency is counted from each
request's scheduled send time, so queueing time is measured too. Overhead = p99 through the proxy
minus p99 straight to the upstream. Numbers from `scripts/bench.sh` (median of 3 rounds per run,
PGO release): 5 runs after the first round of Pingora patches (P1) and 3 runs after the third
round (P3). The rate follows conduit's peak in that run, so a faster release is tested at a
higher rate:

| Load (of conduit's HTTP peak) | Release | req/s | p99 straight to upstream | + tuned nginx | + conduit |
|---|---|---:|---:|---:|---:|
| 50% | P1 | 34,700–36,400 | 0.48–0.50 ms | +0.36–0.49 ms | **+0.60–0.81 ms** |
| | P3 | 38,400–39,200 | 0.52–0.53 ms | +0.49–0.64 ms | **+0.51–0.57 ms** |
| 70% | P1 | 48,600–50,900 | 0.60–0.61 ms | +0.70–0.82 ms | +1.03–1.17 ms |
| | P3 | 53,800–54,900 | 0.67–0.68 ms | +1.39–5.97 ms | +1.50–3.09 ms |

In P1, conduit's highest numbers (+0.81 and +1.17 ms) come from the run with the highest rate
(36,400 and 50,900 req/s); the other four runs +0.60–0.64 ms and +1.03–1.06 ms. In P3, 70% of
peak is already close to the limit of the test machine (proxy, upstream, and load generator share
8 CPUs), and both proxies' p99 widens and varies between runs. At the same rate, P3 beats the
second-round release: see [HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput).

Before the patches, at lower rates (about 28,400 and 39,800 req/s), conduit added
+0.79–0.85 ms and +1.41–1.42 ms (a single 10-second window, not a median).

- At the same absolute rate, nginx adds less, because that rate is only a quarter to a third of
  its capacity.
- At the same relative load (a percentage of each one's own peak, measured separately), conduit
  is steadier. Tuned nginx at 50/60/70% of its peak adds +2.99 / +1.05 / +12 ms of p99 (p99.9
  about 33 ms at 70%). conduit at 49/60/69% adds +0.69 / +1.05 / +1.3–1.4 ms. The access log
  accounts for only about 0.1 ms of that.
- Ordinary traffic (< 1,000 req/s) is roughly 2% of peak. At 9% of peak conduit adds
  +0.11 ms.

The regression gate in `scripts/bench.sh` fails when:
- any request fails;
- conduit adds more than `P99_OVERHEAD_MS` (default 1 ms) of p99 in the 50% scenario;
- conduit's req/s, taken as a ratio against tuned nginx in the same run, drops by more than
  `TOLERANCE` (default 10%) from `target/bench/baseline.json`;
- conduit's p99 rises by more than `P99_TOLERANCE` (default 25%) and by more than 0.2 ms from the
  baseline. The 70% scenario is guarded by this rule, and for the fixed-rate scenarios only p99 is
  compared. With a baseline, the 50% and 70% scenarios use the rate from the baseline, so p99 is
  compared under the same load. Without a baseline, or while recording one, the rates are 50% and
  70% of that run's peak.

The fixed-rate scenarios (50% and 70%) are repeated `RATE_ROUNDS` times (default 3), rotating
upstream, nginx, conduit, and the median is what is judged. A single 10-second window is easily
hit by a brief disturbance from the desktop: in 2 of 6 runs nginx and conduit spiked together
(nginx 2.4 ms and 10.9 ms, against a usual 1.0 ms or so).

Why the ratio and the p99 tolerance are separate: on an idle machine, three runs in a row with the
same binary lowered the req/s of nginx and conduit together by about 5% (the machine heats up),
while the ratio stayed put (HTTP/1.1 0.60–0.62×, HTTPS 0.64×). Saturated p99 varies by up to 15%
between runs (HTTP/2 2.74–3.16 ms). With the old rule (absolute numbers, 10%) the same binary
failed against its own baseline.

`UPDATE_BASELINE=1` saves a new baseline. Record it on an idle machine with a release (PGO) build:
`UPDATE_BASELINE=1 scripts/bench.sh dist/conduit`.

### Threads and runtime

`server.work_stealing: false` (the default) gives every worker thread its own Tokio runtime. A
connection is handed to one thread at accept and never moves, so there is no shared queue and no
task stealing between threads. `true` uses one runtime whose threads steal tasks from each other.
That mode evens out a few heavy long-lived connections (HTTP/2, WebSocket), at a cost in
throughput and latency for many ordinary connections.

A/B with `scripts/ab.sh` (7 alternating rounds, median, proxy on 2 physical cores). The numbers
are the change of `false` against `true`:

| Scenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, saturated | +5.9% | +8.8% | −8.2% |
| HTTPS keep-alive, saturated | +4.7% | +14.4% | −4.1% |
| HTTP/2, saturated | −2.0% | +25.9% | +1.5% |
| New TLS handshake, saturated | −3.6% | +22.9% | −1.8% |
| HTTP at a fixed rate, about 50% of peak (35k req/s) | same | **−6.4%** | −7.6% |
| HTTP at a fixed rate, about 70% of peak (50k req/s) | same | **−47.8%** (6.98 → 3.65 ms) | −5.1% |

The production host sits behind Cloudflare, and traffic from Cloudflare to the origin is almost
entirely HTTP/1.1 keep-alive, far below saturation. There `false` uses less CPU and its p99 is
much steadier. p99 under saturation is worse, because a thread that happens to get more
connections gets no help from the others.

With a runtime per thread, every socket is registered with the epoll of the runtime that opened
it. Pingora's upstream connection pool is shared by all threads, so a connection opened by thread
B can be used by a request on thread A. Every readiness event of that connection then wakes B,
and B wakes A through an eventfd: on average 0.97 eventfd `write()`s per request (counted with an
LD_PRELOAD syscall counter). Since 2026-11-08 conduit fills the peer's `group_key` with the
worker thread number (only with `work_stealing: false` and `threads` > 1). `group_key` is part of
the pool key, so each thread uses its own connections. The result is 0.19 eventfd writes per
request, and an A/B against the previous binary:

| Scenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, saturated | +4.0% | +19.9% (4 of 7 rounds about 1.3 ms, the rest about 1.0–1.1 ms) | −4.2% |
| HTTP with gzip, saturated | +2.0% | −3.6% | −1.1% |
| HTTPS keep-alive, saturated | +2.6% | −0.9% | −2.2% |
| HTTP/2, saturated | +1.0% | +0.3% | −4.5% |
| HTTP at a fixed 30k req/s | same | +5.4% (overlapping per-round ranges) | +2.1% |

The cost: up to `threads` times more idle connections to each backend, and the first request on
each thread opens a connection of its own.

Old note: before the fork, the Pingora task that watched an idle connection could run on another
thread, and sometimes (1–5 per 200,000 requests) Pingora opened a new connection with the ERROR
"failed to acquire reusable stream". Patch `79bf391` removed that task along with the message, so
`conduit_upstream_reuse_lost_total` is now always 0.

### PGO build

The release binary is built with profile-guided optimization (`scripts/pgo.sh`, called from the
`Dockerfile`):
1. A first, instrumented build.
2. That binary serves the training load of `scripts/pgo-train.sh` (about 95,000 requests, about
   5 seconds). The load follows the production host's traffic: mostly HTTP/1.1 keep-alive from
   Cloudflare through the proxy, plus HTTPS, HTTP/2, new TLS handshakes, plugins, cache, the file
   server, compression, redirects, 404/403, and one reload.
3. A second build uses that profile, so LLVM lays out code and chooses what to inline based on
   the paths that are actually hot, not on guesses.

A/B with `scripts/ab.sh` (7 alternating rounds, median, `work_stealing: false`, proxy on
2 physical cores). The numbers are against the binary from before the last code optimizations.
"Code" = optimizations in conduit (a single-backend pool without allocation, request IDs without
`format!`, `X-Forwarded-For` and `X-Real-IP` sharing one value), "PGO" = code + PGO:

| Scenario | req/s code | req/s PGO | p99 PGO | CPU per request PGO |
|---|---:|---:|---:|---:|
| HTTP keep-alive, saturated | +0.9% | **+16.3%** (77,959 → 90,685) | −13.0% | −16.1% |
| HTTPS keep-alive, saturated | +0.5% | **+14.5%** | −13.7% | −14.7% |
| HTTP/2, saturated | −0.2% | **+13.7%** | −13.5% | −13.3% |
| New TLS handshake | +1.2% | +5.8% | −1.9% | −3.9% |
| HTTP at a fixed 30k req/s | same | same | **−24.5%** (1.10 → 0.83 ms) | −15.8% |

PGO wins every round: on HTTP, HTTPS and HTTP/2, the slowest PGO round is still faster than the
fastest baseline round. The code optimizations alone are within noise, because conduit's hot path
is only about 13% of CPU. Handshakes gain the least, because most of their time is in OpenSSL (C),
which is not instrumented.

Deliberately not used:
- `panic = "abort"`: one panic in one request would kill the whole proxy. With unwinding, Pingora
  runs each connection as its own Tokio task, and Tokio catches a panic in that task: only that
  connection drops.
- Other thread-per-core runtimes (monoio, glommio): that would mean leaving Pingora. A runtime per
  thread inside Pingora is already available through `work_stealing: false`.
- Per-request arenas (bumpalo): the remaining allocations are inside Pingora and `http`, not in
  conduit's code.

`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip` and mimalloc have been on from the
start. The benchmark tables above come from the PGO release (`dist/conduit`).

### CPU profile

Measured with samply (4 kHz, 150,000 HTTP/1.1 requests, access log on, plain build). Breakdown of
worker CPU:

| Part | Stock Pingora | After the third round of patches |
|---|---:|---:|
| Syscalls (mostly `send`, including the kernel TCP stack) | 36% | 44% |
| Pingora + tokio in user space | ~51% | ~46% |
| conduit's code (hooks, plugins, access log) | ~13% | ~10% |

The syscall share went up because user space shrank; the number of syscalls per request is the
same.

With stock Pingora, Pingora's share was spread over allocations (mimalloc about 10% of all
samples), a double `HeaderMap` to keep header name case, header clones, two channels per request,
and memcpy of async futures. conduit's biggest code was `upstream_request_filter` (5%; Pingora's
`insert_header` parsed the header name every time) and the access log (3.5–4%).

After the third round, the double `HeaderMap` and the response header clone are gone, the channel
dropped from 3.1% to 1.5%, `upstream_request_filter` is at 3.3%, and `logging` at 1.4% (histograms
and the access log line; writing to the file is done by another thread). No single spot in user
space is above about 1.5% any more: the rest is allocations, memcpy of futures, and Pingora's proxy
glue code.

A finding from this profile that is fixed: Pingora's default L4 write buffer was only 1460 bytes,
so the header and body of a 2 KB response went out in two separate `send()`s and the client sent
an ACK in between: 6 TCP segments per request, where nginx needs 4. With
`server.write_buffer: 16k` conduit now also needs 4 segments per request, HTTP throughput went up
11% and sys time down 18%.

A new profile on 2026-11-08 showed Pingora's upstream connection pool taking about 5.4% of samples
(a spawned task and a timer per request when a connection was returned, `getpeername()` on reuse)
and the tokio scheduler about 6.4%. After the [Pingora patches](../vendor/README.md) those are
about 3.6% (mostly freeing response buffers, which has to happen anyway) and about 4.0%. The other
parts are as in the table above.

### Memory notes

With the kernel policy `transparent_hugepage=always` (as on the production host), mimalloc's
arenas fill up with 2 MB huge pages and idle RSS almost doubles. conduit turns THP off for its own
process (`server.transparent_huge_pages: false`). Turning it on gives about 1% more throughput.
The health checks of all pools share one set of connectors, because every OpenSSL connector loads
the whole system CA store. The 16 KiB write buffer per connection only fills as far as the
response that passes through.

## Commands

```sh
mise run test                 # unit + e2e
mise run test-acme            # ACME end-to-end with Pebble (needs Docker)
mise run test-crowdsec        # CrowdSec end-to-end with a real CrowdSec (needs Docker)
mise run lint                 # fmt + clippy
mise run release              # dist/conduit (PGO), Ubuntu 22.04 / glibc 2.35 baseline, via Docker
mise run pgo                  # local PGO build into target/pgo/conduit (needs rustup llvm-tools)
mise run bench                # conduit vs nginx (tuned and without upstream keep-alive), Docker
mise run compare              # dist/conduit vs nginx, Caddy, Apache, HAProxy, Traefik, Envoy (Docker)
mise run ab base=BIN new=BIN  # A/B of binaries or settings (e.g. new=BIN,work_stealing=true), needs Docker
scripts/soak/deploy.sh HOST [HOURS]  # soak test of dist/conduit on a disposable test machine (see below)
fuzz/run.sh [SECONDS]         # all fuzz targets in parallel, default 300 seconds (needs nightly + cargo-fuzz)

conduit check -c conduit.yaml
conduit run -c conduit.yaml [-d] [-u]
scripts/parity.py CONFIG NPM_IP CONDUIT_IP --http-port 80,18080 --https-port 443,18443
```

`mise` test/lint/build entry points prepare the local Pingora backport. Direct Cargo
commands need preparation first, also after resetting the submodule:

```sh
bash scripts/prepare-vendor.sh
cargo test --locked --all-targets
cargo test --locked --lib -- --ignored --nocapture
cargo test --locked --test e2e upstream_http2_ -- --nocapture
cargo test --locked --test e2e slow_request_head_is_cut_off -- --ignored --nocapture
OHA_BIN=/path/to/oha cargo test --locked --test e2e upstream_http2_load_stability -- --ignored --nocapture
```

ACME and CrowdSec use the dedicated mise tasks above. For the native H2 pool race command,
see [the vendor test instructions](../vendor/README.md#tests). Reusing a release binary for
the H2 tests is supported by `CONDUIT_HTTP2_TEST_BIN`; `CONDUIT_H2_REPORT` selects the soak
JSON output path. The H2 soak requires oha 1.16.0. Default `cargo test` does not run these
explicitly ignored cases.

Fuzzing (`fuzz/`) uses cargo-fuzz. The targets live in `src/fuzzing.rs`, which is why conduit is
built as a library plus a thin binary: the fuzz targets can call internal functions. Seeds are in
`fuzz/seeds/`, while the growing corpus (`fuzz/corpus/`) and crashes (`fuzz/artifacts/`) are not
committed. Every target also checks the properties the code promises, not only the absence of
panics:

| Target | Property checked |
|---|---|
| `config` | YAML parser; accepted configs are then compiled (IP backends only, no DNS) |
| `path` | the normalized result is always an absolute path without empty, `.`, or `..` segments |
| `access_log` | the hand-written JSON is identical to serde's and stays on one line |
| `request` | lower-case host, real IP only from trusted peers, answer status 100–599, a target without a path → 400, ranges within the file |
| `route` | the target produced by the `proxy` action (`strip_prefix`, `add_prefix`, `rewrite`) always starts with `/`, holds only path characters or `%XX` escapes, keeps the query unchanged, and is accepted by Pingora |
| `body_replace` | the replacement result is identical however the body is cut into chunks (1–7 bytes, alternating) |
| `block_exploits` | any input as query, path, user agent and header: no panic, and the keywords that select rules never hide a match (results with and without keywords are always the same) |

The soak test (`scripts/soak/`) runs on a disposable test machine as a transient systemd unit,
`conduit-soak`. The script installs nginx and adds an nftables table on that machine, and all its
services listen only on 127.0.0.1. It consists of:

- **Backends:** nginx on two ports and a WebSocket echo server.
- **Load:** about 650 req/s through oha in 5-minute windows (HTTP/1.1 GET of 256 KiB with gzip and
  POST, HTTPS, HTTP/2, a new TLS handshake per request), plus 40 long-lived WebSockets and one
  short WebSocket every 0.5 seconds.
- **Periodic events:**

  | Event | Interval |
  |---|---|
  | Reload | 5 minutes |
  | One backend refused for 30 seconds | 10 minutes |
  | Access log rotated | 30 minutes |
  | Certificate rotated | 1 hour |
  | Graceful upgrade (`run -d -u` then SIGQUIT, like `systemctl reload`) | at 2 and 6 hours (`SOAK_UPGRADE_AT`), so the last process runs 18 hours |

  Flaps fall in the middle of every other oha window and upgrades in a window without a flap
  or a reload close by, so each failure can be put down to one cause.

- **Samples every 30 seconds:** RSS, fds, threads, CPU, connections, and metrics.
- **Access log:** lines with an error or a 4xx/5xx status are kept from every rotation, to
  trace failed requests on conduit's side.

The results are in `~/soak/out/` on that machine. Before another run starts, its output and
runtime files are preserved in a unique directory under `~/soak/archive/`. Results include
a summary in `report.txt`: hourly medians, RSS per process (15 minutes in, at the end, peak, and
trend in MB per hour, since upgrades start a new process), failed requests grouped by the
disruption in their window, and the kept access log lines. For a smoke test of the harness,
`SOAK_SCALE=40 SOAK_SECONDS=420 SOAK_UPGRADE_AT="1 3"` speeds up every interval and includes
two upgrades within seven minutes.
The harness aborts if a graceful upgrade fails to replace the old process or the admin
endpoint is not served by the new PID.

A full example config with every option: [`conduit.example.yaml`](../conduit.example.yaml).

The config is YAML 1.2, read with [serde-saphyr](https://github.com/bourumir-wyngs/serde-saphyr)
(pure Rust, panic-free, with parsing limits for anchors and aliases):

- only `true`/`false` are booleans; `yes`, `no`, `on` are refused in boolean fields, and `off`
  stays an ordinary word (`upgrade: off`)
- unknown fields and duplicate keys are refused
- a section whose content is all commented out (just `default_site:`) is the same as leaving it
  out
- anchors (`&base`) and merge keys (`<<: *base`) can be used for similar sites
- errors give the line and column without quoting the file's content (the file holds secrets, and
  reload errors go to the log and to `/status`)

The systemd unit and logrotate file: [`deploy/`](../deploy/). The CrowdSec parser and
acquisition: [`deploy/crowdsec/`](../deploy/crowdsec/).

## Roadmap

The [management and Web UI roadmap](roadmap.md) records the agreed next milestones:
headless by default, an optional Hallmark-designed UI, SQLite-backed management state,
local authentication and generic OIDC (including tested Authentik/ZITADEL integrations),
and one revision-controlled change coordinator for UI/API/CLI and YAML imports. It specifies
conflict rejection, durable apply/recovery, audit, and E2E/stability/performance release gates.
These are planned requirements; they do not describe existing admin capabilities.

- **Phase 1 (done 2026-11-08):** the HTTP/1.1 and HTTPS throughput target was changed to ≥0.75×
  tuned nginx or CPU per request ≤1.4×. Ten Pingora patches (three rounds and stage A) plus the new
  access log took it from about 0.6× to 0.76–0.81×; see
  [HTTP/1.1 and HTTPS throughput](#http11-and-https-throughput). Patches that prove themselves can
  be proposed to upstream Pingora ([`vendor/README.md`](../vendor/README.md)).
- **Phase 2 (done):** built-in ACME, CrowdSec bouncer + AppSec, rate and connection limits, mTLS,
  `basic_auth` and `forward_auth`. Added: a static file server and a response cache.
- **Phase 3 (historical checklist completed 2026-11-09): core stability.** Core = routing, TLS, WebSocket/upgrades,
  reloads and graceful upgrades, upstream pools and health checks, the access log. Order:
  1. ✓ Upstream flapping under load (done 2026-11-08, `upstream_flapping` in
     [Test results](#test-results-local)). Finding: upstream timeouts were answered with 502, now
     504.
  2. ✓ A mixed-load soak test (HTTP/1.1, HTTPS, HTTP/2, WebSocket, periodic reloads), 24 hours
     (done 2026-11-09, see [Test results](#test-results-local); harness `scripts/soak/`, see
     [Commands](#commands)). RSS, fds, threads and upstream connections stayed flat, 0 panics,
     every failed request explained. It ran a build from before the 504, `keepalive_timeout` and
     request header deadline fixes, which are covered by their own tests.
  3. ✓ Fuzzing (`fuzz/`, done 2026-11-09): the config parser, path normalization, the access log,
     and requests (host, real IP, routing, plugins, rules, LB hashing, header parser). Finding: a
     target without a path (`GET ?x`) reached the plugins as `/`, now 400.
  4. ✓ Audit of panics and misbehaving clients (done 2026-11-09). 33 `unwrap`/`expect` in non-test
     code, none depending on input (mutex poisoning, invariants, startup). Findings: idle
     keep-alive connections were never closed (now `keepalive_timeout`), and slowloris could hold
     connections forever (Pingora patch `7ffe1c4`). Running out of fds and large headers were
     already safe.
  5. ✓ Graceful upgrade with open WebSockets (done 2026-11-09). Finding: connections dropped when
     the old process stopped were missing from the access log; they are logged now.
  6. ✓ Local parity test (done 2026-11-09, see [Test results](#test-results-local)). The copy of the
     nginx configs on the laptop is identical to the production host's (checksums). The only
     remaining behavior differences are in the nginx exploit-blocking rules (on for 38 of 51
     hosts), now covered by the `block_exploits` plugin: conduit no longer has their false
     positives, and it catches attacks they let through.
- **Completed after the core checklist (2026-11-10):** audit fixes for decoding, descriptor
  containment, authority/encoding cache identity, checked durations, watcher validation,
  serialized admission and wildcard certs; HTTP/2 upstream with TLS/h2c, gRPC streams and
  trailers, protocol isolation, stream limits and the local Pingora pool backport. See
  [current local validation](#current-local-validation).
- **Next, planned:** milestones 4–6 in the [management roadmap](roadmap.md#milestones):
  authenticated management foundation, OIDC and monitoring UI, then configuration editing.
  The configuration mutation API is part of that work; the current admin API already exposes
  health/status/metrics/cache purge. UI design and implementation must use Hallmark.
- **Deferred:** session ticket keys shared between processes, OCSP, OpenTelemetry,
  CrowdSec captcha, DNS-01. CrowdSec and AppSec are not developed
  further; what exists stays, and is optional during migration. Distribution packages
  (`.deb` and runnable containers) remain separate from the current source and documentation validation.
- **Optional:** an HTTP/1→HTTP/1 fast path in the Pingora fork, in case parity with nginx is ever
  needed.
