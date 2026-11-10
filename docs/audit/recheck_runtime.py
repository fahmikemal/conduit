#!/usr/bin/env python3
"""Recheck selected findings against an actual conduit binary, on loopback only.

Usage: python3 docs/audit/recheck_runtime.py /path/to/conduit --out results.json
Tests record observed behavior; a reproduced behavior is not always a product bug.
All server state and keys are disposable. Application source is never modified.
"""
import argparse
import contextlib
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import signal
import socket
import ssl
import subprocess
import tempfile
import threading
import time
import zlib


def port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def request(p, host, path, headers=None, method="GET"):
    c = http.client.HTTPConnection("127.0.0.1", p, timeout=5)
    try:
        c.request(method, path, headers={"Host": host, "Connection": "close", **(headers or {})})
        r = c.getresponse()
        return {"status": r.status, "headers": {k.lower(): v for k, v in r.getheaders()},
                "body": r.read()}
    finally:
        c.close()


def summary(r):
    return {"status": r["status"], "headers": r["headers"], "body_hex": r["body"].hex(),
            "body_text": r["body"].decode("utf-8", errors="replace")}


def wait_for(fn, timeout=10):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            value = fn()
            if value:
                return value
        except (OSError, ValueError, http.client.HTTPException):
            pass
        time.sleep(0.03)
    raise TimeoutError("condition not met")


class Origin(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        with self.server.lock:
            self.server.seen.append({"path": self.path, "host": self.headers.get("Host"),
                                     "accept_encoding": self.headers.get("Accept-Encoding")})
        varying = self.path.startswith("/vary")
        deflate = varying and "deflate" in self.headers.get("Accept-Encoding", "")
        if varying:
            body = b"deflate-origin" if deflate else b"identity-origin"
        elif self.path.startswith("/authority"):
            body = json.dumps({"host": self.headers.get("Host"),
                               "xfh": self.headers.get("X-Forwarded-Host")}).encode()
        else:
            body = self.server.label.encode()
        if deflate:
            body = zlib.compress(body)
        self.send_response_only(200)
        self.send_header("Server", "origin-marker")
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "public, max-age=600")
        if varying:
            self.send_header("Vary", "Accept-Encoding")
        if deflate:
            self.send_header("Content-Encoding", "deflate")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


@contextlib.contextmanager
def origin(label):
    s = ThreadingHTTPServer(("127.0.0.1", 0), Origin)
    s.daemon_threads = True
    s.label, s.seen, s.lock = label, [], threading.Lock()
    worker = threading.Thread(target=s.serve_forever, daemon=True)
    worker.start()
    try:
        yield s
    finally:
        s.shutdown()
        s.server_close()
        worker.join()


@contextlib.contextmanager
def process(binary, cfg, log):
    with log.open("wb") as f:
        p = subprocess.Popen([binary, "run", "-c", str(cfg)], stdout=f, stderr=f)
        try:
            yield p
        finally:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    p.kill()
                    p.wait(timeout=5)


def cert(root, name):
    crt, key = root / (name + ".crt"), root / (name + ".key")
    subprocess.run(["openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt",
                    "ec_paramgen_curve:P-256", "-nodes", "-days", "1", "-subj", f"/CN={name}",
                    "-addext", "subjectAltName=DNS:*.example.test", "-keyout", str(key),
                    "-out", str(crt)], check=True, capture_output=True)
    der = ssl.PEM_cert_to_DER_cert(crt.read_text())
    return crt, key, hashlib.sha256(der).hexdigest()


def fingerprint(p, host):
    c = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    c.check_hostname = False
    c.verify_mode = ssl.CERT_NONE
    with socket.create_connection(("127.0.0.1", p), timeout=5) as raw:
        with c.wrap_socket(raw, server_hostname=host) as s:
            return hashlib.sha256(s.getpeercert(binary_form=True)).hexdigest()


def main():
    a = argparse.ArgumentParser()
    a.add_argument("binary")
    a.add_argument("--out", required=True)
    a.add_argument("--expect-fixed", action="store_true", help="assert closure using the same original requests and controls")
    args = a.parse_args()
    binary = str(Path(args.binary).resolve())
    results = {"binary": binary, "binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
               "version": subprocess.check_output([binary, "--version"], text=True).strip(),
               "tests": {}}
    with tempfile.TemporaryDirectory(prefix="conduit-recheck-") as tmp, origin("origin-A") as oa, origin("origin-B") as ob:
        d = Path(tmp)
        www = d / "www"
        (www / "admin").mkdir(parents=True)
        (www / "admin/secret.txt").write_text("protected-file-marker")
        old, chosen = cert(d, "old"), cert(d, "chosen")
        hp, ap, tp = port(), port(), port()
        cfg, log = d / "config.yaml", d / "conduit.log"

        def write(up):
            cfg.write_text(f'''server:
  threads: 4
  shutdown_grace: 0s
  shutdown_timeout: 1s
  pid_file: {d}/conduit.pid
  upgrade_sock: {d}/upgrade.sock
listen:
  http: ["127.0.0.1:{hp}"]
  https: ["127.0.0.1:{tp}"]
admin: {{ listen: "127.0.0.1:{ap}" }}
defaults: {{ compression: false, server_header: off }}
tls:
  certs:
    - {{ name: old, cert: {old[0]}, key: {old[1]} }}
    - {{ name: chosen, cert: {chosen[0]}, key: {chosen[1]} }}
sites:
  - id: files
    hosts: [files.test]
    files: {{ root: {www} }}
    plugins:
      - rules:
          - match: {{ path: {{ prefix: /admin }} }}
            action: {{ deny: 403 }}
  - id: cached
    hosts: [cache.test]
    upstream: http://127.0.0.1:{up}
    cache: true
  - id: wild
    hosts: ["*.example.test"]
    cert: chosen
    upstream: http://127.0.0.1:{up}
  - id: exact
    hosts: [exact.example.test]
    cert: chosen
    upstream: http://127.0.0.1:{up}
''')

        write(oa.server_port)
        check = subprocess.run([binary, "check", "-c", str(cfg)], text=True, capture_output=True)
        assert check.returncode == 0, check.stderr
        results["initial_config_check"] = check.stdout
        with process(binary, cfg, log) as p:
            wait_for(lambda: request(ap, "localhost", "/healthz")["status"] == 200)
            controls = [request(hp, "files.test", path) for path in
                        ["/admin/secret.txt", "/%61dmin/secret.txt", "/%2561dmin/secret.txt"]]
            assert [r["status"] for r in controls] == [403, 403, 404 if args.expect_fixed else 200]
            if not args.expect_fixed:
                assert controls[-1]["body"] == b"protected-file-marker"
            results["tests"]["F01"] = {"reproduced": True, "responses": list(map(summary, controls))}

            r1 = request(hp, "cache.test:8080", "/authority")
            r2 = request(hp, "cache.test:9090", "/authority")
            rc = request(hp, "cache.test:9090", "/authority?control")
            assert json.loads(r2["body"])["host"] == ("cache.test:9090" if args.expect_fixed else "cache.test:8080")
            assert r2["headers"]["cache-status"] == ("conduit; fwd=miss; stored" if args.expect_fixed else "conduit; hit")
            assert json.loads(rc["body"])["host"] == "cache.test:9090"
            results["tests"]["F03"] = {"reproduced": True, "first": summary(r1),
                                       "second": summary(r2), "uncached_control": summary(rc)}

            rv1 = request(hp, "cache.test", "/vary", {"Accept-Encoding": "deflate"})
            rv2 = request(hp, "cache.test", "/vary", {"Accept-Encoding": "identity"})
            rvc = request(hp, "cache.test", "/vary?control", {"Accept-Encoding": "identity"})
            if args.expect_fixed:
                assert rv2["headers"].get("content-encoding") is None
                assert rv2["body"] == b"identity-origin"
            else:
                assert rv2["headers"].get("content-encoding") == "deflate"
                assert zlib.decompress(rv2["body"]) == b"deflate-origin"
                assert rv2["headers"]["cache-status"] == "conduit; hit"
            assert rvc["body"] == b"identity-origin"
            results["tests"]["F09"] = {"reproduced": True, "first": summary(rv1),
                                       "second": summary(rv2), "uncached_control": summary(rvc)}

            wild_fp, exact_fp = fingerprint(tp, "app.example.test"), fingerprint(tp, "exact.example.test")
            assert wild_fp == (chosen[2] if args.expect_fixed else old[2]) and exact_fp == chosen[2]
            results["tests"]["F08"] = {"reproduced": True, "wildcard_selected": "chosen" if args.expect_fixed else "old",
                                       "exact_control_selected": "chosen", "configured_cert": "chosen"}
            assert rc["headers"].get("server") == "origin-marker"
            results["tests"]["F10"] = {"reproduced": True, "kind": "documented pass-through, README mismatch only",
                                       "server_header_off_observed": rc["headers"]["server"]}

            before = request(hp, "cache.test", "/generation")
            write(ob.server_port)
            p.send_signal(signal.SIGHUP)

            def reloaded():
                st = json.loads(request(ap, "localhost", "/status")["body"])
                return any(b["address"] == f"127.0.0.1:{ob.server_port}"
                           for pool in st["upstreams"] for b in pool["backends"])

            wait_for(reloaded)
            after = request(hp, "cache.test", "/generation")
            request(ap, "localhost", "/cache/purge?site=cached", method="POST")
            purged = request(hp, "cache.test", "/generation")
            assert before["body"] == after["body"] == b"origin-A"
            assert purged["body"] == b"origin-B"
            results["tests"]["F04"] = {"reproduced": True, "kind": "documented cache retention, not sufficient evidence of bug",
                                       "before": summary(before), "after_reload": summary(after),
                                       "after_purge": summary(purged)}
        results["main_log"] = log.read_text(errors="replace")

        zero, zero_log = d / "zero.yaml", d / "zero.log"
        zp = port()
        zero.write_text(f'''server: {{ threads: 1, shutdown_grace: 0s, shutdown_timeout: 1s, pid_file: {d}/zero.pid, upgrade_sock: {d}/zero.sock }}
listen: {{ http: ["127.0.0.1:{zp}"] }}
tls: {{ watch_interval: 0s }}
''')
        zc = subprocess.run([binary, "check", "-c", str(zero)], text=True, capture_output=True)
        if args.expect_fixed:
            assert zc.returncode != 0 and "tls.watch_interval" in zc.stderr
            assert "panicked" not in zc.stderr
            results["tests"]["F06"] = {"closed": True, "check_returncode": zc.returncode, "error": zc.stderr}
        else:
            assert zc.returncode == 0
            with process(binary, zero, zero_log) as pz:
                wait_for(lambda: "panicked" in zero_log.read_text(errors="replace"))
                panic = zero_log.read_text(errors="replace")
                assert "period" in panic and "non-zero" in panic
                alive = pz.poll() is None
                status = request(zp, "unknown.test", "/")["status"] if alive else None
            results["tests"]["F06"] = {"reproduced": True, "check_returncode": zc.returncode,
                                       "process_alive_after_task_panic": alive, "http_status": status,
                                       "log": panic}
    results["mode"] = "closure" if args.expect_fixed else "vulnerable-baseline"
    if args.expect_fixed:
        for name, result in results["tests"].items():
            result.pop("reproduced", None)
            result["passed"] = True
            result["classification"] = "documented_behavior_preserved" if name in ("F04", "F10") else "closed"
    Path(args.out).write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps({"version": results["version"], "results": results["tests"]}, indent=2))


if __name__ == "__main__":
    main()
