#!/usr/bin/env python3
"""conduit against other reverse proxies on this machine, all proxying the same upstream.

    scripts/compare.py [conduit-binary]        (default dist/conduit)

Proxies: nginx tuned, nginx as NPM configures it (HTTP only), Caddy, Apache httpd (event MPM),
HAProxy, Traefik, Envoy and conduit. Each runs in a container with host networking on the
same 2 CPUs (the production host has 2 vCPUs), configured as one would run it in production: 2
workers or threads, upstream keep-alive, TLS with the same ECDSA P-256 certificate, HTTP/2 on
TLS, X-Forwarded-For/Proto to the upstream, and an access log of every request to a file.
conduit runs in a container too (debian:trixie-slim with the binary mounted), so every
proxy is measured the same way: CPU and memory come from the container's cgroup.

Scenarios, as in scripts/bench.sh: HTTP/1.1 keep-alive, HTTPS keep-alive (HTTP/1.1),
HTTP/2, and a new TLS connection per request ("handshake"), each as fast as the proxy goes.
Every scenario runs ROUNDS times with the proxies in a rotating order, and the table shows
the median. "fixed" then holds one rate for every proxy, 50% of the slowest proxy's HTTP
peak (NPM-style excluded), and reports how much each proxy adds to the upstream's own
latency; latency counts from each request's scheduled start, so queueing shows.

Environment: N (50000 requests per run), C (64 connections), SIZE (2048 body bytes),
ROUNDS (3), RATE_SECONDS (10), PROXIES (comma list to run a subset), PROXY_CPUS (0,1),
UP_CPUS (2,3), LOAD_CPUS (4-7). Results go to target/bench/compare.json. Needs docker,
openssl and curl; oha comes from mise.toml. Local only: nothing here talks to a remote host.
"""
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "dist/conduit").resolve()
N = int(os.environ.get("N", 50000))
C = int(os.environ.get("C", 64))
SIZE = int(os.environ.get("SIZE", 2048))
ROUNDS = int(os.environ.get("ROUNDS", 3))
RATE_SECONDS = int(os.environ.get("RATE_SECONDS", 10))
PROXY_CPUS = os.environ.get("PROXY_CPUS", "0,1")
UP_CPUS = os.environ.get("UP_CPUS", "2,3")
LOAD_CPUS = os.environ.get("LOAD_CPUS", "4-7")

UP, UP_STATUS, CONDUIT_ADMIN = 18281, 18280, 18290
IMAGES = {
    "nginx": "nginx:1.28-alpine",
    "caddy": "caddy:2.11.9-alpine",
    "apache": "httpd:2.4.69-alpine",
    "haproxy": "haproxy:lts-alpine",
    "traefik": "traefik:v3.7.17",
    "envoy": "envoyproxy/envoy:v1.40-latest",
    "conduit": "debian:trixie-slim",
}
# name -> (http port, https port or None)
PROXIES = {
    "nginx (NPM-style)": (18282, None),
    "nginx": (18283, 18483),
    "conduit": (18284, 18484),
    "caddy": (18285, 18485),
    "apache": (18286, 18486),
    "haproxy": (18287, 18487),
    "traefik": (18288, 18488),
    "envoy": (18289, 18489),
}
if os.environ.get("PROXIES"):
    keep = {p.strip() for p in os.environ["PROXIES"].split(",")}
    PROXIES = {k: v for k, v in PROXIES.items() if k in keep}
SCENARIOS = ("http", "https", "h2", "handshake")
FWD = "X-Forwarded-For, X-Forwarded-Proto"


def sh(*args, **kw):
    return subprocess.run(args, check=True, text=True, capture_output=True, **kw).stdout


def configs(w: Path):
    """Write every proxy's configuration into the work directory w (mounted at /bench)."""
    (w / "body.txt").write_text("x" * SIZE)
    sh("openssl", "req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1",
       "-nodes", "-days", "2", "-subj", "/CN=bench.local", "-addext",
       "subjectAltName=DNS:bench.local", "-keyout", str(w / "key.pem"), "-out", str(w / "cert.pem"))
    (w / "combined.pem").write_text((w / "cert.pem").read_text() + (w / "key.pem").read_text())
    for f in w.iterdir():
        f.chmod(0o644)

    (w / "up.conf").write_text(f"""
worker_processes 2;
events {{ worker_connections 8192; }}
http {{
  access_log off;
  keepalive_requests 1000000;
  server {{ listen 127.0.0.1:{UP}; location / {{ root /srv; try_files /body.txt =404; }} }}
  # New connections the upstream accepted: shows whether a proxy reuses its connections.
  server {{ listen 127.0.0.1:{UP_STATUS}; location / {{ stub_status; }} }}
}}
""")

    # The same two nginx configurations as scripts/bench.sh.
    p = PROXIES.get("nginx (NPM-style)", (0,))[0]
    (w / "npm.conf").write_text(f"""
worker_processes 2;
events {{ worker_connections 8192; }}
http {{
  log_format proxy '[$time_local] $upstream_cache_status $upstream_status $status - $request_method $scheme $host "$request_uri" [Client $remote_addr] [Length $body_bytes_sent] [Gzip $gzip_ratio] [Sent-to $server] "$http_user_agent" "$http_referer"';
  keepalive_timeout 90s;
  proxy_http_version 1.1;
  gzip on;
  server {{
    listen 127.0.0.1:{p};
    set $forward_scheme http; set $server "127.0.0.1"; set $port {UP};
    access_log /tmp/proxy-host-1_access.log proxy;
    location / {{
      add_header X-Served-By $host;
      proxy_set_header Host $host;
      proxy_set_header X-Forwarded-Scheme $scheme;
      proxy_set_header X-Forwarded-Proto $scheme;
      proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header X-Real-IP $remote_addr;
      proxy_pass $forward_scheme://$server:$port$request_uri;
    }}
  }}
}}
""")
    p, t = PROXIES.get("nginx", (0, 0))
    (w / "nginx.conf").write_text(f"""
worker_processes 2;
events {{ worker_connections 8192; }}
http {{
  log_format json escape=json '{{"ts":"$time_iso8601","host":"$host","client":"$remote_addr","method":"$request_method","uri":"$request_uri","status":$status,"bytes_sent":$body_bytes_sent,"duration":$request_time,"upstream":"$upstream_addr","upstream_header":"$upstream_header_time","user_agent":"$http_user_agent"}}';
  access_log /tmp/access.log json buffer=64k flush=1s;
  keepalive_timeout 90s;
  keepalive_requests 1000000;
  ssl_session_cache shared:SSL:10m;
  upstream backend {{ server 127.0.0.1:{UP}; keepalive 128; }}
  server {{
    listen 127.0.0.1:{p};
    listen 127.0.0.1:{t} ssl;
    http2 on;
    ssl_certificate /bench/cert.pem;
    ssl_certificate_key /bench/key.pem;
    location / {{
      proxy_http_version 1.1;
      proxy_set_header Connection "";
      proxy_set_header Host $host;
      proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header X-Forwarded-Proto $scheme;
      proxy_set_header X-Real-IP $remote_addr;
      proxy_pass http://backend;
    }}
  }}
}}
""")

    p, t = PROXIES.get("conduit", (0, 0))
    (w / "conduit.yaml").write_text(f"""
server: {{ threads: 2, pid_file: /tmp/conduit.pid, upgrade_sock: /tmp/upgrade.sock, shutdown_grace: 1s, shutdown_timeout: 1s }}
listen: {{ http: ["127.0.0.1:{p}"], https: ["127.0.0.1:{t}"] }}
admin: {{ listen: "127.0.0.1:{CONDUIT_ADMIN}" }}
access_log: {{ path: /tmp/logs/access.log }}
tls:
  certs: [{{ name: bench, cert: /bench/cert.pem, key: /bench/key.pem }}]
upstreams:
  app: {{ backends: ["http://127.0.0.1:{UP}"] }}
sites:
  - {{ id: bench, hosts: [bench.local], upstream: app }}
""")

    # Caddy adds X-Forwarded-For/Proto/Host by default and keeps upstream connections alive;
    # its default of 32 idle connections per host is raised to match nginx's keepalive 128.
    # Log rolling is off like everywhere else: by default Caddy rolls the file every 100 MB
    # and gzips the old one inside its own process, which this run would count as proxy CPU.
    p, t = PROXIES.get("caddy", (0, 0))
    (w / "Caddyfile").write_text(f"""
{{
	admin off
	auto_https off
	log {{
		level ERROR
	}}
}}
(proxy) {{
	reverse_proxy 127.0.0.1:{UP} {{
		transport http {{
			keepalive_idle_conns_per_host 128
		}}
	}}
	log {{
		output file /tmp/access.log {{
			roll_disabled
		}}
		format json
	}}
}}
http://:{p} {{
	bind 127.0.0.1
	import proxy
}}
https://:{t} {{
	bind 127.0.0.1
	tls /bench/cert.pem /bench/key.pem
	import proxy
}}
""")

    # Event MPM with 2 processes of 64 threads, fixed for the whole run (no process churn);
    # mod_proxy keeps a pool of upstream connections per process by default.
    p, t = PROXIES.get("apache", (0, 0))
    (w / "httpd.conf").write_text(f"""
ServerRoot "/usr/local/apache2"
Listen 127.0.0.1:{p}
Listen 127.0.0.1:{t} https
LoadModule mpm_event_module modules/mod_mpm_event.so
LoadModule authz_core_module modules/mod_authz_core.so
LoadModule unixd_module modules/mod_unixd.so
LoadModule log_config_module modules/mod_log_config.so
LoadModule headers_module modules/mod_headers.so
LoadModule proxy_module modules/mod_proxy.so
LoadModule proxy_http_module modules/mod_proxy_http.so
LoadModule socache_shmcb_module modules/mod_socache_shmcb.so
LoadModule ssl_module modules/mod_ssl.so
LoadModule http2_module modules/mod_http2.so
User daemon
Group daemon
ServerName bench.local
ErrorLog /proc/self/fd/2
LogLevel warn
PidFile /tmp/httpd.pid
StartServers 2
ServerLimit 2
ThreadLimit 64
ThreadsPerChild 64
MaxRequestWorkers 128
MinSpareThreads 64
MaxSpareThreads 128
MaxConnectionsPerChild 0
KeepAlive On
MaxKeepAliveRequests 0
KeepAliveTimeout 90
LogFormat "{{\\"ts\\":\\"%{{%Y-%m-%dT%H:%M:%S}}t\\",\\"host\\":\\"%{{Host}}i\\",\\"client\\":\\"%a\\",\\"method\\":\\"%m\\",\\"uri\\":\\"%U%q\\",\\"status\\":%>s,\\"bytes_sent\\":%B,\\"duration_us\\":%D,\\"user_agent\\":\\"%{{User-agent}}i\\"}}" json
CustomLog /tmp/access.log json
BufferedLogs On
SSLSessionCache shmcb:/tmp/ssl_scache(512000)
ProxyPreserveHost On
ProxyPass / http://127.0.0.1:{UP}/
RequestHeader set X-Forwarded-Proto expr=%{{REQUEST_SCHEME}}
<VirtualHost 127.0.0.1:{t}>
  SSLEngine on
  SSLCertificateFile /bench/cert.pem
  SSLCertificateKeyFile /bench/key.pem
  Protocols h2 http/1.1
</VirtualHost>
""")

    # HAProxy's default (http-reuse safe) sends the first request of every client connection
    # over a new upstream connection: with a new TLS connection per request that is one
    # upstream connect per request. "always" shares idle connections across clients, as
    # nginx's keepalive pool and conduit do. HAProxy only logs to syslog or stdout, so
    # stdout goes to a file (see run_args()). It writes one writev() per line and may drop
    # lines under contention (its DroppedLogs counter), so its log count can come up short.
    p, t = PROXIES.get("haproxy", (0, 0))
    (w / "haproxy.cfg").write_text(f"""
global
  nbthread 2
  maxconn 20000
  log stdout format raw local0
defaults
  mode http
  log global
  option httplog
  timeout connect 5s
  timeout client 90s
  timeout server 90s
  timeout http-keep-alive 90s
frontend fe
  bind 127.0.0.1:{p}
  bind 127.0.0.1:{t} ssl crt /bench/combined.pem alpn h2,http/1.1
  option forwardfor
  http-request set-header X-Forwarded-Proto https if {{ ssl_fc }}
  http-request set-header X-Forwarded-Proto http if !{{ ssl_fc }}
  default_backend be
backend be
  http-reuse always
  server up 127.0.0.1:{UP}
""")

    # Traefik adds X-Forwarded-* by default and keeps up to 200 idle upstream connections.
    p, t = PROXIES.get("traefik", (0, 0))
    (w / "traefik.yml").write_text(f"""
global: {{ checkNewVersion: false, sendAnonymousUsage: false }}
log: {{ level: ERROR }}
accessLog: {{ filePath: /tmp/access.log, format: json, bufferingSize: 100 }}
entryPoints:
  web: {{ address: "127.0.0.1:{p}" }}
  websecure: {{ address: "127.0.0.1:{t}" }}
providers:
  file: {{ filename: /bench/traefik-dynamic.yml }}
""")
    (w / "traefik-dynamic.yml").write_text(f"""
http:
  routers:
    app: {{ rule: "PathPrefix(`/`)", entryPoints: [web], service: app }}
    app-tls: {{ rule: "PathPrefix(`/`)", entryPoints: [websecure], service: app, tls: {{}} }}
  services:
    app:
      loadBalancer:
        servers: [{{ url: "http://127.0.0.1:{UP}" }}]
tls:
  certificates: [{{ certFile: /bench/cert.pem, keyFile: /bench/key.pem }}]
  stores:
    default:
      defaultCertificate: {{ certFile: /bench/cert.pem, keyFile: /bench/key.pem }}
""")

    # Envoy: use_remote_address appends the client to X-Forwarded-For and sets
    # X-Forwarded-Proto; the cluster pools upstream connections per worker.
    p, t = PROXIES.get("envoy", (0, 0))
    def hcm(prefix):
        return {"name": "envoy.filters.network.http_connection_manager", "typed_config": {
            "@type": "type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager",
            "stat_prefix": prefix, "codec_type": "AUTO", "use_remote_address": True,
            "access_log": [{"name": "envoy.access_loggers.file", "typed_config": {
                "@type": "type.googleapis.com/envoy.extensions.access_loggers.file.v3.FileAccessLog",
                "path": "/tmp/access.log", "log_format": {"json_format": {
                    "ts": "%START_TIME%", "host": "%REQ(:AUTHORITY)%",
                    "client": "%DOWNSTREAM_REMOTE_ADDRESS_WITHOUT_PORT%", "method": "%REQ(:METHOD)%",
                    "uri": "%REQ(:PATH)%", "status": "%RESPONSE_CODE%", "bytes_sent": "%BYTES_SENT%",
                    "duration": "%DURATION%", "upstream": "%UPSTREAM_HOST%",
                    "user_agent": "%REQ(USER-AGENT)%"}}}}],
            "route_config": {"virtual_hosts": [{"name": "all", "domains": ["*"],
                                                "routes": [{"match": {"prefix": "/"}, "route": {"cluster": "up"}}]}]},
            "http_filters": [{"name": "envoy.filters.http.router", "typed_config": {
                "@type": "type.googleapis.com/envoy.extensions.filters.http.router.v3.Router"}}]}}
    def listener(name, port, tls):
        chain = {"filters": [hcm(name)]}
        if tls:
            chain["transport_socket"] = {"name": "envoy.transport_sockets.tls", "typed_config": {
                "@type": "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.DownstreamTlsContext",
                "common_tls_context": {
                    "tls_certificates": [{"certificate_chain": {"filename": "/bench/cert.pem"},
                                          "private_key": {"filename": "/bench/key.pem"}}],
                    "alpn_protocols": ["h2", "http/1.1"]}}}
        return {"name": name, "address": {"socket_address": {"address": "127.0.0.1", "port_value": port}},
                "filter_chains": [chain]}
    (w / "envoy.json").write_text(json.dumps({"static_resources": {
        "listeners": [listener("http", p, False), listener("https", t, True)],
        "clusters": [{"name": "up", "type": "STATIC", "connect_timeout": "1s",
                      "load_assignment": {"cluster_name": "up", "endpoints": [{"lb_endpoints": [
                          {"endpoint": {"address": {"socket_address": {"address": "127.0.0.1", "port_value": UP}}}}]}]}}]}},
        indent=1))


def run_args(name):
    """docker run arguments (after the image options) for each proxy."""
    return {
        "nginx (NPM-style)": (IMAGES["nginx"], ["-v", "{w}/npm.conf:/etc/nginx/nginx.conf:ro"], []),
        "nginx": (IMAGES["nginx"], ["-v", "{w}/nginx.conf:/etc/nginx/nginx.conf:ro"], []),
        "conduit": (IMAGES["conduit"], ["-v", f"{BIN}:/usr/local/bin/conduit:ro", "-e", "CONDUIT_LOG=warn"],
                    ["conduit", "run", "-c", "/bench/conduit.yaml"]),
        "caddy": (IMAGES["caddy"], ["-e", "GOMAXPROCS=2"],
                  ["caddy", "run", "--config", "/bench/Caddyfile", "--adapter", "caddyfile"]),
        "apache": (IMAGES["apache"], ["-v", "{w}/httpd.conf:/usr/local/apache2/conf/httpd.conf:ro"], []),
        "haproxy": (IMAGES["haproxy"], ["--entrypoint", "sh"],
                    ["-c", "exec haproxy -W -db -f /bench/haproxy.cfg >>/tmp/access.log 2>&1"]),
        "traefik": (IMAGES["traefik"], ["-e", "GOMAXPROCS=2"], ["--configFile=/bench/traefik.yml"]),
        "envoy": (IMAGES["envoy"], [],
                  ["-c", "/bench/envoy.json", "--concurrency", "2", "--log-level", "error"]),
    }[name]


def container_name(name):
    return "cmp-" + name.split()[0] + ("-npm" if "NPM" in name else "")


class Container:
    def __init__(self, name, cid):
        self.name, self.cid = name, cid
        self.cg = Path(f"/sys/fs/cgroup/system.slice/docker-{cid}.scope")

    def cpu_usec(self):
        for line in (self.cg / "cpu.stat").read_text().splitlines():
            if line.startswith("usage_usec"):
                return int(line.split()[1])

    def anon_bytes(self):
        for line in (self.cg / "memory.stat").read_text().splitlines():
            if line.startswith("anon "):
                return int(line.split()[1])


def start(name, image, opts, cmd, cpus, w):
    docker_rm(container_name(name))
    opts = [o.replace("{w}", str(w)) for o in opts]
    cid = sh("docker", "run", "-d", "--rm", "--name", container_name(name), "--network", "host",
             "--cpuset-cpus", cpus, "--log-driver", "none", "-v", f"{w}:/bench:ro", *opts,
             image, *cmd).strip()
    return Container(name, cid)


def docker_rm(*names):
    subprocess.run(["docker", "rm", "-f", *names], capture_output=True)


def wait_port(port):
    import socket
    for _ in range(150):
        try:
            socket.create_connection(("127.0.0.1", port), timeout=0.2).close()
            return
        except OSError:
            time.sleep(0.1)
    raise SystemExit(f"port {port} never opened")


def upstream_accepts():
    out = sh("curl", "-s", f"http://127.0.0.1:{UP_STATUS}/")
    return int(out.splitlines()[2].split()[0])


def smoke(name, http, https):
    """Each proxy must answer 200 with the whole body, and HTTP/2 on TLS."""
    out = sh("curl", "-s", "-o", "/dev/null", "-w", "%{http_code} %{size_download}",
             "-H", "Host: bench.local", f"http://127.0.0.1:{http}/")
    want = f"200 {SIZE}"
    if out != want:
        raise SystemExit(f"{name}: http answered {out!r}, want {want!r}")
    if https:
        out = sh("curl", "-sk", "--http2", "-o", "/dev/null", "-w", "%{http_code} %{size_download} %{http_version}",
                 "--resolve", f"bench.local:{https}:127.0.0.1", f"https://bench.local:{https}/")
        if out != f"{want} 2":
            raise SystemExit(f"{name}: https answered {out!r}, want {want + ' 2'!r}")


OHA = sh("mise", "which", "oha", cwd=ROOT).strip()


def oha(scenario, port, n, rate=0):
    args = ["taskset", "-c", LOAD_CPUS, OHA, "-n", str(n), "-c", str(C), "--no-tui",
            "--disable-compression", "--output-format", "json"]
    if scenario in ("http", "fixed"):
        args += ["--host", "bench.local", f"http://127.0.0.1:{port}/"]
        if rate:
            args += ["-q", str(rate), "--latency-correction"]
    else:
        if scenario == "h2":
            args.append("--http2")
        if scenario == "handshake":
            args.append("--disable-keepalive")
        args += ["--insecure", "--connect-to", f"bench.local:{port}:127.0.0.1:{port}",
                 f"https://bench.local:{port}/"]
    return json.loads(sh(*args))


SENT = {}
ACCESS_LOGS = {"nginx (NPM-style)": "/tmp/proxy-host-1_access.log", "conduit": "/tmp/logs/access.log"}


def measure(scenario, name, port, box, n, rate=0):
    oha(scenario, port, 2000, rate)  # warm up
    SENT[name] = SENT.get(name, 0) + 2000 + n
    peak, stop = [0], threading.Event()

    def sample():
        while not stop.is_set():
            peak[0] = max(peak[0], box.anon_bytes())
            stop.wait(0.05)
    t = None
    if box:
        cpu0 = box.cpu_usec()
        t = threading.Thread(target=sample)
        t.start()
    acc0 = upstream_accepts()
    d = oha(scenario, port, n, rate)
    acc = upstream_accepts() - acc0 - 1  # the status request itself is one accept
    row = {"scenario": scenario, "proxy": name}
    if box:
        stop.set()
        t.join()
        row["cpu_s_per_10k"] = (box.cpu_usec() - cpu0) / 1e6 / n * 10000
        row["mem_peak_mb"] = peak[0] / 2**20
    lat = d["latencyPercentiles"]
    row.update(rps=d["summary"]["requestsPerSec"], p50_ms=lat["p50"] * 1000, p99_ms=lat["p99"] * 1000,
               errors=sum(v for k, v in d["statusCodeDistribution"].items() if k != "200")
               + sum(d["errorDistribution"].values()),
               upstream_conns=acc if name != "upstream direct" else None)
    print(f"  {scenario:<9} {name:<18} {row['rps']:9.0f} req/s  p50 {row['p50_ms']:6.2f}  "
          f"p99 {row['p99_ms']:6.2f} ms  cpu {row.get('cpu_s_per_10k', 0):5.2f} s/10k  "
          f"up-conns {acc:6d}  err {row['errors']}", flush=True)
    return row


def main():
    if not BIN.exists():
        raise SystemExit(f"no conduit binary at {BIN}")
    w = Path(tempfile.mkdtemp(prefix="conduit-compare-"))
    w.chmod(0o755)
    names = [container_name(n) for n in PROXIES] + ["cmp-upstream"]
    try:
        configs(w)
        docker_rm(*names)
        start("upstream", IMAGES["nginx"], ["-v", "{w}/up.conf:/etc/nginx/nginx.conf:ro",
                                            "-v", "{w}/body.txt:/srv/body.txt:ro"], [], UP_CPUS, w)
        boxes = {}
        for name in PROXIES:
            image, opts, cmd = run_args(name)
            boxes[name] = start(name, image, opts, cmd, PROXY_CPUS, w)
        for name, (http, https) in PROXIES.items():
            wait_port(http)
            if https:
                wait_port(https)
        wait_port(UP)
        for name, (http, https) in PROXIES.items():
            smoke(name, http, https)
            SENT[name] = 2 if https else 1
        time.sleep(2)
        idle = {name: boxes[name].anon_bytes() / 2**20 for name in PROXIES}
        versions = {
            "conduit": sh(str(BIN), "--version").strip(), **{k: v for k, v in IMAGES.items() if k != "conduit"},
        }
        print(f"N={N} C={C} body={SIZE}B rounds={ROUNDS} proxy CPUs={PROXY_CPUS}")
        print("versions:", json.dumps(versions))

        rows = []
        order = list(PROXIES)
        for r in range(ROUNDS):
            print(f"round {r + 1}/{ROUNDS}")
            rot = order[r % len(order):] + order[:r % len(order)]
            rows.append(measure("http", "upstream direct", UP, None, N))
            for scenario in SCENARIOS:
                n = N // 5 if scenario == "handshake" else N
                for name in rot:
                    http, https = PROXIES[name]
                    port = http if scenario == "http" else https
                    if port:
                        rows.append(measure(scenario, name, port, boxes[name], n))

        def med(rs, k):
            vals = [x[k] for x in rs if x.get(k) is not None]
            return statistics.median(vals) if vals else None

        def summarize(rows):
            groups = {}
            for x in rows:
                groups.setdefault((x["scenario"], x["proxy"]), []).append(x)
            out = []
            for (s, name), rs in groups.items():
                row = {"scenario": s, "proxy": name, "rounds": len(rs),
                       "errors": sum(x["errors"] for x in rs)}
                for k in ("rps", "p50_ms", "p99_ms", "cpu_s_per_10k", "upstream_conns"):
                    row[k] = med(rs, k)
                row["mem_peak_mb"] = max((x.get("mem_peak_mb") or 0 for x in rs), default=0) or None
                out.append(row)
            return out

        summary = summarize(rows)
        peaks = [x["rps"] for x in summary if x["scenario"] == "http"
                 and x["proxy"] not in ("upstream direct", "nginx (NPM-style)")]
        rate = int(min(peaks) * 0.5)
        print(f"fixed: {rate} req/s (50% of the slowest HTTP peak) for {RATE_SECONDS}s, {ROUNDS} rounds")
        fixed = []
        for r in range(ROUNDS):
            rot = order[r % len(order):] + order[:r % len(order)]
            fixed.append(measure("fixed", "upstream direct", UP, None, rate * RATE_SECONDS, rate))
            for name in rot:
                fixed.append(measure("fixed", name, PROXIES[name][0], boxes[name], rate * RATE_SECONDS, rate))
        summary += summarize(fixed)

        # Every proxy must have logged every request. Envoy flushes its access log every 10 s.
        time.sleep(11)
        logged = {}
        for name in PROXIES:
            path = ACCESS_LOGS.get(name, "/tmp/access.log")
            logged[name] = int(sh("docker", "exec", container_name(name), "sh", "-c",
                                  f"wc -l < {path}").strip())
        print("access log lines / requests sent: " + ", ".join(
            f"{k} {logged[k]}/{SENT[k]}" for k in PROXIES))

        out = {"date": time.strftime("%Y-%m-%d"), "n": N, "c": C, "size": SIZE, "rounds": ROUNDS,
               "proxy_cpus": PROXY_CPUS, "fixed_rate": rate, "versions": versions,
               "idle_mem_mb": idle, "access_log": {k: [logged[k], SENT[k]] for k in PROXIES}, "results": summary, "raw": rows + fixed}
        dest = ROOT / "target/bench/compare.json"
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(json.dumps(out, indent=1))
        report(out)
        print(f"written to {dest.relative_to(ROOT)}")
    finally:
        docker_rm(*names)
        shutil.rmtree(w, ignore_errors=True)


def report(out):
    res = out["results"]
    get = lambda s, p: next((x for x in res if x["scenario"] == s and x["proxy"] == p), None)
    titles = {"http": "HTTP/1.1 keep-alive", "https": "HTTPS keep-alive (HTTP/1.1)", "h2": "HTTP/2",
              "handshake": "new TLS connection per request"}
    for s in SCENARIOS:
        ref = get(s, "nginx")
        print(f"\n{titles[s]} (median of {out['rounds']})")
        print(f"  {'proxy':<18} {'req/s':>9} {'vs nginx':>9} {'p50 ms':>7} {'p99 ms':>7} "
              f"{'cpu s/10k':>9} {'mem MB':>7} {'up-conns':>8} {'errors':>6}")
        for x in sorted((x for x in res if x["scenario"] == s), key=lambda x: -x["rps"]):
            vs = f"{x['rps'] / ref['rps']:.2f}x" if ref else "-"
            cpu = f"{x['cpu_s_per_10k']:.2f}" if x.get("cpu_s_per_10k") is not None else "-"
            mem = f"{x['mem_peak_mb']:.0f}" if x.get("mem_peak_mb") else "-"
            up = f"{x['upstream_conns']:.0f}" if x.get("upstream_conns") is not None else "-"
            print(f"  {x['proxy']:<18} {x['rps']:9.0f} {vs:>9} {x['p50_ms']:7.2f} {x['p99_ms']:7.2f} "
                  f"{cpu:>9} {mem:>7} {up:>8} {x['errors']:6d}")
    direct = get("fixed", "upstream direct")
    print(f"\nfixed {out['fixed_rate']} req/s: added to the upstream's own latency "
          f"(direct p50 {direct['p50_ms']:.2f} ms, p99 {direct['p99_ms']:.2f} ms)")
    for x in sorted((x for x in res if x["scenario"] == "fixed" and x["proxy"] != "upstream direct"),
                    key=lambda x: x["p99_ms"]):
        print(f"  {x['proxy']:<18} p50 {x['p50_ms'] - direct['p50_ms']:+6.2f} ms  "
              f"p99 {x['p99_ms'] - direct['p99_ms']:+6.2f} ms  cpu {x['cpu_s_per_10k']:.2f} s/10k  "
              f"errors {x['errors']}")
    print("\nidle memory (anon, MB): " + ", ".join(f"{k} {v:.1f}" for k, v in out["idle_mem_mb"].items()))


if __name__ == "__main__":
    main()
