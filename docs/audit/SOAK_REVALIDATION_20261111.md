# Krynox soak revalidation — 11 November 2026

The previous 24-hour run completed. Its binary predates the current HTTP/2-upstream
release, so it is evidence for the older HTTP/1-upstream implementation. The next 24-hour
run uses the latest validated release binary and the expanded native harness described in
[`scripts/soak/README.md`](../../scripts/soak/README.md).

## Completed older run

- Start/end: 10 November 00:47:50 → 11 November 00:48:09 WIB; 86,419 seconds.
- Binary SHA-256: `a0009745671b39d671a6e20d66f82bb6da18ede83eacf5921113b0c5057f8fbd`.
- 56,153,878 requests, excluding 2,476 separate oha window-deadline cancellations.
  There were 68 failures: 34 POST 502s in backend-reset windows and 34 transport errors
  in upgrade windows. No failure window lacked a scheduled disruption. Event-window
  correlation supports attribution but does not prove every transport error's cause.
- 3,449,000 long-lived WebSocket echoes; 80 reconnects during two upgrades and zero
  outside those windows. 172,449 short WebSocket sessions had zero failures.
- 288 reloads, 24 certificate rotations, 143 backend-down/up cycles, 48 log rotations
  and two successful upgrades. No panic, dead-process sample, log-drop counter or
  upstream-reuse-loss counter was observed.
- Peak proxy RSS 49.58 MiB; median FD 167 and threads 12; final drained FD 53. The last
  process ran nearly 18 hours with bounded resources under this workload. This does
  not establish a universal absence of leaks or an availability SLA.
- Independent recomputation matches request/resource/failure results. The final stderr
  snapshot contains 23 additional INFO shutdown lines written after the server report.

The complete server output, runtime files and original inputs are preserved in
`~/soak/archive/run-20261110T180805Z/`. The local raw snapshot is in
`local/krynox-check-20261111/final/`; the committed [analysis JSON](soak-previous-analysis.json)
and [server report](soak-previous-report.log) retain metrics and evidence hashes.

## Latest release and harness checks

Latest release SHA-256:
`474e0cbf143e0abbda86f1ff6ae42c9a2bfc13ed309f515d86dae381a7a41429`.
The binary and application/build source hashes match
[the release provenance](http2-validation.json). Application sources are unchanged from
the validated HTTP/2 implementation; later revisions changed documentation and test tools.

- Five HTTP/2 release E2E cases passed again; the separate two-minute H2 load case was
  not selected in this rerun. The full application suite was not rerun during this
  harness-only change; its earlier results remain in [the H2 report](HTTP2_UPSTREAM_VALIDATION.md).
- Runtime audit closure checks passed again against this release.
- Local gRPC preflight through the release verified 388 messages, including 194 messages
  of 2 MiB, 97 completed bidirectional RPCs, 97 responses before request EOF and 13
  status-7 trailers, with zero client errors.
- Two focused fixture regressions passed: coalesced GOAWAY/PING preserves the GOAWAY
  event; corrupted echo payloads still raise a validation failure.
- Strict-report negative checks reject protocol errors and payload corruption even inside
  upgrade windows, and reject an HTTP/1 protocol-probe result. Typed shutdown errors in
  upgrade windows remain separately counted. An active-run deployment attempt was rejected.

The first seven-minute smoke exposed a fixture-client parsing issue: hyper-h2 parsed a
shutdown PING after GOAWAY closed its state machine, masking the GOAWAY event with a
`ProtocolError`. The client now passes one complete HTTP/2 frame at a time to hyper-h2,
handles GOAWAY immediately and reconnects. The strict classifier was not relaxed to
accept protocol/payload errors. Interrupted RPCs remain counted as errors; this fixture
does not claim uninterrupted gRPC stream draining across an upgrade.

## Expanded smoke outcome

The final smoke result and evidence hashes are recorded in
[soak-validation.json](soak-validation.json), with the raw summary in
[soak-smoke-report.log](soak-smoke-report.log). It uses the same release, pinned fixture
dependencies and eight load streams as the next 24-hour run. Event intervals are scaled
by 40, the traffic duration is 420 seconds, and two graceful upgrades are exercised.

The new 24-hour result remains pending. A passing smoke establishes harness readiness;
the full run must independently complete and pass its strict checks before a stability
conclusion is made for this release and workload.
