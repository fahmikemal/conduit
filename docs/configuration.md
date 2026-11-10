# conduit configuration reference

This is the complete reference for conduit's configuration file: every section and option,
its default, how it is validated, and what it does to a request. The rules engine and each
plugin are described in depth, with examples. [`conduit.example.yaml`](../conduit.example.yaml)
is the same reference in compact form, as a commented configuration file.

Implementation reference: commit `03cae2b` (10 November 2026). The
[technical guide](technical.md), [audit index](audit/README.md) and
[HTTP/2 validation](audit/HTTP2_UPSTREAM_VALIDATION.md) describe the evidence behind these
contracts. Native builds must first run `bash scripts/prepare-vendor.sh`.

The optional Web UI, SQLite management store, admin OIDC and configuration editing API are
planned in the [management roadmap](roadmap.md). They are not configuration options in the
current release; the admin listener described below still requires its existing private-network
deployment policy.

Contents:

1. [The configuration file](#1-the-configuration-file)
2. [How a request is handled](#2-how-a-request-is-handled)
3. [Global sections](#3-global-sections): `server`, `listen`, `admin`, `defaults`,
   `access_log`, `real_ip`, `tls`, `default_site`, `crowdsec`, `cache`
4. [Upstream pools](#4-upstream-pools)
5. [Sites](#5-sites)
6. [Match conditions](#6-match-conditions)
7. [The `rules` plugin](#7-the-rules-plugin)
8. [The other plugins](#8-the-other-plugins)
9. [Static files](#9-static-files)
10. [Response cache](#10-response-cache)
11. [TLS, ACME and client certificates](#11-tls-acme-and-client-certificates)
12. [Headers conduit adds and passes](#12-headers-conduit-adds-and-passes)
13. [Protocol upgrades (WebSocket and others)](#13-protocol-upgrades-websocket-and-others)
14. [Compression](#14-compression)
15. [Responses conduit generates](#15-responses-conduit-generates)
16. [Access log](#16-access-log)
17. [Admin endpoints and metrics](#17-admin-endpoints-and-metrics)
18. [Coming from nginx](#18-coming-from-nginx)

## 1. The configuration file

conduit reads one YAML file, `/etc/conduit/conduit.yaml` unless `-c` names another.

```sh
conduit check -c conduit.yaml       # validate, load every certificate, print a summary
conduit run -c conduit.yaml         # run in the foreground
conduit run -c conduit.yaml -d      # daemonize (what the systemd unit does)
conduit run -c conduit.yaml -d -u   # graceful upgrade: take over a running conduit's sockets
```

`conduit check` builds the configuration exactly as `run` would (it resolves backend host
names, reads certificates, files, htpasswd files and CA files) and prints the sites, pools,
certificates, CrowdSec and ACME state, followed by any warnings. It exits non-zero on an
error.

### Parsing rules

- The file is YAML 1.2. Only `true` and `false` are booleans: `yes`, `on` or `off` are
  strings, so `force_https: yes` is an error rather than a silent true.
- Unknown keys are errors, everywhere. A typo fails `conduit check` instead of being ignored.
- A key written twice in the same map is an error that names its line and column.
- A section left empty (all of its content commented out) counts as that section's defaults,
  as if it were not written.
- Error messages name the line and column but never quote the file, which may hold secrets
  (an API key, a password hash); errors go to the log and to `/status`.

### Value formats

| Kind     | Written as                                              | Examples                          |
|----------|---------------------------------------------------------|-----------------------------------|
| Duration | a number followed by `ms`, `s`, `m`, `h` or `d`; a bare number is seconds | `500ms`, `30s`, `10m`, `1h`, `30d`, `90` |
| Size     | a number followed by `k`, `m` or `g` (1024-based, either case); a bare number is bytes. `0` means unlimited | `512k`, `100m`, `2g`, `1048576`, `0` |
| Status   | an HTTP status code as a number                         | `403`, `429`                      |
| CIDR     | an address, or an address with a prefix length; a plain address is a /32 or /128 | `10.0.0.0/8`, `2001:db8::/32`, `192.0.2.7` |
| Header name | any valid HTTP field name, compared case-insensitively | `X-Api-Key`, `user-agent`       |

Identifiers (site ids and pool names) are letters, digits, `.`, `-` and `_`.
Duration multiplication that exceeds `u64` seconds is a configuration error, including on
reload; it does not wrap. Additional runtime limits are checked where required, such as a
positive `tls.watch_interval` that fits the runtime clock.

### When changes take effect

conduit rebuilds its configuration on `SIGHUP` and whenever a watched file changes on disk.
Files are checked every `tls.watch_interval` (60 s by default) by modification time, inode
and size, following symlinks (so a certbot renewal that swaps a symlink is seen). Watched
files are:

- the configuration file itself;
- every certificate and key in `tls.certs`, and the ACME certificate file of each site;
- `htpasswd` files of `basic_auth`;
- `mtls.ca` files;
- files served by the `file` action of rules;
- `default_site.page` and `crowdsec.ban_page`.

A new runtime is built completely before publication. Each request holds the runtime
snapshot it started with, so a reload does not replace its routing or plugin state mid-request.
Runtime, certificate store and process-wide settings are published separately; they are not
one global transaction across all handshakes and admin observations. If the new configuration
has an error, it is logged, recorded in `/status` and in
`conduit_config_reloads_total{result="failure"}`, and the running configuration stays.

Some settings are read only at start. Changing them needs a restart or a graceful upgrade
(`systemctl reload conduit` with the provided unit, which runs `conduit run -d -u`):

- `listen`, `admin` and `access_log.path` (a reload logs a warning when they changed);
- the whole `server` section and `tls.watch_interval`;
- `cache.lock_timeout` (a reload logs a warning).

State that survives a reload:

- an upstream pool whose definition is unchanged after DNS resolution keeps its health
  state, ejections and in-flight counts;
- `rate_limit` and `concurrency` keep their counts while the plugin's site, position in the
  plugin list and settings are unchanged;
- CrowdSec decisions are held per process and never dropped by a reload;
- cached responses stay, except those of sites that were removed or no longer have `cache:`.

A restart or graceful upgrade starts with empty caches, counts and decisions (the decisions
are pulled again at start).

## 2. How a request is handled

Every request goes through these steps in this order. Each step can answer the request
itself, which ends it.

1. **Site selection.** The host is taken from the `Host` header (HTTP/1) or `:authority`
   (HTTP/2), without its port and trailing dot, lowercased. A host with characters outside
   letters, digits, `-`, `.`, `_` (and brackets and colons for IPv6) counts as no host. An
   exact host name wins over a wildcard; among wildcards, the longest suffix wins.
2. **Client address and request id.** The client address is the TCP peer, or the address a
   trusted proxy states ([`real_ip`](#real_ip)). Each request gets an id
   ([headers](#12-headers-conduit-adds-and-passes)).
3. **Request target check.** The target must start with `/` (or be `OPTIONS *`); anything
   else, such as `GET ?x`, is answered with 400, as nginx does.
4. **ACME challenges.** `/.well-known/acme-challenge/<token>` is answered on every host,
   before anything below (including `force_https`): first from conduit's own ACME orders,
   then from `tls.acme_webroot` when it is set.
5. **Unknown host.** No site for the host: `default_site.page` with 200 for GET and HEAD,
   404 otherwise.
6. **`force_https`.** On a plain HTTP connection, a 301 to `https://<host><original target>`.
7. **Client certificate (`mtls`).** A required certificate that is missing or invalid: 403.
8. **Declared body size.** A `Content-Length` above `max_body_size`: 413.
9. **Plugins**, in the order written. The path is decoded and normalized first when any
   plugin needs it (`rules`, `block_exploits`, or any plugin with `match:`); a path that
   cannot be normalized is answered with 400. A plugin that has to wait (`basic_auth`
   verifying a new password, `forward_auth` asking its service) is awaited in its place in
   the chain, so the written order always holds.
10. **File/cache eligibility.** Determine whether to attempt file lookup or enable the
    response cache after the plugin chain. File lookup and serving happen after AppSec
    in step 12.
11. **CrowdSec AppSec**, for sites with `crowdsec: { appsec: true }`, once every plugin let
    the request through.
12. **Answer:** a file, or the request is proxied: a backend is chosen from the pool (a
    `proxy` rule's or the site's), the request headers are set, and the request is sent.
    A body larger than `max_body_size` without `Content-Length` (chunked) is cut off with
    413 once it passes the limit.
13. **Response.** Every response of a site, proxied, cached, from a file, generated by a
    plugin or an error, gets the `Server` header, the plugins' response header changes,
    `Strict-Transport-Security` (HTTPS only), `x-request-id`, and `Cache-Status` on sites
    with a cache. Compression and `body_replace` apply to proxied bodies.
14. **Log.** One access log line and the metrics, after the response is complete (for an
    upgraded connection, when it closes).

Rules and other plugins never change what the client sent unless they say so: a request
that no `proxy` rule rewrites goes upstream with its original, un-normalized target.

## 3. Global sections

### `server`

Process settings, read at start only.

| Key | Default | Description |
|-----|---------|-------------|
| `threads` | `2` | Worker threads shared by all listeners. |
| `work_stealing` | `false` | `false`: one single-threaded runtime per thread; a connection stays on the thread it was handed to, which gives more requests per second and a lower tail latency under load. `true`: one runtime whose threads steal work from each other, which evens out a few heavy long-lived connections (HTTP/2, WebSocket) at some cost to throughput. |
| `pid_file` | `/run/conduit/conduit.pid` | PID file, used by graceful upgrades. |
| `upgrade_sock` | `/run/conduit/upgrade.sock` | Unix socket over which a new process takes the listening sockets of the old one during a graceful upgrade. |
| `error_log` | unset | Where the daemon's stderr goes. Unset: stderr (the journal under systemd). |
| `upstream_keepalive_pool_size` | `128` | Idle upstream connections kept per worker thread for reuse. |
| `shutdown_grace` | `60s` | How long a stopping process (SIGTERM) or one being replaced (graceful upgrade) keeps serving in-flight requests. |
| `shutdown_timeout` | `5s` | Extra time for worker runtimes to wind down after the grace period. |
| `transparent_huge_pages` | `false` | `true` lets the kernel back the heap with transparent huge pages: about 1% more throughput, roughly twice the idle memory. |
| `write_buffer` | `16k` | Per-connection write buffer on the listeners. A response whose header and body fit leaves in one write and one packet. A `Content-Length` body is flushed when complete or when the buffer fills; chunked bodies are flushed chunk by chunk. `0`: unbuffered. |

### `listen`

| Key | Default | Description |
|-----|---------|-------------|
| `http` | `[]` | Addresses for plain HTTP, as `host:port`: `0.0.0.0:80`, `[::]:80`, `127.0.0.1:8080`. |
| `https` | `[]` | Addresses for HTTPS. HTTP/2 is offered over ALPN next to HTTP/1.1. |

At least one address is required. `[::]` listens on IPv6 only, so `0.0.0.0:443` and
`[::]:443` can be listed together on dual-stack hosts. These client-facing listeners do not
offer cleartext HTTP/2 (h2c). Upstream `h2c://` is supported separately in
[upstream pools](#4-upstream-pools). Read at start only.

### `admin`

| Key | Default | Description |
|-----|---------|-------------|
| `listen` | unset (off) | Address of the operator endpoints (`/metrics`, `/healthz`, `/status`, cache purge), e.g. `127.0.0.1:9180`. There is no authentication: bind it to loopback or a management network. See [section 17](#17-admin-endpoints-and-metrics). |

### `defaults`

Defaults for every site; most can be overridden per site.

| Key | Default | Per site | Description |
|-----|---------|----------|-------------|
| `connect_timeout` | `10s` | yes | Time to establish a connection to a backend. For `https://` backends, the TCP connection and the TLS handshake together get twice this. |
| `read_timeout` | `90s` | yes | Longest wait for each read from a backend (the response header, then each piece of the body). |
| `write_timeout` | `90s` | yes | Longest wait for each write to a backend. |
| `upstream_idle_timeout` | `60s` | no | How long an idle upstream connection waits in the pool for reuse. |
| `keepalive_timeout` | `75s` | no | How long an idle HTTP/1.1 client connection waits for its next request, in whole seconds (at least 1). A client's own `Keep-Alive: timeout=` is not followed. |
| `max_body_size` | `2000m` | yes | Largest request body. `0`: unlimited. See [step 8 and 12](#2-how-a-request-is-handled). |
| `compression` | `true` | yes | Compress text-like responses the upstream left uncompressed. See [section 14](#14-compression). |
| `server_header` | `conduit` | no | The `Server` header of every response, sent exactly as written; it replaces the upstream's. `off` (or `false`): conduit sends none, and the upstream's passes through. 1 to 64 printable ASCII characters without surrounding spaces. |

### `access_log`

| Key | Default | Description |
|-----|---------|-------------|
| `path` | unset (off) | File receiving one JSON object per request. See [section 16](#16-access-log). Read at start only. |
| `queue_capacity` | `65536` | Lines buffered in memory before new lines are dropped (and counted in `conduit_access_log_dropped_total`), split evenly across worker threads, at least 1024 each. |

The file is reopened by itself after logrotate moves it; no signal is needed.

### `real_ip`

The address conduit treats as the client: for `ip_filter`, limits, CrowdSec, `X-Real-IP`,
`X-Forwarded-For`, `forward_auth` and the access log. It follows nginx's realip module.

| Key | Default | Description |
|-----|---------|-------------|
| `header` | `X-Real-IP` | Header holding the client address. Sites can override it with `real_ip_header` (for example `CF-Connecting-IP`). |
| `trusted` | `[]` | Peers whose header is believed: CIDRs, or `cloudflare` for Cloudflare's published ranges. |
| `recursive` | `true` | For a comma-separated header such as `X-Forwarded-For`: walk it from the right and take the first address that is not trusted. `false`: take the rightmost entry. |

How the client address is resolved:

1. The TCP peer is not in `trusted`: the peer is the client, and the header is ignored.
2. The peer is trusted and the header is absent: the peer is the client.
3. The peer is trusted and the header is present:
   - `recursive: false`: its rightmost entry is the client;
   - `recursive: true`: entries are taken from the right; each trusted entry is skipped,
     and the first untrusted one is the client. An entry that is not an address stops the
     walk at the last address taken.

IPv4-mapped IPv6 addresses (`::ffff:192.0.2.7`, from dual-stack sockets) count as IPv4
everywhere. A trusted peer may also pass `X-Forwarded-Proto`, `X-Forwarded-Host` and
`x-request-id` through ([section 12](#12-headers-conduit-adds-and-passes)).

Behind Cloudflare:

```yaml
real_ip:
  trusted: [cloudflare]
sites:
  - id: www
    hosts: [www.example.com]
    upstream: http://127.0.0.1:3000
    real_ip_header: CF-Connecting-IP
```

### `tls`

| Key | Default | Description |
|-----|---------|-------------|
| `certs` | `[]` | Certificates: `{ name, cert, key }`. `cert` is a PEM chain, leaf first; `key` must match it. Names must be unique. |
| `default_cert` | unset | Name of the certificate served when the SNI matches no certificate (or there is none). Without it such handshakes fail. |
| `acme_webroot` | unset | Directory served at `/.well-known/acme-challenge/` on every host, for an external ACME client (certbot `--webroot`). Only file names of letters, digits, `-` and `_` (up to 256) are served. |
| `watch_interval` | `60s` | How often watched files are checked for changes ([section 1](#when-changes-take-effect)). Must be positive and fit the runtime clock. Read at start only. |
| `acme` | unset | Automatic certificates for sites with `acme: true`. See [section 11](#acme). |

### `default_site`

| Key | Default | Description |
|-----|---------|-------------|
| `page` | unset | HTML file returned with 200 for GET and HEAD on hosts that match no site. Other methods, and every request without a page, get 404. Read at load and watched. |

### `crowdsec`

The CrowdSec bouncer, used by sites with the [`crowdsec` plugin](#crowdsec-1). Without this
section the plugin is an error.

| Key | Default | Description |
|-----|---------|-------------|
| `lapi_url` | `http://127.0.0.1:8080` | CrowdSec local API base URL (`http://` or `https://`). |
| `api_key` / `api_key_file` | unset | The bouncer key (`cscli bouncers add conduit`). Exactly one of the two is required. |
| `update_interval` | `10s` | How often new and expired decisions are pulled. At least `1s`. |
| `origins` | `[]` | Only decisions from these origins (`crowdsec`, `cscli`, `CAPI`, `lists`, ...). Empty: all. |
| `scenarios_containing` | `[]` | Only decisions whose scenario contains one of these strings. |
| `scenarios_not_containing` | `[]` | Leave out decisions whose scenario contains one of these strings. |
| `ca_file` | unset | CA certificate (PEM) for an `https://` local API or AppSec URL, instead of the system store. |
| `ban_page` | built-in page | HTML file returned with 403 to refused clients. |
| `appsec` | unset | The AppSec component (WAF), for sites with `crowdsec: { appsec: true }`. |

`crowdsec.appsec`:

| Key | Default | Description |
|-----|---------|-------------|
| `url` | `http://127.0.0.1:7422` | AppSec endpoint. |
| `timeout` | `200ms` | Longest wait for a verdict. Must be above 0. |
| `failure_action` | `allow` | When AppSec gives no verdict (timeout, error, unexpected status): `allow` or `block` the request. |
| `body_limit` | `64k` | Request bodies of declared length up to this size are sent along. At most `64k`. |

How decisions work:

- Decisions are pulled in stream mode: the full list at start, then only what changed every
  `update_interval`, into an in-memory table. Checking a client is a few hash lookups, with
  no network call on the request path.
- Only `ip` and `range` scopes are used; country and AS decisions are ignored. Ranges stay
  ranges (a /16 is one entry).
- Every decision type refuses the client. `captcha` is not served; such clients are refused
  like banned ones, which is CrowdSec's fallback for remediations a bouncer does not support.
- When the local API cannot be reached, the decisions already held keep applying until they
  expire, and the next successful pull fetches the full list again.
- A refused client gets 403 with the ban page and `Cache-Control: no-store`. The access log
  line says `"blocked_by": "crowdsec"` (or `"appsec"`).
- Clients are checked by their real address ([`real_ip`](#real_ip)), so behind Cloudflare a
  visitor is blocked, not Cloudflare.

How AppSec works: after every plugin of the site let the request through, conduit sends the
request's method, target, headers and client address to AppSec (as CrowdSec's
`X-Crowdsec-Appsec-*` headers), with the body when its length is declared, within
`body_limit`, and the client is not waiting for `100 Continue`. Chunked or larger bodies are
not sent; AppSec then judges the headers alone. A 200 verdict lets the request through; a
403 verdict refuses it with the status AppSec names (403 by default) and the ban page. The
body read for AppSec is forwarded upstream unchanged.

### `cache`

Memory for the response cache, shared by all sites with `cache:`. Sites cache nothing unless
they say so ([section 10](#10-response-cache)).

| Key | Default | Description |
|-----|---------|-------------|
| `max_size` | `64m` | Bodies and headers kept, all sites together; least recently used entries go first. `0`: unlimited. |
| `max_object_size` | `8m` | Larger responses are passed through without being stored. `0`: unlimited. |
| `lock_timeout` | `5s` | How long requests for something being fetched wait for that fetch instead of all going upstream at once. Read at start only. |

## 4. Upstream pools

A pool is a set of interchangeable backends. Pools are defined by name under `upstreams:`
and referred to by sites and `proxy` rules, or written inline where they are used.

```yaml
upstreams:
  web:
    backends:
      - http://10.0.0.11:8080
      - { url: http://10.0.0.12:8080, weight: 3 }
      - http://app.internal:8080        # one backend per address the name resolves to
    balance: least_conn
    retries: 1
    passive: { failures: 3, eject: 30s }
    health_check: { path: /healthz, interval: 5s, timeout: 2s, rise: 2, fall: 3 }
```

Anywhere a pool is expected (a site's `upstream:`, a `proxy` action's `upstream:`), it can
be written three ways:

```yaml
upstream: web                         # a pool name from `upstreams:`
upstream: http://127.0.0.1:3000       # a URL: a pool with this one backend and defaults
upstream:                             # an inline pool definition
  backends: [http://10.0.0.11:8080, http://10.0.0.12:8080]
  balance: round_robin
```

A string containing `://` is a URL; any other string is a pool name. Inline pools of a site
are named `site:<id>`, those of `proxy` rules `site:<id>:rule<plugin>.<rule>` (both numbered
from 1); these names appear in `/status`, metrics and logs.

| Key | Default | Description |
|-----|---------|-------------|
| `backends` | required | A list of `http://host[:port]`, `https://host[:port]` or `h2c://host[:port]` URLs, or `{ url, weight }`. No path. The port defaults to 80 (HTTP/h2c) or 443 (HTTPS). |
| `balance` | `round_robin` | `round_robin`, `random`, `least_conn` or `hash`. |
| `hash_key` | `client_ip` with `hash` | What `balance: hash` hashes; only allowed with `hash`. |
| `retries` | `1` | Further backends tried when connecting fails. |
| `passive` | `{ failures: 3, eject: 30s }` | Ejection after errors on live traffic. `failures: 0` turns it off. |
| `health_check` | unset (off) | Active probes. |
| `verify_tls` | `true` | Verify the certificates (chain and name) of `https://` backends. |
| `sni` | the URL's host | SNI and expected certificate name for `https://` backends. |
| `http_version` | `"1.1"` | `"1.1"`, `"2"`, or `auto`. HTTP/2 is required with `"2"`; `auto` negotiates HTTP/2 or HTTP/1.1 on TLS and uses HTTP/1.1 on plain HTTP. `h2c://` implies `"2"` when this key is omitted. |
| `http2_max_streams` | `128` | Maximum concurrent streams per HTTP/2 connection, further bounded by the upstream's SETTINGS. Range: 1 to 4294967295. |

### Backends and DNS

Host names are resolved when the configuration loads (at start and on each reload), not per
request. Every address a name resolves to becomes a backend of its own, with the weight of
the entry. Validation:

- the URL must start with `http://`, `https://` or `h2c://` and have no path (a trailing `/` is
  allowed);
- `weight` must be at least 1;
- the same address must not appear twice in a pool (also when two names resolve to it);
- a name that does not resolve is an error, and the reload keeps the running configuration.

Upstream requests use HTTP/1.1 over pooled keep-alive connections by default. HTTP/2 can be
selected per named or inline pool, independently of the protocol the client uses:

```yaml
upstreams:
  internal_grpc:
    backends: [h2c://127.0.0.1:50051]
    http2_max_streams: 128
  secure_service:
    backends: [https://api.internal:443]
    http_version: "2"             # require HTTP/2, including when ALPN is absent
  ordinary_web:
    backends: [https://web.internal:443]
    http_version: auto            # TLS ALPN: HTTP/2 preferred, HTTP/1.1 allowed
sites:
  - id: grpc
    hosts: [grpc.example.com]
    upstream: internal_grpc
```

`http_version: "2"` with an `http://` backend also uses cleartext HTTP/2 prior knowledge.
`h2c://` is the shorthand, including in `upstream: h2c://127.0.0.1:50051`. There is no HTTP/1
`Upgrade: h2c` negotiation, and cleartext HTTP/2 does not fall back to HTTP/1.1. An explicit
`http_version: "1.1"` or `auto` conflicts with an `h2c://` backend and is rejected.

### Upstream protocol behavior

| Pool policy | `http://` | `https://` | `h2c://` |
|---|---|---|---|
| omitted | HTTP/1.1 | HTTP/1.1 | required HTTP/2 prior knowledge |
| `"1.1"` | HTTP/1.1 | HTTP/1.1 | configuration error |
| `"2"` or numeric `2` | required HTTP/2 prior knowledge | required TLS HTTP/2 | required HTTP/2 prior knowledge |
| `auto` | HTTP/1.1 | ALPN HTTP/2 preferred, HTTP/1.1 allowed | configuration error |

With required TLS HTTP/2, a backend that does not negotiate H2 yields 502 before the
application request is sent. `auto` permits fallback. Changing the client protocol does not
change the pool policy.

HTTP health checks use the selected upstream protocol too; TCP checks inspect only the
transport connection. Protocol/stream-limit changes take
effect on reload; incompatible pooled connections are isolated. `/status` exposes each
backend's configured `http_version` (`auto` reports the policy, not the negotiated result).

gRPC request/response streaming and response trailers pass through HTTP/2. Clients must
also connect to conduit using HTTP/2 over TLS for native gRPC; cleartext HTTP/2 on the
client-facing listener is not implemented. Ordinary HTTP/1 clients can still use HTTP/2
backends for regular HTTP requests.

HTTP/1 upgrades such as WebSocket require an HTTP/1 upstream connection. `auto` chooses
HTTP/1.1 for these requests; required `"2"` rejects a forwarded upgrade with 400. HTTP/2
extended CONNECT (RFC 8441) is not implemented.

### Balancing

| `balance` | Behavior |
|-----------|----------|
| `round_robin` | Backends in turn, in proportion to their weights. |
| `random` | A random backend, in proportion to the weights. |
| `least_conn` | The backend with the fewest requests in flight relative to its weight; ties rotate. |
| `hash` | Consistent (ketama) hashing of `hash_key`: the same key keeps going to the same backend, and adding or removing a backend moves only a share of the keys. |

`hash_key` values:

| Key | Hashes |
|-----|--------|
| `client_ip` (default) | the client address, after `real_ip` |
| `uri` | the request target (path and query) |
| `path` | the request path as received |
| `header:<name>` | the value of a request header |
| `cookie:<name>` | the value of a cookie |

A missing header or cookie hashes as an empty key, so all such requests go to the same
backend.

### Retries

When connecting to a backend fails (refused, unreachable, connect or TLS handshake timeout),
nothing has reached it, so another backend is tried, whatever the method. `retries` is the
number of further backends; a backend is never tried twice for one request. With
`retries: 1` and three backends, a request is tried on at most two of them. Once a request
was sent, it is never retried: a timeout or reset after connecting is answered with 504 or
502.

### Passive ejection

Each connect failure, and each error after connecting that ends a request before a response
header arrives (reset, read timeout), counts against the backend. After `failures`
consecutive errors the backend is skipped for `eject`. Any response header from the backend,
even a 5xx, resets the count: an HTTP error status is an answer, not a failure.

### Active health checks

```yaml
health_check:
  type: http          # http (default) or tcp
  path: /healthz      # default /; must start with /
  host: app.internal  # Host of the probe; default: the backend URL's host
  expect: [200, 204]  # accepted statuses; default: any 2xx or 3xx
  interval: 5s
  timeout: 2s
  rise: 2             # consecutive successes that bring a backend back
  fall: 3             # consecutive failures that take it out
```

An `http` probe is a `GET` of `path` with `User-Agent: conduit-health-check`, sent the way
proxied requests are (TLS, SNI and certificate checks per backend). A `tcp` probe only
connects. `rise` and `fall` must be at least 1.

### Choosing a backend

A backend is chosen among those that pass their health checks (all do without checks), are
not ejected, and were not already tried for this request. When none qualifies, conduit
falls back to any backend not yet tried, since trying one beats failing every request when
the health checks themselves are wrong. When no backend is left: 502.

### Pools across reloads

A pool whose definition is the same after DNS resolution (backends, weights, balancing,
checks, retries, TLS settings) is carried over by a reload with its health state, ejections
and in-flight counts. A pool that changed starts fresh. `conduit check` and reload log lines
report unused named pools as warnings.

## 5. Sites

A site is a set of host names served the same way.

```yaml
sites:
  - id: app                          # required
    hosts: [app.example.com, www.app.example.com]   # required
    upstream: web
    force_https: true
    hsts: "max-age=63072000; includeSubDomains"
    plugins:
      - rate_limit: { requests: 20, per: 1s }
```

| Key | Default | Description |
|-----|---------|-------------|
| `id` | required | Stable identifier for logs, metrics, `/status` and cache purges. Letters, digits, `.`, `-`, `_`; unique. |
| `hosts` | required | Exact names, or `*.example.com` for any subdomain. At least one. |
| `upstream` | unset | Pool name, URL or inline pool ([section 4](#4-upstream-pools)). Required unless the site serves `files:`. |
| `enabled` | `true` | `false`: the site is parsed but neither validated further nor routed. |
| `cert` | unset | Name of a `tls.certs` entry for these hosts, when SNI-based selection would pick another. Cannot be combined with `acme`. |
| `acme` | `false` | Obtain and renew one certificate for all hosts of the site ([section 11](#acme)). |
| `force_https` | `false` | Answer plain HTTP requests with a 301 to HTTPS. |
| `hsts` | unset | `Strict-Transport-Security` value added to every HTTPS response. |
| `max_body_size` | `defaults.max_body_size` | Largest request body; `0`: unlimited. |
| `connect_timeout` | `defaults.connect_timeout` | |
| `read_timeout` | `defaults.read_timeout` | |
| `write_timeout` | `defaults.write_timeout` | |
| `upgrade` | `websocket` | Which protocol upgrades reach the upstream: `websocket`, `any`, `off` ([section 13](#13-protocol-upgrades-websocket-and-others)). |
| `upgrade_idle_timeout` | `read_timeout` | An upgraded connection closes after this long without data in either direction. |
| `real_ip_header` | `real_ip.header` | Header carrying the client address for this site. |
| `compression` | `defaults.compression` | |
| `upstream_host` | the client's `Host` | `Host` sent upstream. |
| `mtls` | unset | Client certificates ([section 11](#client-certificates-mtls)). |
| `plugins` | `[]` | Plugins, run in order ([sections 7](#7-the-rules-plugin) and [8](#8-the-other-plugins)). |
| `files` | unset | Serve GET and HEAD from a directory ([section 9](#9-static-files)). |
| `cache` | unset | Cache upstream responses ([section 10](#10-response-cache)). |

### Host names and routing

- Names are compared lowercased, without a trailing dot. Valid characters are letters,
  digits, `-`, `.` and `_`.
- `*.example.com` matches `a.example.com` and `a.b.example.com`, but not `example.com`
  itself; list the apex separately.
- An exact name always wins over a wildcard, and a longer wildcard suffix wins over a
  shorter one: with `*.example.com` and `*.dev.example.com`, `x.dev.example.com` goes to
  the second.
- A host (or wildcard) listed by two enabled sites is an error, as is a duplicate site id.
- Requests whose host matches no site go to [`default_site`](#default_site).

### `force_https` and `hsts`

`force_https` redirects with 301 to `https://<host><target>`, where `<host>` is the
request's host without its port and `<target>` the original path and query. The redirect
therefore goes to port 443. ACME HTTP-01 challenges are still answered over HTTP.

`hsts` is added to responses on HTTPS connections only, after the plugins' response header
changes (so a `headers` plugin cannot remove it). The connection's own scheme counts: behind
a proxy that terminates TLS and connects over plain HTTP, no HSTS is added.

`conduit check` warns when a site with `force_https` or `hsts` has a host no certificate
covers, while HTTPS listeners exist.

### Timeouts in one place

| Setting | Scope | Default | What it bounds |
|---------|-------|---------|----------------|
| `connect_timeout` | site | 10s | TCP connect to a backend (TLS: connect and handshake get twice this together) |
| `read_timeout` | site | 90s | each read from a backend, the response header included |
| `write_timeout` | site | 90s | each write to a backend |
| `upgrade_idle_timeout` | site | `read_timeout` | silence on an upgraded connection, both directions |
| `upstream_idle_timeout` | global | 60s | an idle pooled upstream connection |
| `keepalive_timeout` | global | 75s | an idle client connection between requests |
| `forward_auth.timeout` | plugin | 5s | the auth service's answer |
| `crowdsec.appsec.timeout` | global | 200ms | the AppSec verdict |
| `health_check.timeout` | pool | 2s | one probe |

A backend timeout is answered with 504; other upstream errors with 502.

## 6. Match conditions

A `match:` block selects requests. It is used by rules, and by `files`, `cache`,
`rate_limit`, `concurrency`, `basic_auth`, `forward_auth`, `body_replace` and
`block_exploits` (including its `except` entries) to limit what they apply to.

```yaml
match:
  method: [GET, HEAD]                       # any of these
  path: { prefix: /api/ }                   # decoded, normalized path
  uri: { regex: "^/download\\?id=[0-9]+$" } # raw target, path and query
  query: { exact: "debug=1" }               # raw query string
  scheme: https                             # http or https
  headers:
    x-api-key: { present: true }
    user-agent: { regex: "(?i)curl" }
    accept: { equals: "application/json" }
```

All conditions given must hold. A condition left out does not restrict. An empty match (or
none) matches every request.

| Field | Compared with |
|-------|---------------|
| `method` | the request method; a list, any of which matches. Written in any case. |
| `path` | the request path, percent-decoded and normalized (see below). |
| `uri` | the request target exactly as received: path and query, not decoded. |
| `query` | the query string as received, without `?`; empty when there is none. |
| `scheme` | `http` or `https`: whether the client's connection to conduit uses TLS. |
| `headers` | a map of header name to condition. |

There is no host condition: hosts select sites, and each site has its own plugins.

### String conditions

`path`, `uri` and `query` take exactly one of:

| Form | Matches when the value |
|------|------------------------|
| `{ exact: "/login" }` | is exactly this |
| `{ prefix: "/api/" }` | starts with this |
| `{ regex: "^/u/[0-9]+$" }` | contains a match of the regular expression |

Regular expressions use the syntax of Rust's `regex` crate:

- a regex matches anywhere in the value unless anchored with `^` and `$`;
- `(?i)` at the start makes it case-insensitive;
- there are no backreferences and no lookaround; in exchange, matching takes linear time
  whatever the input, so a request cannot make a rule slow;
- in YAML double-quoted strings, write `\\` for a backslash (`"\\.php$"`), or use single
  quotes (`'\.php$'`).

A prefix is a plain string prefix, not a path segment: `{ prefix: /admin }` also matches
`/administrator` and `/admin-old`. Use `{ prefix: /admin/ }` plus `{ exact: /admin }`, or
`{ regex: "^/admin(/|$)" }`, to match a segment.

### Header conditions

Each header takes exactly one of:

| Form | Matches when the header |
|------|-------------------------|
| `{ present: true }` | is present and not empty |
| `{ present: false }` | is missing or empty |
| `{ equals: "..." }` | equals this value exactly (case-sensitive) |
| `{ regex: "..." }` | contains a match of the regular expression |

Header names are case-insensitive. A header sent several times is compared as all its
values joined with `, `. A missing header compares as an empty value, so `{ regex: "^$" }`
matches a missing or empty header.

### Path normalization

`path` is the path as an application would read it, like nginx's `$uri`:

1. percent-escapes are decoded (`%5F` becomes `_`, `%2F` becomes `/`);
2. empty segments are dropped (`//a` becomes `/a`);
3. `.` segments are dropped and `..` removes the previous segment;
4. a trailing slash is kept (and added after a final `.` or `..`).

So `/%61dmin/`, `//admin/`, `/x/../admin/` and `/./admin/` all match a rule for
`/admin/`, and encoding cannot slip a request past a rule. A path with an invalid
escape, an encoded NUL byte, or a `..` above the root is answered with 400 (as nginx does)
whenever something needs the normalized path. The request is still forwarded with its
original target; normalization is for matching only.

## 7. The `rules` plugin

`rules` is a list of rules, tried in the order written. The first rule whose `match` holds
acts; the rest are not looked at. When none matches, the request goes on to the next plugin.

```yaml
plugins:
  - rules:
      - name: no admin from outside      # optional, for the debug log
        match: { path: { prefix: /admin/ } }
        action: { deny: 403 }
      - match: { path: { exact: /old } }
        action: { redirect: { status: 301, to: "https://{host}/new" } }
```

A rule without `match` matches every request, so it belongs last. A rule's `name` defaults
to `rule #<n>` and appears in configuration errors and in the debug log when it matches.

Because the first match wins, write specific rules before general ones. nginx picks
`location` blocks by specificity; conduit does not, see [section 18](#18-coming-from-nginx).

### Actions

Each rule has exactly one action.

#### `deny`

```yaml
action: { deny: 403 }
```

Answers with this status (200 to 599) and no body of its own. Statuses of 300 and above get
conduit's small HTML status page ([section 15](#15-responses-conduit-generates)). There is no
equivalent of nginx's 444 (close without answering): `deny: 444` sends a response with
status 444.

#### `redirect`

```yaml
action: { redirect: { status: 301, to: "https://example.com{request_uri}" } }
```

| Key | Default | Description |
|-----|---------|-------------|
| `status` | `302` | Any 3xx: 301, 302, 303, 307, 308. |
| `to` | required | The `Location`, with placeholders. |

Placeholders in `to`:

| Placeholder | Value | For `https://a.example.com/x%20y/../z?q=1` |
|-------------|-------|------------|
| `{scheme}` | `http` or `https`, the client's connection | `https` |
| `{host}` | the request host, lowercased, without port | `a.example.com` |
| `{path}` | the normalized, decoded path | `/z` |
| `{request_uri}` | the target as received, path and query | `/x%20y/../z?q=1` |

Any other `{...}` is a configuration error, as is an unclosed `{`. Use `{request_uri}` to
keep the query string and the client's exact encoding. `{path}` is decoded, so it may carry
spaces or non-ASCII bytes into the `Location` unencoded; prefer `{request_uri}` unless the
normalized form is what you want. A `Location` that cannot be sent as a header (control
characters) is answered with 400.

#### `respond`

```yaml
action:
  respond:
    status: 200
    headers: { Content-Type: application/json, Cache-Control: no-store }
    body: '{"ok":true}'
```

Answers with this status (200 to 599), headers and body. No `Content-Type` is added: set it.
A response with an empty body, a status of 300 or above and no `Content-Type` gets the HTML
status page as its body.

#### `file`

```yaml
action:
  file:
    path: /etc/conduit/files/mobile-config.json
    content_type: application/json
    headers: { Cache-Control: no-cache }
```

Answers 200 with the contents of a file, read when the configuration loads and kept in
memory. The file is watched: changing it reloads the configuration. A file that cannot be
read is a configuration error. `content_type` is required. Use it for small files (a
`robots.txt`, an app's configuration); serve directories with [`files:`](#9-static-files).

#### `proxy`

```yaml
action:
  proxy:
    upstream: api                 # optional: another pool; default the site's
    strip_prefix: /api            # optional
    add_prefix: /v2               # optional
    # or, instead of the two prefixes:
    rewrite: { regex: "^/u/([0-9]+)$", to: "/users/${1}" }
```

`proxy` does not answer the request: it decides where the request goes and with which path,
then the request goes on to the next plugin. The plugins after the rule still run (a
`basic_auth` after it still asks for a password).

| Key | Description |
|-----|-------------|
| `upstream` | A pool name, a URL or an inline pool, as a site's `upstream:`. Default: the site's own; a rule without one on a site without an upstream is an error. |
| `strip_prefix` | Removed from the start of the path when present. Must start with `/`; no `?` or `#`. |
| `add_prefix` | Put in front of the path, after `strip_prefix`. Must start with `/`; no `?` or `#`. Trailing slashes are dropped (the path brings its own). |
| `rewrite` | `{ regex, to }`: the first match of `regex` in the path is replaced by `to`. Cannot be combined with the prefixes. `to` must not contain `?` or `#`. |

How the upstream path is built:

1. It starts from the normalized path (the one rules matched), so `/%61pi/x` and
   `/x/../api/x` cannot dodge a `strip_prefix: /api`.
2. `strip_prefix` is removed if the path starts with it; a `/` is put back when the rest
   does not start with one. It is a string prefix, not a segment: with `strip_prefix: /api`,
   `/apix` becomes `/x`. Match with `prefix: /api/` (and `exact: /api`) to avoid that.
3. `add_prefix` is put in front.
4. Or `rewrite` replaces the first match. In `to`, `$1` or `${1}` is a numbered group and
   `${name}` a named group (`(?P<name>...)`); write `${1}` when a letter, digit or `_`
   follows (`$1a` would be a group named `1a`), and `$$` for a literal `$`. When the regex
   does not match, the path stays as it is.
5. A result that does not start with `/` gets one.
6. The result is percent-encoded where needed (everything outside RFC 3986 path
   characters, `%` included) and the original query string is appended unchanged.

Because the path was decoded, an encoded `%2F` in the client's path reaches the upstream as
`/`, as with nginx's `rewrite` and `proxy_pass` with a URI. A rule without `strip_prefix`,
`add_prefix` or `rewrite` changes only the pool; the target is then forwarded as received.

| Rule | Request | Upstream gets |
|------|---------|---------------|
| `strip_prefix: /api` | `/api/users?a=1` | `/users?a=1` |
| `strip_prefix: /api` | `/api` | `/` |
| `strip_prefix: /api/`, `add_prefix: /v2/` | `/api/users` | `/v2/users` |
| `add_prefix: /v2` | `/` | `/v2/` |
| `rewrite: { regex: "^/old/(.*)$", to: "/new/${1}" }` | `/old/a/b?q` | `/new/a/b?q` |
| `rewrite: { regex: "^/x/", to: "" }` | `/x/y` | `/y` |
| `strip_prefix: /api` | `/api/a b/ü` | `/a%20b/%C3%BC` |

Other effects of a `proxy` rule:

- A request routed by a `proxy` rule is always proxied; `files:` never serves it.
- If several `rules` plugins route the same request, the first route found wins.
- The cache key of a routed request includes its pool and its rewritten target, so two
  routes never share cached entries.
- `Host` is still the client's (or the site's `upstream_host`).

### Examples

Block a path, everywhere it could be spelled:

```yaml
- rules:
    - match: { path: { regex: "(?i)^/wp-(admin|login)" } }
      action: { deny: 404 }
```

Answer a CORS preflight locally:

```yaml
- rules:
    - match: { method: [OPTIONS], headers: { origin: { present: true } } }
      action:
        respond:
          status: 204
          headers:
            Access-Control-Allow-Origin: "*"
            Access-Control-Allow-Methods: "GET, POST, PUT, DELETE, OPTIONS"
            Access-Control-Allow-Headers: "Authorization, Content-Type"
            Access-Control-Max-Age: "86400"
```

`www` to the apex, keeping path and query:

```yaml
- id: www
  hosts: [www.example.com]
  upstream: http://127.0.0.1:1    # never used: every request is redirected
  plugins:
    - rules:
        - action: { redirect: { status: 301, to: "https://example.com{request_uri}" } }
```

A maintenance page for everyone but the office:

```yaml
- rules:
    - match: { headers: { x-maintenance-bypass: { equals: "s3cret" } } }
      action: { proxy: {} }       # route to the site's upstream and stop the rules
    - action:
        respond:
          status: 503
          headers: { Content-Type: text/html, Retry-After: "600" }
          body: "<h1>Back soon</h1>"
```

(`proxy: {}` is a rule that matches and routes to the site's own upstream, which ends the
rule list for that request.)

One host, several backends:

```yaml
- id: portal
  hosts: [portal.example.com]
  upstream: http://127.0.0.1:3000           # the web front end
  plugins:
    - rules:
        - match: { path: { prefix: /api/ } }
          action: { proxy: { upstream: http://127.0.0.1:8090, strip_prefix: /api } }
        - match: { path: { exact: /api } }
          action: { proxy: { upstream: http://127.0.0.1:8090, strip_prefix: /api } }
        - match: { path: { prefix: /docs/ } }
          action: { proxy: { upstream: docs, strip_prefix: /docs, add_prefix: /portal-docs } }
```

Browsers to a new host during a migration, API clients unchanged:

```yaml
- rules:
    - match:
        method: [GET, HEAD]
        headers: { user-agent: { regex: "(?i)mozilla" } }
      action: { redirect: { to: "https://git.example.com{request_uri}" } }
```

## 8. The other plugins

Plugins are written as a list of one-key maps, run in the order written on every request of
the site:

```yaml
plugins:
  - crowdsec:                 # no settings: the defaults
  - block_exploits:
  - rate_limit: { requests: 20, per: 1s }
  - basic_auth: { htpasswd: /etc/conduit/htpasswd }
  - headers: { response: { set: { X-Frame-Options: DENY } } }
```

What each plugin does, and when:

| Plugin | Acts on | Can answer the request |
|--------|---------|------------------------|
| `rules` | request | yes (or route it) |
| `headers` | upstream request, every response | no |
| `ip_filter` | request | yes |
| `crowdsec` | request (decisions; AppSec after all plugins) | yes |
| `rate_limit` | request | yes |
| `concurrency` | request, until the response is sent | yes |
| `basic_auth` | request (may wait for a hash) | yes |
| `forward_auth` | request (waits for the auth service) | yes |
| `body_replace` | upstream response body | no |
| `block_exploits` | request | yes (or only log) |

Order matters for the plugins that answer: the first that answers ends the request, and a
plugin after it never sees it. Suggested order: `crowdsec` and `ip_filter` first (cheap, and
nothing else should see such clients), `block_exploits`, then limits, then authentication,
then `rules` that should only apply to authenticated requests. `headers` and `body_replace`
can go anywhere; when two plugins set the same header, the later one wins.

A plugin may appear several times, for example two `rate_limit` entries with different
`match` blocks.

### `headers`

```yaml
- headers:
    request:
      set: { X-Env: prod }
      remove: [X-Debug]
    response:
      set: { X-Frame-Options: DENY, X-Content-Type-Options: nosniff }
      remove: [X-Powered-By]
```

- `request` changes the request sent upstream. It runs after conduit set its own forwarding
  headers, so it can override or remove them (`X-Forwarded-Proto`, `Host`). Rules and other
  plugins always see the client's original headers, whatever the position of `headers`.
- `response` changes every response of the site: proxied, cached, files, local answers of
  rules and plugins, and error pages. Unlike nginx's `add_header`, it applies to every status.
- `remove` runs before `set`. `set` replaces every value of the header.
- `response.remove: [Server]` removes conduit's own `Server` header for this site;
  `Strict-Transport-Security` (`hsts`), `x-request-id` and `Cache-Status` are added after
  the plugins and cannot be removed this way.

### `ip_filter`

```yaml
- ip_filter: { allow: [192.0.2.0/24, 2001:db8::/32], deny: [192.0.2.66], status: 403 }
```

| Key | Default | Description |
|-----|---------|-------------|
| `allow` | `[]` | When not empty, only clients in these CIDRs pass. |
| `deny` | `[]` | Clients in these CIDRs are always refused, even when also allowed. |
| `status` | `403` | Status for refused clients; 4xx or 5xx. |

The client is the real client address ([`real_ip`](#real_ip)).

### `crowdsec`

```yaml
- crowdsec:                   # decisions only
- crowdsec: { appsec: true }  # decisions, and every request inspected by AppSec
```

Refuses clients with a CrowdSec decision, with 403 and the ban page. Needs the top-level
[`crowdsec:`](#crowdsec) section; `appsec: true` also needs `crowdsec.appsec`. The decision
check runs at the plugin's place in the chain; AppSec runs once all plugins have let the
request through. Put it first.

### `rate_limit`

```yaml
- rate_limit:
    requests: 5
    per: 1m
    key: client_ip
    match: { method: [POST], path: { exact: /api/login } }
    status: 429
```

| Key | Default | Description |
|-----|---------|-------------|
| `requests` | required | Requests allowed per window; at least 1. |
| `per` | `1s` | Window length; at least `100ms`. |
| `key` | `client_ip` | What requests are counted by. |
| `match` | all requests | Only matching requests are counted and limited. |
| `status` | `429` | Status for refused requests; 4xx or 5xx. |

Keys: `client_ip` (an IPv6 client counts as its /64, which one subscriber usually holds
whole), `host`, or `header:<name>` (requests without the header count by client address).

The limit is a sliding window: the count of the previous window, weighted by how much of it
still overlaps the last `per`, plus the count of the current window. Only admitted requests
use up the allowance, so a client sending faster than the limit still gets the limit
through. Refused requests get `Retry-After` with `per` in whole seconds (at least 1), are
logged with `"blocked_by": "rate_limit"` and counted in `conduit_limit_refused_total`.

Counts live in memory, per process, in a fixed-size sketch (64 KiB per plugin, whatever the
number of clients). A sketch can overestimate under heavy load (an unlucky client may share
a counter with another), never underestimate. Counts survive reloads while the plugin's
site, position and settings are unchanged.

Admission for the same key is serialized across workers with a fixed set of small locks,
so concurrent requests cannot all spend the same remaining allowance.

### `concurrency`

```yaml
- concurrency: { max: 8, key: client_ip, status: 429 }
```

| Key | Default | Description |
|-----|---------|-------------|
| `max` | required | Requests of one key in progress at once; at least 1. |
| `key` | `client_ip` | As for `rate_limit`. |
| `match` | all requests | Only matching requests count. |
| `status` | `429` | Status for refused requests; 4xx or 5xx. |

A request counts from its header until its response is complete; a request beyond `max` is
refused at once. It counts requests, not connections, so it also limits clients behind a CDN,
where all connections come from the CDN. An upgraded connection (WebSocket) counts for as
long as it stays open. Counting uses a 32 KiB sketch per plugin, with the same properties as
`rate_limit`.

### `basic_auth`

```yaml
- basic_auth:
    users:
      alice: "$2y$10$..."            # htpasswd -nB alice
    htpasswd: /etc/conduit/htpasswd  # user:hash lines, re-read when the file changes
    realm: restricted
    match: { path: { prefix: /admin/ } }
```

| Key | Default | Description |
|-----|---------|-------------|
| `users` | `{}` | User name to bcrypt hash. |
| `htpasswd` | unset | A file of `user:hash` lines; blank lines and `#` comments are skipped. Watched. |
| `realm` | `restricted` | Shown by browsers. No `"`, `\` or control characters. |
| `match` | all requests | Only matching requests need a password. |

At least one user is required, from `users`, `htpasswd` or both; a user listed twice is an
error. Hashes must be bcrypt (`$2a$`, `$2b$`, `$2y$`), as `htpasswd -nB <user>` and
`caddy hash-password` print them; other hash types are refused at load. User names cannot
contain `:`.

Behavior:

- No or malformed credentials: 401 with `WWW-Authenticate: Basic realm="...",
  charset="UTF-8"`.
- bcrypt runs off the request threads, at most one hash per CPU at a time, so a burst of
  wrong passwords costs CPU time but never blocks other requests. A request that waits more
  than 5 s for its turn gets 503.
- Unknown users are checked against a real hash too, so they take as long as known ones.
- Verified credentials are remembered (as a keyed digest, never in clear) until the next
  reload or until 1024 have accumulated, so a browser's repeated requests do not pay for
  bcrypt each time.
- The `Authorization` header is forwarded upstream; remove it with
  `headers: { request: { remove: [Authorization] } }` if the upstream should not see it.

Put a `rate_limit` before it against password guessing.

### `forward_auth`

```yaml
- forward_auth:
    url: http://127.0.0.1:9091/api/authz/forward-auth   # Authelia
    copy_headers: [Remote-User, Remote-Groups, Remote-Email, Remote-Name]
    timeout: 5s
```

| Key | Default | Description |
|-----|---------|-------------|
| `url` | required | The auth service's verification endpoint, `http://` or `https://`. |
| `copy_headers` | `[]` | Headers of a 2xx answer set on the request sent upstream. |
| `timeout` | `5s` | Longest wait for the answer; above 0. |
| `ca_file` | system store | CA file for an `https://` url. |
| `match` | all requests | Only matching requests are checked. |

For each request conduit sends a `GET` to `url` with:

- the client's headers (cookies and `Authorization` included), except those about the
  client's own connection (`Host`, `Content-Length`, `Transfer-Encoding`, `Connection`,
  `Keep-Alive`, `TE`, `Trailer`, `Upgrade`, `Expect`), and except `Forwarded`, `X-Real-IP`
  and every `X-Forwarded-*` header, which a client could forge;
- `X-Forwarded-Method`, `X-Forwarded-Uri` (path and query as received), `X-Forwarded-Host`
  (the client's `Host`), `X-Forwarded-Proto` and `X-Forwarded-For` (the real client
  address), set by conduit.

The request body is not sent. Then:

- a 2xx answer lets the request through. Each header of `copy_headers` is set upstream from
  the answer, or removed when the answer lacks it, so clients cannot set `Remote-User`
  themselves;
- any other answer goes back to the client as it is: status, headers (several
  `Set-Cookie` included) and body, up to 1 MiB, such as a redirect to the login page or a
  401;
- no answer within `timeout`, or an error: 502 (and a warning, at most once a minute).

Works with Authelia, Authentik, oauth2-proxy and similar services. The headers copied from
the answer are part of the cache key, so users behind `forward_auth` never share a cached
response.

### `body_replace`

```yaml
- body_replace:
    replace:
      - { from: "http://10.0.0.5:8080", to: "https://app.example.com" }
      - { from: "Powered by X", to: "" }
    types: [text/html, application/javascript]
    match: { path: { prefix: /app/ } }
```

| Key | Default | Description |
|-----|---------|-------------|
| `replace` | required | `{ from, to }` pairs; `from` must not be empty. |
| `types` | `[text/html]` | Content types (without parameters) whose bodies change; `"*"` for any. |
| `match` | all requests | Only matching requests have their responses changed. |

Like nginx's `sub_filter`, applied as the body streams:

- Matching is case-sensitive. At each position the pairs are tried in the order written and
  the first that matches is replaced. Every occurrence is replaced.
- Bodies are not buffered: at most the longest `from` minus one byte is held back between
  chunks, so a match split across chunks is still found.
- For matching requests, `Accept-Encoding`, `Range` and `If-Range` are not forwarded, so the
  upstream sends a whole, uncompressed body. A response that still arrives encoded, a 206,
  responses to HEAD, 1xx, 204 and 304 pass unchanged.
- Changed responses lose `Content-Length`, `Accept-Ranges`, `ETag` and `Last-Modified`
  (they describe the upstream's bytes) and are sent chunked to HTTP/1.1 clients. Compression
  towards the client applies afterwards.
- Only proxied responses change: files, local answers and error pages do not.

### `block_exploits`

```yaml
- block_exploits:                # alone: all defaults
- block_exploits:
    mode: block                  # or log
    status: 403
    categories: [traversal, dotfiles, sqli, xss, php, rce, ssrf, scanners]
    disable: [sqli-tautology]
    skip_params: [q, search]
    match: { path: { prefix: /api/ } }
    except:
      - match: { path: { prefix: /editor/preview } }
        categories: [xss]
```

Refuses requests that carry common exploit payloads before they reach the upstream, like an
nginx "block exploits" include, but with rules written to leave ordinary requests alone: an
OAuth `redirect_uri=http://...`, a search for "union station", a repository file under
`etc/passwd`.

| Key | Default | Description |
|-----|---------|-------------|
| `mode` | `block` | `block` refuses the request; `log` lets it through and only counts and logs it. |
| `status` | `403` | Status for refused requests; 4xx or 5xx. |
| `categories` | all but `spam` | Categories checked. When given, at least one. |
| `disable` | `[]` | Rule ids switched off (each detection's log line names its rule). |
| `skip_params` | `[]` | Query parameters left out of the inspection (a search box, a code editor). |
| `match` | all requests | Only matching requests are inspected. |
| `except` | `[]` | `{ match, categories }`: matching requests skip these categories (all when none are given). |

What is inspected:

- **the query string**, percent-decoded as applications read it (`+` is a space), decoded
  once more when escapes remain (double encoding), and also with SQL comments removed when
  it has any (`union/**/select`). `skip_params` are removed before inspection;
- **the path**, decoded and normalized as rules see it (and without SQL comments);
- **the `User-Agent`**, for scanners and header payloads;
- **other header values**, only for payloads aimed at headers (Log4Shell, Shellshock, OGNL)
  and only when the value holds `${` or `%{` or starts with `(`;
- the presence of `x-middleware-subrequest` (Next.js middleware bypass).

Request bodies are not read; CrowdSec AppSec covers those. Matching is linear-time whatever
the input: a fast literal pre-filter selects the rules worth running, and a request without
any suspicious substring costs one scan.

Categories and rules:

| Category | Rule ids | Catches |
|----------|----------|---------|
| `traversal` | `traversal-overlong`, `traversal-semicolon`, `traversal-backslash`, `traversal-encoded`, `traversal-dotdot`, `traversal-file` | overlong UTF-8 for `.`, `/`, `\`; `..;` (read as `..` by Tomcat and Spring); backslash traversal; dots and slashes still encoded after decoding (double encoding); two or more steps up in the query; system files (`/etc/passwd`, ...) and one step up to an application's configuration as a query value |
| `dotfiles` | `dotfiles` | secrets and repositories at the root of the site: `/.env`, `/.git/`, `/.svn/`, `/.aws/`, `/.ssh/`, `/.htpasswd`, `/.DS_Store`, shell histories... Deeper ones are left alone (a forge shows `.env.example` files, a WebDAV share syncs `.git` folders). |
| `sqli` | `sqli-union`, `sqli-stacked`, `sqli-time`, `sqli-subquery`, `sqli-file`, `sqli-error`, `sqli-schema`, `sqli-tautology` | `UNION SELECT` followed by what starts a column list (not a word of a sentence); destructive or administrative statements stacked after `;`; time-based blind injection (`sleep(`, `benchmark(`, `waitfor delay`); a subquery after `AND`/`OR`; file access and command procedures (`load_file(`, `into outfile`, `xp_cmdshell`); error-based extraction (`extractvalue(`, `updatexml(`); reads of system schemas (`information_schema`, `sqlite_master`, `mysql.user`); tautologies after `OR`/`AND` (`1=1`, `'a'='a'`, `2>1`) |
| `xss` | `xss-script`, `xss-handler`, `xss-uri` | script tags; event handlers in a tag or after a closing quote (those that need a click or typing are left out); `javascript:` URLs followed by code |
| `php` | `php-cgi`, `php-ini`, `php-wrapper`, `php-phpunit`, `php-globals`, `php-object`, `php-code` | PHP-CGI argument injection (CVE-2012-1823, CVE-2024-4577); `allow_url_include` and `auto_prepend_file` overrides; stream wrappers as values (file inclusion); PHPUnit's `eval-stdin.php` (CVE-2017-9841); `GLOBALS`/`_SERVER`-style overwrites from the `register_globals` era; serialized objects; PHP code as a value (`<?php`, `eval(`, `base64_decode(`, `phpinfo`) |
| `rce` | `rce-log4shell`, `rce-shellshock`, `rce-ognl`, `rce-spring4shell`, `rce-ssti`, `rce-command`, `rce-nextjs-subrequest` | Log4Shell JNDI lookups, nested to hide or not; Shellshock; OGNL; Spring4Shell; template injection; shell commands after a separator and reverse shells; the Next.js `x-middleware-subrequest` header (CVE-2025-29927) |
| `ssrf` | `ssrf-metadata` | cloud metadata endpoints as a query value |
| `scanners` | `scanner` | scanners' user agents (sqlmap, nikto, nuclei and the common block-exploits list) |
| `spam` | `spam` | the common block-exploits list's spam words, as whole words; off unless listed in `categories` |

A detection is logged as a warning (at most 20 lines a second, so a scan cannot flood the
log) with its category, rule id, where it was found, site, client and target:

```
block_exploits: blocked sqli (sqli-union) in query: site app, client 203.0.113.9, GET app.example.com/?id=1%20union%20select%201,2
```

Refused requests are logged with `"blocked_by": "block_exploits:<category>"`;
`conduit_block_exploits_total{site, category, action}` counts every detection, in both
modes. A good way to start on a busy site is `mode: log` for a few days, then `disable`,
`skip_params` or `except` for what turns out to be legitimate, then `mode: block`.

## 9. Static files

```yaml
- id: docs
  hosts: [docs.example.com]
  files:
    root: /srv/docs
    index: [index.html]
    try_files: ["{path}", "{path}/", "/404.html"]
    precompressed: [br, gzip]
    hide: ["*.bak", node_modules]
```

| Key | Default | Description |
|-----|---------|-------------|
| `root` | required | Directory the request path maps into; must exist. |
| `match` | all requests | Only matching requests are served from files; the rest go upstream. Needs an upstream. |
| `strip_prefix` | unset | Removed from the path before the lookup: with `/static`, `/static/app.js` is `<root>/app.js`. |
| `index` | `[index.html, index.txt]` | Files served for a directory, first found wins. Plain file names only. |
| `try_files` | `[]` | Paths tried in order (see below). |
| `browse` | `false` | List directories that have no index file. |
| `precompressed` | `[]` | Serve `<file>.br`, `.zst` or `.gz` next to a file when the client accepts it, in the order given: any of `br`, `zstd`, `gzip`. |
| `hide` | `[]` | File or directory names never served: exact names or `*.ext`. |
| `pass_thru` | `false` | A file that does not exist goes to the upstream instead of a 404. Needs an upstream. |

### Which requests are served from files

GET and HEAD requests matching `files.match` (all of them by default), that no `proxy` rule
routed and that every plugin let through. Other methods go upstream; on a site without an
upstream they get 405 with `Allow: GET, HEAD`.

A site may have `files:` and an upstream together: with `match` (only some paths are files),
with `pass_thru` (files first, then the application), or with `try_files` falling back to a
file.

### Lookup

The lookup uses the normalized path (decoded, `..` resolved). `strip_prefix` must match at a
segment boundary: with `/static`, `/static` and `/static/x` are looked up as `/` and `/x`, and
`/staticx` is not found.

Percent escapes are decoded once before plugin checks and file lookup. Expanded
`try_files` candidates preserve literal percent bytes; they are not decoded again.

Without `try_files`, the path itself is looked up:

- a file is served;
- a directory requested without a trailing slash is redirected (308) to the same path with
  one; with a slash, its first `index` file is served, or a listing when `browse` is on;
- a file requested with a trailing slash is redirected (308) to the path without it;
- otherwise: 404, or the upstream with `pass_thru`.

With `try_files`, each entry is tried in order until one is found:

- an entry starts with `/` or `{path}`; `{path}` is the request path (after `strip_prefix`)
  and is the only placeholder;
- `{path}` alone names files only, with the redirects above; `{path}/` (or any entry ending
  in `/`) names a directory, served by its index file;
- any other entry, such as `/index.html`, is served as found, without a redirect: the URL
  the client asked for stays;
- when no entry is found: 404, or the upstream with `pass_thru`.

A single-page application, with client-side routes answered by the app:

```yaml
files:
  root: /srv/spa/dist
  try_files: ["{path}", "/index.html"]
```

The front end on disk, `/api/` to the application. "Everything but `/api/`" cannot be one
`files.match` regex (Rust regexes have no lookahead), so route `/api/` with a rule instead:
rules run before files, and a routed request is never served from files.

```yaml
upstream: http://127.0.0.1:8080
plugins:
  - rules:
      - match: { path: { prefix: /api/ } }
        action: { proxy: {} }
files:
  root: /srv/spa/dist
  try_files: ["{path}", "/index.html"]
```

### What is never served

- Anything outside `root`. Every file is opened with `openat2(RESOLVE_BENEATH)` relative to
  the root, so `..`, absolute symlinks and symlinks leading out of the root fail in the
  kernel, even when a link is planted inside the root. On kernels without `openat2` (or
  where seccomp blocks it), conduit walks pinned directory descriptors with `O_NOFOLLOW`.
  This fallback rejects all symlinks, including links that stay inside the root. Directory
  listings use the verified directory descriptor through `/proc/self/fd` and omit symlinks.
- Dotfiles and dot-directories at any depth, except `.well-known`.
- Names listed in `hide`, at any depth.

These count as missing (404, or the upstream with `pass_thru`). A file the process may not
read is answered with 403; other disk errors with 500.

### Responses

- `Content-Type` from the file extension: HTML, CSS, JavaScript (with `charset=utf-8` for
  text types), JSON, XML, feeds, images (PNG, JPEG, GIF, WebP, AVIF, SVG, ICO), fonts
  (WOFF, WOFF2, TTF, OTF), WebAssembly, PDF, archives, office documents, audio and video
  (MP4, WebM, HLS, DASH, MP3, Ogg, FLAC...). Unknown extensions are
  `application/octet-stream`.
- `ETag` (from the modification time and size, plus the encoding for precompressed files)
  and `Last-Modified`.
- Conditional requests, in RFC 9110 order: `If-Match` and `If-Unmodified-Since` (412),
  `If-None-Match` and `If-Modified-Since` (304).
- Single byte ranges (`Range: bytes=...`, with `If-Range`): 206, or 416 when unsatisfiable.
  Several ranges in one request get the whole file with 200.
- `Accept-Ranges: bytes`; `Vary: Accept-Encoding` when `precompressed` is set.
- Precompressed siblings are served with `Content-Encoding` when the client accepts the
  encoding (`q=0` refuses it). Other compressible files are compressed on the fly when the
  site's `compression` is on (whole files only, never ranges).
- Directory listings are HTML with `Cache-Control: no-cache`, without dotfiles and hidden
  names, and stop at 10,000 entries.

Files are read off the request threads: one hop opens and reads files up to 128 KiB; larger
files are read 128 KiB at a time while the previous chunk is sent.

## 10. Response cache

```yaml
cache:                           # top level: memory shared by all sites
  max_size: 256m
sites:
  - id: cms
    hosts: [cms.example.com]
    upstream: http://127.0.0.1:2368
    cache:                       # or `cache: true` for the defaults
      match: { path: { prefix: /blog/ } }
      default_ttl: 10m
      stale_if_error: 1h
```

| Key | Default | Description |
|-----|---------|-------------|
| `match` | every GET and HEAD | Only matching requests use the cache. |
| `default_ttl` | unset | Freshness for responses that state none. Unset: such responses are not stored. Above 0. |
| `stale_if_error` | unset | How long an expired response may stand in when the upstream fails, unless the response says otherwise. |

`cache: false` is an error (leave the key out instead). A site with `cache:` needs an
upstream. Entries live in memory only.

### What may be served from the cache

GET and HEAD requests without a body and without an `Upgrade` header, matching `match`, that
are proxied (not served from files and not answered by a plugin).

### What is stored

Responses follow RFC 9111 for a shared cache, with stricter rules:

- stored when the response's `Cache-Control` (`s-maxage`, then `max-age`) or `Expires`
  makes it fresh, unless it says `no-store` or `private`;
- a response with `no-cache` is stored but already stale: it is revalidated with the
  upstream before every use;
- a request with `Authorization` is stored only when the response allows it for shared
  caches (`public`, `s-maxage`, `must-revalidate`);
- never stored: responses with `Set-Cookie`, 206 responses, `Vary: *`;
- `default_ttl` applies to responses with no freshness of their own, of statuses cacheable
  by default (200, 203, 204, 300, 301, 308, 404, 405, 410, 414, 501), unless they say
  `no-store` or `private`, and never to requests with `Authorization`;
- responses larger than `cache.max_object_size` pass through unstored.

The upstream decides what is shared: a page that differs per cookie must say `private` (or
`no-store`), or every visitor gets the first one's copy.

### Keys and variants

The key includes the site, scheme, normalized routing host, original request authority
(including its port), and target (path and query as received). The original authority also
separates `Host: app.test:8080` from `Host: app.test:9090`. Additional identity includes:

- headers set by `forward_auth` (`copy_headers`);
- the client certificate's fingerprint on `mtls` sites;
- an `X-Forwarded-Host` a trusted proxy passed on;
- the pool and rewritten target of a `proxy` rule, and whether `body_replace` applies.

Different copied forward-auth headers or client certificate fingerprints separate cache
entries. The origin must still declare responses private when they depend on identity that
is not represented in the key, such as an application session cookie.

`Vary` is honored: each combination of the listed request headers is its own variant.
Explicit `Vary: Accept-Encoding` preserves all raw field values, field boundaries, unknown
codings, q-values and absent versus empty. This conservative separation may store multiple
entries for equivalent spellings. Without explicit Vary, compressed responses also vary on
the client's acceptance of the stored encoding; unknown encodings use raw header values.

### Freshness, revalidation and failures

- A fresh entry is served without asking the upstream (`Cache-Status: conduit; hit`).
- An expired entry with a validator is revalidated with a conditional request; a 304 from
  the upstream refreshes it.
- `stale-while-revalidate` in a response lets an expired entry be served while it is
  refreshed in the background. That background request is not logged or counted, as with
  nginx's default.
- `stale-if-error` in a response, or the site's `stale_if_error`, lets an expired entry be
  served when the upstream fails.
- Concurrent requests for the same missing entry wait up to `cache.lock_timeout` for the
  first one's fetch instead of all going upstream.
- URLs found uncacheable are remembered, so their requests skip the cache lock.

### `Cache-Status` and the log

Responses of sites with a cache carry a `Cache-Status` header (RFC 9211); the access log's
`cache` field and `conduit_cache_requests_total` use the matching result:

| `cache` | `Cache-Status` | Meaning |
|---------|----------------|---------|
| `hit` | `conduit; hit` | served from the cache |
| `stale` | `conduit; hit; detail=stale` | an expired entry served (while revalidating, or on error) |
| `revalidated` | `conduit; fwd=stale; fwd-status=304` | expired, confirmed by the upstream |
| `miss` | `conduit; fwd=miss; stored` | fetched and stored (also after an expired entry was replaced) |
| `uncacheable` | `conduit; fwd=miss` | fetched; the response may not be stored |
| `bypass` | `conduit; fwd=bypass` | the request may not use the cache (method, `match`, body) |

### Purging

```sh
curl -X POST 'http://127.0.0.1:9180/cache/purge?site=cms'
curl -X POST 'http://127.0.0.1:9180/cache/purge?site=cms&prefix=/blog/'
```

Drops a site's entries, or those whose target (path and query, as clients send them) starts
with `prefix`. The answer is `{"site": "cms", "purged": <count>}`. A reload drops the
entries of sites that no longer have `cache:`. Changing an upstream, its HTTP policy or
headers does not by itself invalidate a fresh cached response. Purge affected entries when
a cutover must immediately serve the new origin. The endpoint removes entries present at
the time of purge; an in-flight fill can insert an entry afterwards.

## 11. TLS, ACME and client certificates

### Certificate selection

The certificate is chosen per handshake from the SNI, across all certificates:

1. the certificate a site names with `cert:` for that exact host, or the ACME certificate
   of a site with `acme: true`; then an explicit `cert:` override for a matching wildcard
   site (one label only, as for wildcard SANs);
2. a certificate whose SAN lists the name exactly;
3. a certificate with a wildcard SAN covering it: `*.example.com` covers exactly one label
   (`a.example.com`, not `a.b.example.com` and not `example.com`);
4. `tls.default_cert`;
5. otherwise the handshake fails.

When several certificates list the same name, the first in `tls.certs` wins. Because
selection goes by SAN and not by site, a host gets HTTPS whenever any configured certificate
covers it, whether or not its site mentions a certificate. With nginx, a host is served over
TLS only where its `server` block has `ssl_certificate`.

Certificates and keys are reloaded when their files change (a certbot renewal), without
dropping connections. `conduit_tls_certificate_expiry_timestamp_seconds` and `/status`
report each certificate's expiry.

HTTPS listeners offer HTTP/2 and HTTP/1.1 over ALPN.

### ACME

```yaml
tls:
  acme:
    directory: letsencrypt         # or letsencrypt-staging, or a directory URL
    email: ops@example.com
    storage: /var/lib/conduit/acme
sites:
  - id: app
    hosts: [app.example.com, www.app.example.com]
    upstream: http://127.0.0.1:3000
    acme: true
```

| Key | Default | Description |
|-----|---------|-------------|
| `directory` | `letsencrypt` | `letsencrypt`, `letsencrypt-staging`, or an `https://` directory URL. |
| `email` | unset | Contact address for the account (expiry and policy mail from the CA). |
| `storage` | `/var/lib/conduit/acme` | Account keys and certificates: one directory per CA, one `<site id>.pem` (key and chain) per site. |
| `ca_file` | system store | CA certificate (PEM) trusted for the directory's HTTPS, e.g. a test CA. |
| `profile` | unset | Certificate profile offered by the CA, e.g. `tlsserver` or `shortlived`. |
| `renew_before` | a third of the lifetime | Renew this long before expiry when the CA suggests no renewal window; capped at half the lifetime. |

Enabling it accepts the CA's terms of service. How it works:

- one certificate per site covers all of its hosts (at most 100);
- validation is HTTP-01, answered by conduit itself from memory on every host, on
  `listen.http` (required; the CA connects to port 80). Wildcard hosts cannot be validated
  over HTTP and are refused: use a certificate file for them;
- a site with `acme: true` cannot also name a `cert:`;
- a background service orders a certificate when a site has none or its hosts changed, and
  renews inside the window the CA suggests (ARI, RFC 9773), or when a third of its lifetime
  is left if the CA suggests none;
- the PEM file is replaced atomically and swapped in by the reloader, so connections are
  never dropped;
- until the first certificate arrives, `conduit check` and the log warn that the site is
  waiting, and HTTPS on its hosts uses whatever other certificate covers them (or
  `default_cert`);
- `/status` shows each ACME site's expiry, renewal time, whether the CA set the window, and
  the last error with its retry time; `conduit_acme_orders_total` counts orders.

### Client certificates (mTLS)

```yaml
- id: internal
  hosts: [internal.example.com]
  upstream: http://127.0.0.1:8081
  mtls:
    ca: /etc/conduit/clients-ca.pem   # CAs that sign client certificates
    optional: false
```

| Key | Default | Description |
|-----|---------|-------------|
| `ca` | required | PEM file with the CA certificates that sign client certificates. Watched. |
| `optional` | `false` | `true`: clients without a certificate pass (the upstream sees `X-Client-Verify: NONE`). A certificate that fails verification is refused either way. |

- The handshake asks for a client certificate only on SNI names of mTLS sites; other sites
  on the same listener are unaffected. TLS sessions are kept apart per CA set.
- A missing certificate (when required) gets 403 `client certificate required`; an invalid
  one gets 403 with the reason. The access log says `"blocked_by": "mtls"`.
- A request reaching an mTLS site over a connection whose handshake was for another host
  (an HTTP/2 connection reused across hosts, or an SNI naming another host) gets 421, which
  makes clients retry on a connection of their own.
- The upstream receives `X-Client-Verify` (`SUCCESS` or `NONE`), `X-Client-Cert-Subject`
  (RFC 4514, e.g. `CN=alice,O=Example`) and `X-Client-Cert-Fingerprint` (SHA-256, hex).
  Clients cannot set these headers themselves: any they send are removed.
- An mTLS site without HTTPS listeners refuses every request (`conduit check` warns).

## 12. Headers conduit adds and passes

### To the upstream

| Header | Value |
|--------|-------|
| `Host` | the client's `Host` as received (for HTTP/2, its `:authority`); the site's `upstream_host` when set |
| `X-Forwarded-For` | the client's own `X-Forwarded-For` values, if any, followed by the real client address, like nginx's `$proxy_add_x_forwarded_for` |
| `X-Real-IP` | the real client address |
| `X-Forwarded-Proto` | `http` or `https`: the connection's scheme, or the value a trusted proxy sent |
| `X-Forwarded-Host` | the client's `Host` as received, or the value a trusted proxy sent |
| `x-request-id` | the request id |
| `X-Client-Verify`, `X-Client-Cert-Subject`, `X-Client-Cert-Fingerprint` | on `mtls` sites |
| `forward_auth` `copy_headers` | from the auth service's answer |

Then the `headers` plugins' `request` changes apply, which can override or remove any of
these. The leftmost `X-Forwarded-For` entries come from the client and can be forged: an
upstream should read `X-Real-IP` or the rightmost entry.

On sites with `upgrade: any`, the hop-by-hop headers no handshake uses (`Keep-Alive`, `TE`,
`Proxy-Connection`) are removed from upgrade requests, as nginx does.

### From a trusted proxy

When the TCP peer is in `real_ip.trusted`, conduit also believes:

- `X-Forwarded-Proto` (`http` or `https`): passed upstream, and used for the cache key and
  `forward_auth`;
- `X-Forwarded-Host`: passed upstream unchanged (and part of the cache key);
- `x-request-id`: kept as the request id when it is 1 to 128 characters of letters,
  digits and `-_.:/+=`.

From other peers these are replaced by conduit's own values. Note that `scheme` in match
conditions, `{scheme}` in redirects, `force_https` and `hsts` use the connection's own
scheme, not `X-Forwarded-Proto`.

### To the client

| Header | When |
|--------|------|
| `Server` | every response: `defaults.server_header`, replacing the upstream's (unless `off`) |
| `x-request-id` | every response of a request that got an id |
| `Strict-Transport-Security` | `hsts` sites, HTTPS connections |
| `Cache-Status` | sites with `cache:` |

Request ids are 24 hexadecimal digits, unique within the process, unless a trusted proxy
supplied one. They appear in the access log, so a client can quote the id of a failed
request.

## 13. Protocol upgrades (WebSocket and others)

| `upgrade` | Behavior |
|-----------|----------|
| `websocket` (default) | WebSocket handshakes (`Upgrade: websocket`) reach the upstream, with the hop-by-hop headers normalized. Any other upgrade reaches the upstream as an ordinary request, without its upgrade headers. |
| `any` | Every upgrade reaches the upstream with its handshake headers as the client sent them: also Tailscale's control protocol (headscale), DERP and other non-WebSocket protocols, like nginx with `proxy_set_header Upgrade $http_upgrade`. |
| `off` | No upgrade: the upstream sees an ordinary request and cannot switch protocols. |

Once the upstream answers `101 Switching Protocols`:

- the connection is closed after `upgrade_idle_timeout` (default `read_timeout`) with no data
  in either direction; every byte in either direction restarts the timer. Raise it for
  protocols that sit idle between messages (a long-poll control session: `15m`);
- `max_body_size` no longer applies: the bytes are the new protocol's stream, not a body;
- the request still counts for `concurrency` until the connection closes;
- the access log line is written when the connection closes, with the total duration and
  bytes. A connection still open when the process stops or is replaced is logged too.

WebSockets are upgrades of HTTP/1.1 requests; upgraded upstream connections use HTTP/1.1.

## 14. Compression

With `compression: true` (the default), responses are compressed on the way to the client
when:

- the client accepts it: conduit uses the first of `gzip`, `br` and `zstd` that the client
  lists in `Accept-Encoding` (q-values are not weighed);
- the response is not compressed already;
- the content type is text-like: `text/*`, JavaScript, JSON, the web app manifest, XML, RSS
  and SVG;
- the status is 200, 403 or 404, as with nginx. A 206 carries a byte range of the
  uncompressed representation and is never compressed.

The level is 1, the fastest, as nginx's default. Files are compressed the same way when
whole; precompressed siblings are preferred when `precompressed` is set.

## 15. Responses conduit generates

| Status | When |
|--------|------|
| 301 | `force_https` |
| 308 | a file server's canonical redirect (trailing slash added or removed) |
| 400 | malformed request; a target not starting with `/`; a path with an invalid escape, NUL or `..` above the root (when something needs the normalized path); an invalid redirect `Location` |
| 401 | `basic_auth` without valid credentials; a `forward_auth` service's own 401 |
| 403 | `ip_filter`, `crowdsec`, AppSec, `block_exploits`, `mtls`, a file the process may not read (`ip_filter` and `block_exploits` take another status; AppSec may name one) |
| 404 | unknown host without `default_site.page`; a missing file |
| 405 | a non-GET/HEAD request to a site that only serves files |
| 412, 416, 304, 206 | conditional and range requests for files |
| 413 | a body above `max_body_size` |
| 421 | an mTLS site reached over a connection made for another host |
| 429 | `rate_limit`, `concurrency` (configurable) |
| 502 | upstream error or refused connection; no backend left; `forward_auth` service unreachable |
| 503 | `basic_auth` could not get a turn at hashing within 5 s |
| 504 | upstream connect, TLS handshake, read or write timeout |

Local responses with an empty body, a status of 300 or above (except 304) and no
`Content-Type` get a small HTML page naming the status, signed `conduit`. When proxying
fails (502, 504, a 413 for a body that grew past the limit), the error response closes the
client connection; so does a local answer to a request whose body was not read. A client
that disconnects before the response is logged with 499, as nginx does; no response is sent.

## 16. Access log

With `access_log.path` set, each request writes one JSON object per line:

```json
{"ts":"2026-11-07T06:45:30.123Z","request_id":"0123456789abcdef01234567","site":"api","host":"api.example.com","client":"198.51.100.14","method":"GET","scheme":"https","uri":"/api/records?id=1","proto":"HTTP/2","status":200,"bytes_sent":228,"duration_ms":1.534,"upstream":"10.0.0.2:8090","upstream_status":200,"upstream_connect_ms":0.0,"upstream_header_ms":1.2,"tls":"TLSv1.3","user_agent":"Dart/3.9 (dart:io)"}
```

| Field | Present | Description |
|-------|---------|-------------|
| `ts` | always | completion time, RFC 3339 UTC with milliseconds |
| `request_id` | always | as sent in `x-request-id` |
| `site` | when a site matched | the site id |
| `host` | always | the request host as conduit read it (lowercased, no port; empty when missing) |
| `client` | always | the real client address (`null` if unknown) |
| `method`, `scheme`, `uri`, `proto` | always | `uri` is the target as received; `proto` is `HTTP/1.1`, `HTTP/2`, ... |
| `status` | always | the status sent; 499 when the client went away first |
| `bytes_sent` | always | response body bytes sent |
| `bytes_received` | when not 0 | request body bytes read |
| `duration_ms` | always | from the request header to the end of the response |
| `upstream` | when proxied | backend address `ip:port` |
| `upstream_status` | when the backend answered | its status |
| `upstream_connect_ms` | when a new connection was made | time to connect |
| `upstream_header_ms` | when the backend answered | from choosing the backend to its response header |
| `retries` | when not 0 | further backends tried |
| `tls` | on TLS connections | `TLSv1.2`, `TLSv1.3` |
| `user_agent`, `referer` | when sent | |
| `error` | when the request failed | the error type |
| `blocked_by` | when refused | `crowdsec`, `appsec`, `mtls`, `rate_limit`, `concurrency`, `basic_auth`, `forward_auth`, `block_exploits:<category>` |
| `cache` | sites with `cache:` | `hit`, `stale`, `revalidated`, `miss`, `uncacheable`, `bypass` |

Lines are written by one background thread: request threads format into per-thread buffers,
and nothing on the request path touches the disk. Within one write interval lines are grouped
by worker thread rather than strictly in completion order; `ts` orders them. When the writer
falls behind and a thread's buffer is full (`queue_capacity`), new lines are dropped and
counted rather than slowing traffic. CrowdSec can read the log with
`deploy/crowdsec/conduit-logs.yaml`.

## 17. Admin endpoints and metrics

On `admin.listen`, without authentication:

| Endpoint | Description |
|----------|-------------|
| `GET /metrics` | Prometheus text format. |
| `GET /healthz` | `ok`: the process is alive. |
| `GET /status` | JSON: sites (upstream, files, cache, plugins, mTLS), pools and backends (healthy, ejected, in flight), certificates and expiry, ACME and CrowdSec state, the last reload and its error, cache size. |
| `POST /cache/purge?site=<id>[&prefix=<path>]` | Drops cached responses ([section 10](#purging)). |

Metrics:

| Metric | Labels | Description |
|--------|--------|-------------|
| `conduit_requests_total` | `site`, `code` | responses sent, by status class; `site="-"` for unknown hosts |
| `conduit_request_duration_seconds` | `site` | histogram, request header to end of response |
| `conduit_upstream_header_duration_seconds` | `site` | histogram, backend chosen to response header |
| `conduit_response_body_bytes_total` | `site` | |
| `conduit_requests_in_flight` | | |
| `conduit_backend_requests_total` | `pool`, `backend` | retries included |
| `conduit_backend_failures_total` | `pool`, `backend` | connect failures and upstream errors |
| `conduit_backend_up` | `pool`, `backend` | 1 when passing active checks (always 1 without) |
| `conduit_backend_ejected` | `pool`, `backend` | 1 while passively ejected |
| `conduit_backend_requests_in_flight` | `pool`, `backend` | |
| `conduit_upstream_retries_total` | `pool` | |
| `conduit_upstream_reuse_lost_total` | | idle upstream connections that could not be reused |
| `conduit_limit_refused_total` | `site`, `limit` | `rate_limit` or `concurrency` |
| `conduit_auth_requests_total` | `site`, `plugin`, `result` | `allow`, `deny`, `error` |
| `conduit_block_exploits_total` | `site`, `category`, `action` | `block` or `log` |
| `conduit_crowdsec_decisions` | | decisions held |
| `conduit_crowdsec_pulls_total` | `result` | |
| `conduit_crowdsec_last_pull_success_timestamp_seconds` | | |
| `conduit_crowdsec_blocked_total` | `by` | `crowdsec` or `appsec` |
| `conduit_crowdsec_appsec_requests_total` | `result` | `allow`, `block`, `error` |
| `conduit_cache_requests_total` | `site`, `result` | as the log's `cache` field |
| `conduit_cache_size_bytes`, `conduit_cache_objects`, `conduit_cache_evicted_objects` | | |
| `conduit_tls_certificate_expiry_timestamp_seconds` | `name` | |
| `conduit_acme_orders_total` | `result` | |
| `conduit_config_reloads_total` | `result` | `success`, `failure` |
| `conduit_config_last_reload_success_timestamp_seconds` | | |
| `conduit_access_log_dropped_total` | | |
| `conduit_build_info` | `version` | always 1 |

## 18. Coming from nginx

### Directives and their equivalents

| nginx | conduit |
|-------|---------|
| `listen 80; listen 443 ssl;` | `listen: { http: [...], https: [...] }`, shared by all sites |
| `server_name a.example.com *.example.com;` | `hosts: [a.example.com, "*.example.com"]` |
| `default_server` | `default_site.page` (or 404) |
| `ssl_certificate`, `ssl_certificate_key` | `tls.certs`, selected by SNI ([section 11](#certificate-selection)) |
| certbot | `acme: true`, or `tls.acme_webroot` for an external client |
| `return 301 https://$host$request_uri;` | `force_https: true` |
| `add_header Strict-Transport-Security ...` | `hsts:` |
| `proxy_pass http://127.0.0.1:3000;` | `upstream: http://127.0.0.1:3000` |
| `upstream { server ...; }` | `upstreams:` pools |
| `least_conn;`, `hash $remote_addr consistent;` | `balance: least_conn`, `balance: hash` |
| `location` blocks | `rules` (first match in written order) |
| `proxy_pass http://up/;` in `location /api/` | `proxy: { upstream: up, strip_prefix: /api }` |
| `rewrite ^/old/(.*)$ /new/$1 break;` | `proxy: { rewrite: { regex: "^/old/(.*)$", to: "/new/${1}" } }` |
| `return 301 https://example.com$request_uri;` | `redirect: { status: 301, to: "https://example.com{request_uri}" }` |
| `return 200 "ok";` | `respond: { status: 200, body: ok }` |
| `allow`, `deny` | `ip_filter` |
| `auth_basic`, `auth_basic_user_file` | `basic_auth` (bcrypt hashes only) |
| `auth_request` | `forward_auth` |
| `limit_req` | `rate_limit` |
| `limit_conn` | `concurrency` (counts requests, not connections) |
| `sub_filter` | `body_replace` |
| `proxy_set_header` | `headers.request` |
| `add_header`, `proxy_hide_header` | `headers.response` |
| `proxy_set_header Host example.internal;` | `upstream_host: example.internal` |
| `set_real_ip_from`, `real_ip_header`, `real_ip_recursive` | `real_ip` |
| `root`, `alias`, `index`, `try_files`, `autoindex`, `gzip_static` | `files:` (`root`, `strip_prefix`, `index`, `try_files`, `browse`, `precompressed`) |
| `client_max_body_size` | `max_body_size` |
| `proxy_connect_timeout`, `proxy_read_timeout`, `proxy_send_timeout` | `connect_timeout`, `read_timeout`, `write_timeout` |
| `proxy_http_version 1.1; proxy_set_header Upgrade ...` | nothing for WebSocket; `upgrade: any` for other protocols |
| `gzip on;` | `compression` (on by default) |
| `proxy_cache` | `cache:` |
| `stub_status` | `/metrics`, `/status` |

### Behavior that differs

- **Location precedence.** nginx picks an exact `=` location first, then the longest prefix,
  then regexes in order. conduit's rules are tried strictly in the written order, so write
  exact and specific rules before general ones.
- **Certificates.** conduit selects certificates by SNI across all of them: a host whose
  nginx `server` had no TLS gets HTTPS from conduit when any configured certificate (or
  `default_cert`) covers it.
- **Defaults.**

  | | nginx | conduit |
  |---|---|---|
  | request body limit | 1m | 2000m |
  | connect timeout | 60s | 10s |
  | read / send timeouts | 60s | 90s |
  | `Host` to upstream | `$proxy_host` (the upstream's name) | the client's `Host` |
  | `X-Forwarded-*`, `X-Real-IP` | not sent unless configured | always sent |
  | upstream protocol | HTTP/1.0 before 1.29.7; HTTP/1.1 since 1.29.7 | HTTP/1.1 by default; optional HTTP/2 and h2c |
  | WebSocket | needs explicit headers | on |
  | compression | off | on, level 1, text types |
  | `add_header` | 2xx and 3xx unless `always` | every response |
  | retries to the next backend | on error and timeout | on connect failure only (never after the request was sent) |
  | unknown host | the default server | 404 or `default_site.page` |
  | `Server` | `nginx/<version>` | `conduit` (no version) |

- **Header names.** conduit's `headers` plugin and match conditions compare header names
  case-insensitively; values are compared exactly.
- **No equivalent** for: variables and `map`, `if`, `error_page` and
  `proxy_intercept_errors` (conduit's error pages are fixed), `return 444`, `satisfy any`
  (plugins all have to pass), per-location certificates, client-facing cleartext HTTP/2, streams (TCP and
  UDP proxying), and Lua or njs scripting.
