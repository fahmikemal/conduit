#!/usr/bin/env python3
"""Sends the same requests to NPM and to conduit and reports where they differ.

    scripts/parity.py CONFIG NPM_ADDR CONDUIT_ADDR [--http-port A,B] [--https-port A,B]
                      [--echo] [--cases FILE] [--full FILE]

CONFIG is a conduit config; its site hosts are requested at /, over HTTP and HTTPS, with a
browser and a non-browser User-Agent. Addresses are IPs; ports default to 80/443 for both,
e.g. --http-port 80,18080 --https-port 443,18443 when conduit runs beside NPM.
Compared: status, Location, Content-Type, Content-Encoding and a few headers.

--echo: the upstreams are scripts/parity/stub.py, which answers with what it received; the
requests NPM and conduit forwarded are compared too (target, method, headers). Client
addresses are shown as CLIENT, since the two proxies see the test client differently.

--cases FILE: more requests, one per line: `METHOD HOST PATH [| Name: value]...`, sent over
HTTPS (and HTTP when the line starts with `http `). Only for local tests: any method goes.

Differences are summarized per field; --full FILE gets every request with what each proxy
answered and forwarded. A port in Host, X-Forwarded-Host and Location is dropped, since it only
reflects the test ports.
"""

import argparse
import gzip
import json
import re
import subprocess
import sys
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor

UAS = {
    "browser": "Mozilla/5.0 (X11; Linux x86_64) conduit-parity",
    "client": "conduit-parity/1.0",
}
HEADERS = [
    "location",
    "content-type",
    "content-encoding",
    "access-control-allow-origin",
    "access-control-allow-methods",
    "access-control-allow-headers",
    "cache-control",
    "strict-transport-security",
]
CLIENTS = re.compile(r"\b(127\.0\.0\.1|172\.17\.0\.1|::1)\b")


def hosts_from(config: str) -> list[str]:
    text = open(config).read()
    sites = text.split("\nsites:\n", 1)[1]
    out = []
    for block in re.split(r"\n(?=  - id:)", sites):
        if re.search(r"^\s+enabled:\s*false", block, re.M):
            continue
        m = re.search(r"^\s+hosts:\s*\[(.*)\]", block, re.M)
        if m:
            out += [h.strip().strip('"') for h in m.group(1).split(",") if "*" not in h]
    return out


def fetch(ip, port, scheme, host, path, method, headers, echo):
    url = f"{scheme}://{host}:{port}{path}"
    cmd = ["curl", "-sk", "--path-as-is", "-D", "-", "-o", "-" if echo else "/dev/null",
           "--max-time", "20", "--resolve", f"{host}:{port}:{ip}"]
    cmd += ["-I"] if method == "HEAD" else ["-X", method]
    for h in headers:
        cmd += ["-H", h]
    r = subprocess.run(cmd + [url], capture_output=True)
    if r.returncode not in (0, 23):
        return {"status": f"curl-{r.returncode}"}
    raw = r.stdout
    # Header blocks (a 100 Continue, then the response), then the body.
    head, body = raw, b""
    pos = 0
    while raw.startswith(b"HTTP/", pos):
        end = raw.find(b"\r\n\r\n", pos)
        if end < 0:
            break
        head, pos = raw[:end], end + 4
    body = raw[pos:]
    lines = head.decode("latin-1").split("\r\n")
    start = max(i for i, l in enumerate(lines) if l.startswith("HTTP/"))
    parts = lines[start].split()
    res = {"status": parts[1] if len(parts) > 1 else "?"}
    for line in lines[start + 1:]:
        if ":" in line:
            k, v = line.split(":", 1)
            k = k.strip().lower()
            if k in HEADERS:
                v = v.strip()
                if k == "content-type":
                    v = v.split(";")[0].strip().lower()
                if k == "location":
                    # a port in our own redirect only reflects the test port
                    v = re.sub(r"^(https?://[^/:]+):\d+", r"\1", v)
                res[k] = v
    if echo and res.get("content-type") == "application/json" and res["status"] == "200":
        try:
            if res.get("content-encoding") == "gzip":
                body = gzip.decompress(body)
            seen = json.loads(body)
        except ValueError:
            seen = None
        if isinstance(seen, dict) and "headers" in seen:
            res["up.port"] = str(seen["port"])
            res["up.method"] = seen["method"]
            res["up.target"] = seen["target"]
            res["up.body_len"] = str(seen["body_len"])
            hs = defaultdict(list)
            for k, v in seen["headers"]:
                hs[k].append(v)
            for k, vs in hs.items():
                v = CLIENTS.sub("CLIENT", ", ".join(vs))
                if k in ("host", "x-forwarded-host"):
                    v = re.sub(r":\d+$", "", v)
                res[f"up.{k}"] = v
            if "up.x-request-id" in res:
                res["up.x-request-id"] = "<id>"
    return res


def read_cases(path):
    out = []
    for line in open(path):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = [p.strip() for p in line.split("|")]
        words = parts[0].split()
        schemes = ["https"]
        if words[0] == "http":
            schemes, words = ["http", "https"], words[1:]
        method, host, path = words[0], words[1], words[2]
        for scheme in schemes:
            out.append((host, scheme, method, path, tuple(parts[1:])))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("config")
    ap.add_argument("npm")
    ap.add_argument("conduit")
    ap.add_argument("--http-port", default="80,80")
    ap.add_argument("--https-port", default="443,443")
    ap.add_argument("--echo", action="store_true")
    ap.add_argument("--cases")
    ap.add_argument("--full")
    a = ap.parse_args()
    hp = [int(x) for x in a.http_port.split(",")]
    sp = [int(x) for x in a.https_port.split(",")]

    cases = []
    for host in hosts_from(a.config):
        cases.append((host, "http", "GET", "/", (f"User-Agent: {UAS['client']}",)))
        for ua in UAS.values():
            cases.append((host, "https", "GET", "/", (f"User-Agent: {ua}",)))
    if a.cases:
        cases += read_cases(a.cases)

    def run(case):
        host, scheme, method, path, headers = case
        ports = hp if scheme == "http" else sp
        old = fetch(a.npm, ports[0], scheme, host, path, method, headers, a.echo)
        new = fetch(a.conduit, ports[1], scheme, host, path, method, headers, a.echo)
        return case, old, new

    same = 0
    diffs = []
    results = []
    with ThreadPoolExecutor(8) as pool:
        for case, old, new in pool.map(run, cases):
            results.append((case, old, new))
            fields = {k: (old.get(k), new.get(k)) for k in set(old) | set(new) if old.get(k) != new.get(k)}
            if fields:
                diffs.append((case, fields))
            else:
                same += 1

    by_field = defaultdict(list)
    for case, fields in diffs:
        for k, v in fields.items():
            by_field[(k, v)].append(case)
    print(f"{same} identical, {len(diffs)} different, {len(cases)} requests per proxy\n")
    def shape(case, v):
        # One line per kind of difference: the request's own host reads <host>.
        return v.replace(case[0], "<host>") if isinstance(v, str) else v
    grouped = defaultdict(list)
    for (k, (o, n)), cs in by_field.items():
        for c in cs:
            grouped[(k, shape(c, o), shape(c, n))].append(c)
    by_field = {(k, (o, n)): cs for (k, o, n), cs in grouped.items()}
    for (k, (o, n)), cs in sorted(by_field.items(), key=lambda x: (x[0][0], -len(x[1]))):
        host, scheme, method, path, headers = cs[0]
        extra = f" {' | '.join(headers)}" if headers else ""
        print(f"{k:<26} npm={o!r} conduit={n!r}  ×{len(cs)}  e.g. {method} {scheme}://{host}{path}{extra}")
    if a.full:
        with open(a.full, "w") as f:
            for (host, scheme, method, path, headers), old, new in results:
                f.write(f"{method} {scheme}://{host}{path} {' | '.join(headers)}\n")
                for k in sorted(set(old) | set(new)):
                    mark = "  " if old.get(k) == new.get(k) else "≠ "
                    f.write(f"  {mark}{k:<26} npm={old.get(k)!r}  conduit={new.get(k)!r}\n")
    return 0 if not diffs else 1


if __name__ == "__main__":
    sys.exit(main())
