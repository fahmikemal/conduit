#!/usr/bin/env python3
"""Bounded loopback soak of a real binary; requires openssl and oha.

Traffic covers HTTP/1, TLS, HTTP/2, cache, static files, gzip and WebSockets.
The existing repository WebSocket harness supplies 40 persistent connections.
All configuration, keys, logs and service state are disposable.
"""
import argparse
import contextlib
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import statistics
import subprocess
import sys
import tempfile
import time

from recheck_runtime import cert, fingerprint, origin, port, process, request, wait_for


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("binary")
    parser.add_argument("--oha", required=True)
    parser.add_argument("--seconds", type=int, default=600)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    assert args.seconds >= 60
    binary = str(Path(args.binary).resolve())
    output = Path(args.out).resolve()
    output.mkdir(parents=True, exist_ok=True)
    ws_script = Path(__file__).resolve().parents[2] / "scripts/soak/ws.py"
    result = {"binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
              "planned_seconds": args.seconds, "passed": False}
    samples, reloads, rotations, clients = [], 0, 0, []
    with tempfile.TemporaryDirectory(prefix="conduit-fixed-soak-") as tmp, \
            origin("origin-stability-marker-" * 100) as upstream, contextlib.ExitStack() as stack:
        root = Path(tmp)
        www = root / "www"
        www.mkdir()
        (www / "small.txt").write_text("static-stability-marker\n" * 100)
        (www / "large.txt").write_text("compressible-stability-marker\n" * 8192)
        bundles = []
        for i in range(3):
            crt, key, fp = cert(root, f"rotation-{i}")
            bundles.append((crt.read_bytes() + key.read_bytes(), fp))
        active = root / "active.pem"
        active.write_bytes(bundles[0][0])
        hp, tp, ap, wp = port(), port(), port(), port()
        cfg = root / "config.yaml"
        cfg.write_text(f'''server:
  threads: 4
  shutdown_grace: 0s
  shutdown_timeout: 1s
  pid_file: {root}/conduit.pid
  upgrade_sock: {root}/upgrade.sock
listen:
  http: ["127.0.0.1:{hp}"]
  https: ["127.0.0.1:{tp}"]
admin: {{ listen: "127.0.0.1:{ap}" }}
access_log: {{ path: {root}/access.log }}
tls:
  watch_interval: 1s
  default_cert: active
  certs: [{{ name: active, cert: {active}, key: {active} }}]
sites:
  - id: proxy
    hosts: [bench.test]
    upstream: http://127.0.0.1:{upstream.server_port}
    plugins: [{{ rate_limit: {{ requests: 100000000, per: 1h }} }}]
  - id: cache
    hosts: [cache.test]
    upstream: http://127.0.0.1:{upstream.server_port}
    cache: true
  - id: files
    hosts: [files.test]
    files: {{ root: {www} }}
  - id: websocket
    hosts: [ws.soak.test]
    upstream: http://127.0.0.1:{wp}
''')
        subprocess.run([binary, "check", "-c", str(cfg)], check=True, capture_output=True)

        def launch(command, name):
            out = stack.enter_context((output / name).open("wb"))
            child = subprocess.Popen(command, stdout=out, stderr=out)

            def stop():
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=5)
            stack.callback(stop)
            return child

        launch([sys.executable, str(ws_script), "server", str(wp)], "ws-server.log")
        wait_for(lambda: request(wp, "localhost", "/")["status"] == 200)
        log = output / "conduit.log"
        proxy = stack.enter_context(process(binary, cfg, log))
        wait_for(lambda: request(ap, "localhost", "/healthz")["status"] == 200)
        wait_for(lambda: request(hp, "bench.test", "/")["status"] == 200)
        assert fingerprint(tp, "bench.test") == bundles[0][1]
        started = time.monotonic()
        end = time.time() + args.seconds
        ws = launch([sys.executable, str(ws_script), "client", str(hp), str(tp),
                     str(end), str(output / "ws.csv"), str(output / "ws-events.log")], "ws-client.log")
        cases = [("http", "bench.test", "/", 200, False, False, False),
                 ("https", "bench.test", "/", 100, True, False, False),
                 ("h2", "bench.test", "/", 100, True, True, False),
                 ("cache", "cache.test", "/", 200, False, False, False),
                 ("files", "files.test", "/small.txt", 100, False, False, False),
                 ("gzip", "files.test", "/large.txt", 20, False, False, True)]
        for name, host, path, rate, tls, h2, gzip in cases:
            command = [args.oha, "--no-tui", "--output-format", "json", "--worker-threads", "1",
                       "-c", "8", "-n", str(rate * args.seconds), "-q", str(rate),
                       "--latency-correction", "--disable-compression", "-t", "10s"]
            if gzip:
                command += ["-H", "Accept-Encoding: gzip"]
            if h2:
                command += ["--http2"]
            if tls:
                command += ["--insecure", "--connect-to", f"{host}:{tp}:127.0.0.1:{tp}",
                            f"https://{host}:{tp}{path}"]
            else:
                command += ["--host", host, f"http://127.0.0.1:{hp}{path}"]
            clients.append((name, rate * args.seconds, launch(command, name + ".json")))
        next_reload, next_rotate, next_report = 10, 30, 30
        while time.monotonic() - started < args.seconds:
            elapsed = time.monotonic() - started
            assert proxy.poll() is None, "proxy exited during soak"
            assert ws.poll() is None, "WebSocket harness exited early"
            status = Path(f"/proc/{proxy.pid}/status").read_text()
            sample = {"seconds": round(elapsed, 2), "rss_kib": int(re.search(r"VmRSS:\s+(\d+)", status)[1]),
                      "threads": int(re.search(r"Threads:\s+(\d+)", status)[1]),
                      "fds": len(list(Path(f"/proc/{proxy.pid}/fd").iterdir()))}
            samples.append(sample)
            state = json.loads(request(ap, "localhost", "/status")["body"])
            assert state["config"]["last_reload"]["ok"], state["config"]["last_reload"]
            if elapsed >= next_rotate:
                rotations += 1
                data, expected_fp = bundles[rotations % len(bundles)]
                replacement = root / "replacement.pem"
                replacement.write_bytes(data)
                os.replace(replacement, active)
                # Exercise the periodic file watcher, independently of explicit SIGHUP.
                wait_for(lambda: fingerprint(tp, "bench.test") == expected_fp, timeout=5)
                next_rotate += 30
            if elapsed >= next_reload:
                proxy.send_signal(signal.SIGHUP)
                reloads += 1
                next_reload += 10
            if elapsed >= next_report:
                print(json.dumps({"sample": sample, "reloads_sent": reloads,
                                  "certificate_rotations_observed": rotations}), flush=True)
                next_report += 30
            time.sleep(2)
        for name, expected, child in clients:
            assert child.wait(timeout=30) == 0, name
            data = json.loads((output / (name + ".json")).read_text())
            assert sum(data["errorDistribution"].values()) == 0, (name, data["errorDistribution"])
            assert data["statusCodeDistribution"] == {"200": expected}, (name, data["statusCodeDistribution"])
        assert ws.wait(timeout=35) == 0
        ws_rows = list(csv.DictReader((output / "ws.csv").open()))
        final_ws = {k: int(v) for k, v in ws_rows[-1].items()}
        assert final_ws["errors"] == final_ws["reconnects"] == final_ws["short_err"] == 0, final_ws
        assert final_ws["msgs"] >= args.seconds * 35, final_ws
        assert final_ws["short_ok"] >= args.seconds, final_ws
        assert not (output / "ws-events.log").read_text()
        metrics = request(ap, "localhost", "/metrics")["body"].decode()
        (output / "metrics.txt").write_text(metrics)
        failures = re.search(r'conduit_config_reloads_total\{result="failure"\} (\d+)', metrics)
        assert failures is None or int(failures[1]) == 0
        assert 'conduit_access_log_dropped_total 0' in metrics
        successes = int(re.search(r'conduit_config_reloads_total\{result="success"\} (\d+)', metrics)[1])
        assert successes >= reloads + rotations, (successes, reloads, rotations)
        assert "panicked at" not in log.read_text()
        warmed = [s for s in samples if s["seconds"] >= 30]
        initial_rss = statistics.median(s["rss_kib"] for s in warmed[:15])
        final_rss = statistics.median(s["rss_kib"] for s in warmed[-15:])
        assert final_rss - initial_rss < 64 * 1024, (initial_rss, final_rss)
        assert max(s["fds"] for s in warmed) < 400
        assert max(s["threads"] for s in warmed) < 64
        assert max(s["rss_kib"] for s in warmed) < 256 * 1024
        result.update(passed=True, elapsed_seconds=round(time.monotonic() - started, 2),
                      http_requests=sum(n for _, n, _ in clients), websocket=final_ws,
                      reloads_sent=reloads, reloads_succeeded=successes, certificate_rotations_observed=rotations,
                      warmed_initial_rss_kib=initial_rss, warmed_final_rss_kib=final_rss,
                      max_rss_kib=max(s["rss_kib"] for s in warmed),
                      max_fds=max(s["fds"] for s in warmed), max_threads=max(s["threads"] for s in warmed))
        (output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
        (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2), flush=True)


if __name__ == "__main__":
    main()
