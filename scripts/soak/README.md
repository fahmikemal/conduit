# Native release soak on a disposable host

The expanded harness covers HTTP/1, HTTPS and frontend HTTP/2, native upstream h2c and
TLS HTTP/2, WebSocket, and bidirectional gRPC over both H2 transports. Eight oha streams
supply about 950 requests/s, plus WebSocket and verified gRPC messages. Origins and clients
listen/connect only on loopback. The host needs nginx with the `http2` directive (1.25.1+),
Python 3.12+, OpenSSL, nftables, systemd and passwordless sudo for the named test unit/table.

## Prepare the artifacts

Build the release through the normal release pipeline or select a previously validated
release whose source hashes match the checkout. Supply its full provenance where available:

```sh
uv pip install --target local/soak-python -r scripts/soak/requirements.txt
SOAK_BIN=/absolute/path/to/release/conduit \
OHA_BIN=/absolute/path/to/oha \
SOAK_PYTHON_LIB="$PWD/local/soak-python" \
SOAK_RELEASE_PROVENANCE=/absolute/path/to/release-provenance.json \
scripts/soak/deploy.sh HOST 24
```

`SOAK_RELEASE_PROVENANCE` is optional for general use; when supplied it must contain
`release_binary_sha256` and `source_sha256` and they are checked before staging. The test
uses self-signed, disposable fixture certificates; verification is disabled only for those
loopback fixture connections. Do not reuse those TLS settings as production configuration.

## Smoke before the full run

Use the same binaries/dependencies with the following settings for a seven-minute smoke:

```sh
SOAK_SCALE=40 SOAK_SECONDS=420 SOAK_UPGRADE_AT='1 3' \
SOAK_BIN=/absolute/path/to/release/conduit OHA_BIN=/absolute/path/to/oha \
SOAK_PYTHON_LIB="$PWD/local/soak-python" scripts/soak/deploy.sh HOST 24
```

Run the focused client regressions before deploying:

```sh
SOAK_PYTHON_LIB="$PWD/local/soak-python" python3 scripts/soak/test_grpc.py -v
```

Wait for completion, inspect `~/soak/out/report.txt` and require `checks.json` to pass.
Then invoke the full command without the smoke overrides. Default upgrades occur around
hours 2 and 6, leaving the final process running about 18 hours to reveal resource growth.
The `hours=24` field in the smoke start event is overridden by `target_seconds=420`.

Deployment refuses to replace an active run, rejects incomplete output, checks fixture ports,
and stages/verifies a hashed bundle before starting. Completed `out/`, `run/` and previous
input binaries/scripts are preserved under a unique `~/soak/archive/run-*` directory. Only
the disposable fixture processes and dedicated `inet soak` nftables table are cleaned up.
The deployer does not stop an unrelated nginx system service or replace global Python packages.

## What is checked

- HTTP/2 origins return their observed `$server_protocol`; probes require `HTTP/2.0`.
- gRPC clients use TLS HTTP/2 to conduit and validate payload hashes/order, 2 MiB framed
  messages, responses before request EOF, and grpc-status 0/7 trailers. Both cleartext and
  TLS upstream paths are used. The fixture uses the pinned pure-Python hyper-h2 stack.
  The client reconnects on GOAWAY, including when a shutdown PING shares the TCP read;
  these interrupted RPCs count as errors and are accepted only in upgrade windows.
- Reloads alternate upstream stream limits 32/64 while existing streams remain active.
- Backend resets, reloads, certificate/log rotation and graceful upgrades run periodically.
- CSV records process RSS/FD/threads, protocol probes and certificate fingerprints.
- The report separates oha window-deadline cancellations from request failures and correlates
  observed failures with injected disruptions. This correlation is not universal causal proof.
- `report.py OUT_DIR --strict` produces `checks.json` and fails on missing streams/results,
  panic/dead samples, log-drop/reuse-loss counters, unexpected gRPC/probe failures, or missing
  certificate-rotation evidence. Payload/format mismatches are not accepted as upgrade errors.

The proxy data path is unchanged by these test tools. A completed run is workload-specific
stability evidence, not proof of no leaks or an availability SLA. Inspect raw results and the
per-process trend; aggregate medians across restarts can hide growth within one process.

## Read results

```sh
ssh HOST systemctl status conduit-soak
ssh HOST cat soak/out/report.txt
ssh HOST cat soak/out/checks.json
python3 -I scripts/soak/report.py /path/to/copied/out --strict
```

The server unit is transient and may disappear after successful completion; the end marker,
final report/checks and systemd journal together establish the outcome. `run-manifest.json`
records artifact/dependency hashes, source revision, release provenance and environment.
