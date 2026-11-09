# Pingora fork

`vendor/pingora` is Pingora 0.9.1 (tag `0.9.1`, commit `f86ffda`) with conduit's performance
patches on branch `conduit`. Each patch is one commit on top of that tag, with its reasoning and
its tests, so it can be read with `git -C vendor/pingora log -p 0.9.1..conduit`.

The fork exists because the HTTP/1.1 and HTTPS throughput gap with nginx is inside Pingora, not
in conduit's code (see "HTTP/1.1 and HTTPS throughput" in the
[technical documentation](../docs/technical.md#http11-and-https-throughput)). Decision of
2026-11-07: stay on Pingora and patch its hot spots rather than write an HTTP/1 stack of our own.

## How it is wired in

- `vendor/pingora` is a git submodule pointing at branch `conduit` of the fork repository. Clone
  conduit with `git clone --recurse-submodules`, or run `git submodule update --init` after a
  plain clone. New patches are committed in the fork and pushed to branch `conduit`, then the
  submodule commit in the conduit repository is updated.
- `Cargo.toml` takes every `pingora-*` crate from this folder through `[patch.crates-io]`, so no
  Pingora crate is built twice. The version stays 0.9.1, and `Cargo.lock` keeps exactly the same
  dependency versions as before the fork.
- The `Dockerfile` copies `vendor/` into the release build. `.dockerignore` drops `.git` and
  `target` inside it.
- `.graphifyignore` excludes `vendor/pingora/` from the graph. This file stays in.

## Patches

Numbers are measured with `scripts/ab.sh` (old and new binaries alternating on the same CPUs,
7 rounds, median), plain release build without PGO, 64 keep-alive connections, 2 KB bodies,
access log on. "CPU" is conduit's CPU time per request.

| Commit | What it does | CPU HTTP | CPU HTTPS |
|---|---|---:|---:|
| `79bf391` | Idle upstream connections are kept without a task per connection | −9.3% | −7.4% |
| `32d98ca` | The reuse check compares the address from the digest, without `getpeername()` | ±0 (1 syscall per request fewer) | ±0 |
| `ecef83b` | The compression MIME check uses a prefix, not a regex | | |
| `9586c10` | Hop-by-hop header removal is skipped when there are none | | |
| `c448eff` | `insert_header` with an already parsed name does not parse it again (`CasedHeaderName`) | | |
| the three above, together | | −3.9% | −3.0% |
| `9c64421` | Header name case is kept as a style plus exceptions, not a second HeaderMap | ±0 | −1.0% |
| `3357c9e` | A batch of response tasks is flushed once, not once per task | ±0 (already 1 send) | ±0 |
| `bb3859d` | The upstream response header is handed over to the proxy, not copied | −1.4% | −2.2% |
| `c4cce19` | An HTTP/1 task channel of its own (`task_pipe`), instead of two `tokio::sync::mpsc` per request | ±0 | −2.3% |
| `0a6aee4` | HTTP modules in a request's context are looked up in order, without a shared `Arc<HashMap>` index | ±0¹ | −2.7%¹ |

¹ Measured together with small changes in conduit (stage A, see the
[technical documentation](../docs/technical.md)): gzip −1.9%, HTTP/2 −2.9%.

Total up to `3357c9e`: CPU HTTP −14%, HTTPS −12%. The CPU resolution of `scripts/ab.sh` is about
±1% (a 10 ms tick over 50,000 requests), so patches below 1% show more clearly in a samply
profile: `bb3859d` removes the response header clone (1.05% of worker samples → 0), `c4cce19`
cuts the channel's share from 3.1% to 1.5%.

`3357c9e` targets responses that conduit compresses (the `gzip` scenario in `scripts/ab.sh`,
HTTP with `Accept-Encoding: gzip`): req/s +22.8%, CPU per request −13.5%, p99 −19.5%.

`e539402` only silences one compiler warning in the `pingora` crate.

### Stability patches

| Commit | What it does |
|---|---|
| `7ffe1c4` | The whole HTTP/1 request header is bounded by `read_timeout`, not each read |
| `499f89d` | The `Server` header on Pingora's own responses can be configured (`set_server_header`) |

`7ffe1c4`: Pingora applied `read_timeout` (60 seconds) to each read while a request header came
in, so a client sending one byte every few seconds (slowloris) could hold the connection and its
buffer forever. Now, once the first header byte arrives (including leftover pipelined bytes), the
rest of the header must be complete before `now + read_timeout`; otherwise the read fails with
`ReadTimedout` ("request header incomplete after ..."). Waiting for the next request on a
keep-alive connection (before its first byte) is unchanged; conduit bounds that with
`defaults.keepalive_timeout`. Tests: `read_timeout_bounds_the_whole_request_header` in
`pingora-core`, and conduit's e2e `slow_request_head_is_cut_off` (the connection is cut after
62 seconds).

`499f89d`: responses that Pingora writes itself rather than the application (400 for a request
header that cannot be parsed, 405 for `CONNECT`, 502 when `proxy_upstream_filter` refuses, purge
answers) always carried `Server: Pingora`. `pingora_core::protocols::http::set_server_header()`
sets the value for the whole process, or drops the header with `None`; the default stays
`Pingora`. Those responses are now built each time (an error path, rare) instead of copied from a
static built once, so a change on reload applies at once. conduit calls it at start and on every
reload with `defaults.server_header` (default `conduit`, `off` for none). Tests: `server_header_follows_the_setting` in
`pingora-core`, and conduit's e2e `server_header`.

### Backports from upstream

Fixes that are already on an upstream branch but not in a release yet. These commits are dropped
when rebasing onto a release that contains them.

| Commit | Origin | What it does |
|---|---|---|
| `be20dfc` | upstream `b5dea5a` (PR #1021, issue #1013) | Brotli streams are finished properly at the end of the body |

`be20dfc`: Pingora 0.9.1's brotli compressor only *flushed* at the end of the body, so the final
meta-block (ISLAST) was never written. Strict decoders (`brotli -d`, the `brotli` crate) reject
such a response as a truncated stream. This hit conduit whenever a client put `br` first in
`Accept-Encoding`, because Pingora picks the first algorithm in that list. Browsers put `gzip`
first, so they were not affected. Tests: the brotli tests in `pingora-core` (brought over from
upstream), and conduit's e2e `file_server`, which decodes `br`, `zstd`, and `gzip` responses with
strict decoders. Without this fix that test fails with `br: Unexpected EOF`.

### `79bf391`: upstream connection pool without tasks

In stock Pingora, every time an upstream connection went back to the pool, Pingora spawned a
tokio task that held the connection behind an `Arc<Mutex>` and waited on a oneshot, a `Notify`,
and a timer. The next request had to wake that task and wait for it to finish before it could use
the connection. For a busy keep-alive upstream that meant a spawned task, a timer, several
allocations, and two task wake-ups on every request.

Now `TransportConnector` keeps idle connections in a pool of its own
(`pingora-core/src/connectors/idle.rs`):

- Per reuse key, the most recently returned connection is used first. The global size limit
  (`upstream_keepalive_pool_size`) and eviction of the oldest connection stay the same.
- Nobody polls idle connections. A connection is checked when it is taken, and one sweeper task
  per pool checks them all every second: connections closed by the server, holding unexpected
  data, or past the idle timeout are closed.
- Both checks see a FIN, an RST, or data that arrived while idle, because the connection stays
  registered with tokio's I/O driver, which keeps recording its readiness.

Differences from stock:

- A connection the server closed stays open on conduit's side until the next sweep (at most about
  1 second) or until it is taken.
- The idle timeout can be up to 1 second late. A connection past its timeout is never handed to a
  request.
- `pingora_pool::ConnectionPool` is unchanged and still used for HTTP/2.

### Small patches

- `32d98ca`: Pingora called `getpeername()` every time a connection was reused, to make sure it
  really pointed at the right peer. The TCP connector already records that address in the
  connection's digest at connect time, so `HttpPeer` now compares against that record. Peers
  behind a proxy and peers that override `matches_fd()` keep the old way.
- `ecef83b`: with compression on, every response had its MIME type checked, even when the client
  sent no `Accept-Encoding`. The regex is replaced by `starts_with` over the same list. A test pins
  the old regex's results, including case sensitivity.
- `9586c10`: every request to an HTTP/1 upstream ran ten hop-by-hop header lookups that almost
  never found anything. Now the header names present are checked in one pass first.
- `c448eff`: `append_header` and `insert_header` turned the name into its written form and then
  parsed it back, even for `HeaderName` constants. String names with upper case were parsed,
  lower-cased, and copied on every request. `IntoCaseHeaderName` gains an
  `into_header_name_and_case()` method (whose default still parses), and a new type,
  `CasedHeaderName`, keeps an already parsed name together with its written form. conduit uses it
  for `X-Forwarded-For`, `X-Real-IP`, `X-Forwarded-Proto`, `X-Forwarded-Host`, and `x-request-id`.
  The written form of every header on the wire is unchanged.

### `9c64421`: header name case without a second map

Pingora keeps the written form of every header name (`Content-Type` vs `content-type`) so that
forwarded headers are written exactly as they were received. It did that with a second HeaderMap
alongside the value map, built while parsing, cloned with it, and dropped with the header. Yet the
names in one message almost always follow one style: all lower case (hyper, many HTTP/2-era
programs) or upper case at the start and after `-` (nginx, curl, browsers).

Now a header keeps that style plus at most 8 names that do not follow it
(`pingora-http/src/name_case.rs`). The full map is only built when a message mixes more styles or
one name arrives in two forms, and it is laid out the same as the map that used to be kept from
the start. The bytes on the wire are identical; a randomized test of 3,000 rounds compares them
with the old way. In the profile, the `HeaderMap` share dropped from 8.7% to 6.3% of user samples,
but throughput barely moved: comparing names to find the style eats most of the saving.

### `3357c9e`: one flush per response batch

`response_duplex_vec()` wrote a header without Content-Length, then each chunk, then the last
chunk, and flushed after each one. A small compressed response went out as three `send()`s and
three TCP segments: header, body, `0\r\n\r\n` (seen with an LD_PRELOAD syscall counter). Now the
batch is flushed once at the end:

- If the batch ends the stream, the body, the last chunk, and the flush run under the body's write
  timeout.
- If not, a header that asks to be flushed (1xx, or without Content-Length, like server-sent
  events) is still flushed before returning.
- A single task (`response_duplex`) is flushed as before.

A new test counts flushes through `FlushTrackingMock`: a whole chunked response 3 → 1 flush, a
response with Content-Length 2 → 1.

### `0a6aee4`: HTTP modules without a shared index

`HttpModuleCtx` looked up modules (e.g. `ResponseCompression`) through an
`Arc<HashMap<TypeId, usize>>` shared by every context. Each request cloned that `Arc` when its
context was created and released it at the end, so every request on every thread raised and
lowered the same reference count; every `get`/`get_mut` also hashed the `TypeId` with SipHash. A
request has only a few modules, so modules are now looked up in order with a downcast, with no
shared state. The check that each module type is added only once is still there, done when the
first context is created. Before the patch, the module code took 1.8% of worker samples.

### `bb3859d`: upstream response header without a copy

`read_response_task()` returned a copy of the response header and kept the original. The proxy
reads every upstream response through this function, so every request paid for a `HeaderMap`
clone (all names and values, plus the case map) and then threw away two copies.

Once the header is handed over, the session needs little of it: the body framing is already set
up, leaving the status and the keep-alive decision. Now the final response header is moved into
the task. The session records the status and `header_keepalive()` (the header part of
`respect_keepalive()`, split out unchanged) before the header leaves. `resp_header()` and
`get_header()` still answer: the header is parsed again from the raw bytes the first time it is
asked for, the same way `read_response()` does. 1xx headers are still copied, because the session
still has to read past them.

The tests compare the handed-over header, the re-parsed header, and the keep-alive result with
`read_response()` for 9 cases (keep-alive, timeout, close from the request or the response,
HTTP/1.0, HTTP/1.0 chunked, TE together with CL, mixed and repeated header names).

### `c4cce19`: an HTTP/1 task channel of its own

`proxy_1to1()` created two `tokio::sync::mpsc` channels per request: the request body to the
upstream half and response tasks back to the downstream half. Creating them (an Arc holding a
semaphore and a first block of 32 slots), the permits taken and returned on every turn of the
downstream loop, sending, receiving, and dropping the receiver took 3.1% of worker samples.

Both halves are futures in one task, so the channel is never contended and only carries a few
tasks. `task_pipe` is a 4-slot ring in a single allocation, with the part of the mpsc API the H1
path uses (`try_reserve`, `reserve`, `Permit`, `send`, `is_closed`, `recv`) and the same
semantics:

- State is guarded by a small spinlock: acquire = one compare-and-swap, release = a plain store.
  The first version used `std::sync::Mutex`, which releases with an atomic swap to look for a
  sleeping thread, and those two instructions were half the channel's cost (a 2.5% share).
- Each end stores its waiting waker plus a flag and wakes it by reference, so waiting again does
  not clone and drop the waker.

The limit: one task per end (in the proxy both are polled by the same task). The h2, custom, and
subrequest paths keep using tokio channels. Besides 6 channel unit tests, conduit's e2e
`request_and_response_bodies` sends bodies from 0 B to 3 MB with Content-Length and chunked to
fast and slow upstreams (backpressure), parallel uploads, and a 20 MB response.

## Tests

Pingora's tests need a target directory without spaces, because jemalloc (a Pingora
dev-dependency) refuses to build in a path that contains spaces:

```sh
cd vendor/pingora/pingora-core
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=$HOME/.cache/conduit-pingora-target cargo test --lib --features openssl
```

The target directory is on a regular disk, not `/tmp`: Pingora's test build is over 5 GB and
fills a tmpfs quota.

What is run for this fork: all unit tests of `pingora-http`, `pingora-core` (with OpenSSL), and
`pingora-proxy` (lib), plus conduit's unit, e2e, ACME (Pebble), and CrowdSec tests. The
`pingora-proxy` integration tests (`tests/`) do not run here: they use `openresty` as the origin.

## Updating to a new Pingora

```sh
cd vendor/pingora
git fetch --depth 1 origin tag 0.X.Y
git rebase --onto 0.X.Y 0.9.1 conduit
```

Then change the version in conduit's `Cargo.toml` and measure every patch again with
`scripts/ab.sh`. Patches that upstream has accepted are dropped during the rebase.

## Upstreaming

`git format-patch 0.9.1..conduit` produces one file per patch, ready to propose to
cloudflare/pingora. The clearest candidates: `32d98ca`, `ecef83b`, `9586c10`, `c448eff`,
`3357c9e`, and `bb3859d`; also `7ffe1c4` (the slowloris fix) and `499f89d` (the `Server` header).
`c4cce19` uses `unsafe` (a spinlock) and changes the channel type on the H1 path, so it is better
discussed first. `79bf391` changes the pool's behavior (idle connections closed late), so it also
needs a discussion with the maintainers first.

## Upstream status (checked 2026-11-09)

The latest release is still 0.9.1. What was checked: the 74 branches of cloudflare/pingora (the
newest, `ewang/ci-fuzz-soak-nextest`, is 36 commits ahead of 0.9.1; `main` 20 commits), and the PRs
and issues matching each patch's keywords. Upstream rewrites SHAs during its internal sync, so old
branches were compared by code content, not by SHA.

None of our patches has been done upstream. Where they touch:

| Our patch | Upstream | What it means for the fork |
|---|---|---|
| `79bf391` (pool without tasks) | PR #1005 + `56965c3` fix a race between `reused_stream` and the idle poller (issue #998) on the newest branch | That race does not exist in the fork: the poller is gone. On rebase, `connectors/mod.rs` conflicts; keep our version |
| `32d98ca` (reuse check without `getpeername()`) | PR #971 (open): virtual L4 streams fail in `getpeername()` | A neighboring problem with a different motive. Our patch is still needed |
| `499f89d` (`Server` header) | Issue #644 closed with a workaround in `response_filter`/`fail_to_proxy`; Pingora's own responses still send `Server: Pingora` on the newest branch | Patch still needed |
| `7ffe1c4` (slowloris) | Issue #447 (downstream timeouts) and PR #1000 (header size limits, open); still no deadline for the whole header | Patch still needed; PR #1000 complements it rather than replacing it |
| `9586c10` (hop-by-hop) | PR #734 (hop-by-hop removal) is already in 0.9.x | Our patch speeds up that code; still needed |

Other unreleased upstream fixes to bring along when moving to the next release:

- `b5dea5a` brotli: already backported as `be20dfc`.
- `9fe75ee` + `7de8d9d` (issue #981): a no-steal runtime that has already shut down can be handed
  to a new thread. conduit uses no-steal runtimes (`work_stealing: false`), but never builds a new
  runtime in the middle of a process, so it cannot be reached today.
- `1b241f7`: empty read buffers are released while waiting for input (memory of idle
  connections). It conflicts with `7ffe1c4` in `v1/server.rs`.
- `1adcb38`: a panic on a backend with weight 0. conduit already refuses weight 0 when loading the
  config.

Open issues that do not affect conduit: #1024 (an HTTP/2 upstream pool leak; conduit only speaks
HTTP/1 to upstreams). The zstd advisories (RUSTSEC-2026-0395/0402/0403) are already covered:
`Cargo.lock` uses `zstd` 0.14.3 and `zstd-safe` 8.2.0.
