#!/usr/bin/env bash
# conduit against nginx, proxying the same upstream on this machine, with a regression gate.
# Proxies get 2 CPUs (the production host has 2 vCPUs); upstream and load generator get their own.
#
#   scripts/bench.sh [conduit-binary]
#
# Environment: N (requests per scenario), C (connections), SIZE (body bytes),
#   BASELINE (default target/bench/baseline.json): conduit's numbers are compared with it.
#     req/s is compared as a ratio to tuned nginx in the same run (machine heat and background
#     load move both proxies; the absolute numbers drift ~5% over consecutive runs) and fails
#     below TOLERANCE percent (default 10); p99 fails above P99_TOLERANCE percent (default 25,
#     since p99 at saturation varies up to ~15% between runs on an idle machine);
#   UPDATE_BASELINE=1: store this run as the new baseline;
#   RATE_SECONDS (default 10), RATE_ROUNDS (default 3) and P99_OVERHEAD_MS (default 1): see
#     http50.
#
# Scenarios: plain HTTP keep-alive, HTTPS keep-alive (HTTP/1.1), HTTP/2, and a new TLS
# handshake for every request, each as fast as the proxy goes. http50 and http70 then hold
# a fixed rate of 50% and 70% of conduit's HTTP peak (the baseline's rates when comparing
# with one) against the upstream directly and through each proxy; latency counts from each request's scheduled start, so queueing
# shows. Each fixed rate runs RATE_ROUNDS times, alternating upstream, nginx and conduit, and
# the gate uses the median: one 10 s window is easily hit by a hiccup of the desktop (both
# proxies jump together when it happens). The script fails when conduit adds more than
# P99_OVERHEAD_MS to the direct p99 at
# 50%; http70, where queueing dominates (tuned nginx adds ~12 ms at 70% of its own peak),
# is held to the baseline like the other scenarios.
# "nginx (NPM-style)" proxies through variables without upstream keep-alive, as Nginx Proxy
# Manager does; "nginx" is tuned with keep-alive.
# Needs docker and openssl; oha comes from mise.toml.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=$(realpath "${1:-target/release/conduit}")
N=${N:-50000}
C=${C:-64}
SIZE=${SIZE:-2048}
PROXY_CPUS=${PROXY_CPUS:-0,1}
UP_CPUS=${UP_CPUS:-2,3}
LOAD_CPUS=${LOAD_CPUS:-4-7}
BASELINE=${BASELINE:-target/bench/baseline.json}
TOLERANCE=${TOLERANCE:-10}
P99_TOLERANCE=${P99_TOLERANCE:-25}
RATE_SECONDS=${RATE_SECONDS:-10}
RATE_ROUNDS=${RATE_ROUNDS:-3}
P99_OVERHEAD_MS=${P99_OVERHEAD_MS:-1}
RATE=0  # requests per second for http50/http70, set from the http results
UP=18081 NPM=18082 NGX=18083 NGX_TLS=18443 CON=18084 CON_TLS=18444 ADMIN=18090
NGINX_IMAGE=${NGINX_IMAGE:-nginx:1.28-alpine}

work=$(mktemp -d)
conduit_pid=
cleanup() {
  # Wait for it: a conduit still draining would sit on the proxy CPUs during the next run.
  [ -n "$conduit_pid" ] && kill "$conduit_pid" 2>/dev/null && wait "$conduit_pid" 2>/dev/null || true
  docker rm -f conduit-bench-up conduit-bench-npm conduit-bench-nginx >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

head -c "$SIZE" /dev/zero | tr '\0' 'x' > "$work/body.txt"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
  -subj /CN=bench.local -addext subjectAltName=DNS:bench.local \
  -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
chmod 644 "$work/key.pem"

cat > "$work/up.conf" <<EOF
worker_processes 2;
events { worker_connections 8192; }
http {
  access_log off;
  keepalive_requests 1000000;
  server { listen 127.0.0.1:$UP; location / { root /srv; try_files /body.txt =404; } }
}
EOF

# NPM's request path: HTTP/1.1 to the upstream through variables (no upstream keep-alive),
# its headers, access log on.
cat > "$work/npm.conf" <<EOF
worker_processes 2;
events { worker_connections 8192; }
http {
  log_format proxy '[\$time_local] \$upstream_cache_status \$upstream_status \$status - \$request_method \$scheme \$host "\$request_uri" [Client \$remote_addr] [Length \$body_bytes_sent] [Gzip \$gzip_ratio] [Sent-to \$server] "\$http_user_agent" "\$http_referer"';
  keepalive_timeout 90s;
  proxy_http_version 1.1;
  gzip on;
  server {
    listen 127.0.0.1:$NPM;
    set \$forward_scheme http; set \$server "127.0.0.1"; set \$port $UP;
    access_log /tmp/proxy-host-1_access.log proxy;
    location / {
      add_header X-Served-By \$host;
      proxy_set_header Host \$host;
      proxy_set_header X-Forwarded-Scheme \$scheme;
      proxy_set_header X-Forwarded-Proto \$scheme;
      proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
      proxy_set_header X-Real-IP \$remote_addr;
      proxy_pass \$forward_scheme://\$server:\$port\$request_uri;
    }
  }
}
EOF

# nginx as one would tune it: upstream keep-alive, TLS session cache, JSON access log.
cat > "$work/nginx.conf" <<EOF
worker_processes 2;
events { worker_connections 8192; }
http {
  log_format json escape=json '{"ts":"\$time_iso8601","host":"\$host","client":"\$remote_addr","method":"\$request_method","uri":"\$request_uri","status":\$status,"bytes_sent":\$body_bytes_sent,"duration":\$request_time,"upstream":"\$upstream_addr","upstream_header":"\$upstream_header_time","user_agent":"\$http_user_agent"}';
  access_log /tmp/access.log json buffer=64k flush=1s;
  keepalive_timeout 90s;
  keepalive_requests 1000000;
  ssl_session_cache shared:SSL:10m;
  upstream backend { server 127.0.0.1:$UP; keepalive 128; }
  server {
    listen 127.0.0.1:$NGX;
    listen 127.0.0.1:$NGX_TLS ssl;
    http2 on;
    ssl_certificate /etc/bench/cert.pem;
    ssl_certificate_key /etc/bench/key.pem;
    location / {
      proxy_http_version 1.1;
      proxy_set_header Connection "";
      proxy_set_header Host \$host;
      proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
      proxy_set_header X-Forwarded-Proto \$scheme;
      proxy_set_header X-Real-IP \$remote_addr;
      proxy_pass http://backend;
    }
  }
}
EOF

cat > "$work/conduit.yaml" <<EOF
server: { threads: 2, pid_file: $work/conduit.pid, upgrade_sock: $work/upgrade.sock, shutdown_grace: 1s, shutdown_timeout: 1s }
listen: { http: ["127.0.0.1:$CON"], https: ["127.0.0.1:$CON_TLS"] }
admin: { listen: "127.0.0.1:$ADMIN" }
access_log: { path: $work/logs/access.log }
tls:
  certs: [{ name: bench, cert: $work/cert.pem, key: $work/key.pem }]
upstreams:
  app: { backends: ["http://127.0.0.1:$UP"] }
sites:
  - { id: bench, hosts: [bench.local], upstream: app }
EOF

wait_port() {
  for _ in $(seq 100); do (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return; sleep 0.1; done
  echo "port $1 never opened" >&2; exit 1
}

docker run -d --rm --name conduit-bench-up --network host --cpuset-cpus "$UP_CPUS" \
  -v "$work/up.conf:/etc/nginx/nginx.conf:ro" -v "$work/body.txt:/srv/body.txt:ro" \
  "$NGINX_IMAGE" >/dev/null
docker run -d --rm --name conduit-bench-npm --network host --cpuset-cpus "$PROXY_CPUS" \
  -v "$work/npm.conf:/etc/nginx/nginx.conf:ro" "$NGINX_IMAGE" >/dev/null
docker run -d --rm --name conduit-bench-nginx --network host --cpuset-cpus "$PROXY_CPUS" \
  -v "$work/nginx.conf:/etc/nginx/nginx.conf:ro" -v "$work/cert.pem:/etc/bench/cert.pem:ro" \
  -v "$work/key.pem:/etc/bench/key.pem:ro" "$NGINX_IMAGE" >/dev/null
CONDUIT_LOG=warn taskset -c "$PROXY_CPUS" "$BIN" run -c "$work/conduit.yaml" 2>"$work/conduit.err" &
conduit_pid=$!
for p in $UP $NPM $NGX $NGX_TLS $CON $CON_TLS; do wait_port "$p"; done

pids_of() { docker top "$1" -o pid | tail -n +2; }
cpu_ticks() { local t=0; for p in "$@"; do t=$((t + $(awk '{print $14 + $15}' "/proc/$p/stat"))); done; echo $t; }
peak_rss_kb() { local t=0; for p in "$@"; do t=$((t + $(awk '/VmHWM/ {print $2}' "/proc/$p/status"))); done; echo $t; }

# load <scenario> <port> <requests>
load() {
  local scenario=$1 port=$2 n=$3
  local common=(-n "$n" -c "$C" --no-tui --disable-compression --output-format json)
  case $scenario in
    http) taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" --host bench.local "http://127.0.0.1:$port/" ;;
    http50|http70) taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" -q "$RATE" --latency-correction \
        --host bench.local "http://127.0.0.1:$port/" ;;
    https|h2|handshake)
      local extra=()
      [ "$scenario" = h2 ] && extra=(--http2)
      [ "$scenario" = handshake ] && extra=(--disable-keepalive)
      taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" "${extra[@]}" --insecure \
        --connect-to "bench.local:$port:127.0.0.1:$port" "https://bench.local:$port/" ;;
  esac
}

: > "$work/results.jsonl"
# run <scenario> <name> <port> pids...
run() {
  local scenario=$1 name=$2 port=$3; shift 3
  local n=$N
  [ "$scenario" = handshake ] && n=$((N / 5))
  case $scenario in http50|http70) n=$((RATE * RATE_SECONDS)) ;; esac
  load "$scenario" "$port" 2000 >/dev/null   # warm up
  local before; before=$(cpu_ticks "$@")
  load "$scenario" "$port" "$n" > "$work/out.json"
  local ticks=$(( $(cpu_ticks "$@") - before )) mem=0
  [ $# -gt 0 ] && mem=$(( $(peak_rss_kb "$@") / 1024 ))
  python3 - "$work/out.json" "$work/results.jsonl" "$scenario" "$name" "$mem" "$ticks" \
    "$(getconf CLK_TCK)" "$n" "$#" <<'PY'
import json, sys
out, results, scenario, name, mem, ticks, hz, n, procs = sys.argv[1:]
d = json.load(open(out))
bad = sum(v for k, v in d["statusCodeDistribution"].items() if k != "200") \
    + sum(d["errorDistribution"].values())
lat = d["latencyPercentiles"]
row = {
    "scenario": scenario, "proxy": name, "rps": d["summary"]["requestsPerSec"],
    "p50_ms": lat["p50"] * 1000, "p99_ms": lat["p99"] * 1000, "errors": bad,
    "peak_mb": int(mem) or None,
    "cpu_s_per_10k": int(ticks) / int(hz) / int(n) * 10000 if int(procs) else None,
}
with open(results, "a") as f:
    f.write(json.dumps(row) + "\n")
fmt = lambda v, spec: "-" if v is None else format(v, spec)
print(f"{scenario:<10} {name:<18} {row['rps']:9.0f} {row['p50_ms']:8.2f} {row['p99_ms']:8.2f} "
      f"{bad:7d} {fmt(row['peak_mb'], 'd'):>8} {fmt(row['cpu_s_per_10k'], '.2f'):>10}")
PY
}

echo "N=$N C=$C body=${SIZE}B proxy CPUs=$PROXY_CPUS  ($("$BIN" --version), $NGINX_IMAGE)"
printf "%-10s %-18s %9s %8s %8s %7s %8s %10s\n" scenario proxy req/s "p50 ms" "p99 ms" errors "peak MB" "cpu s/10k"
NPM_PIDS=$(pids_of conduit-bench-npm)
NGX_PIDS=$(pids_of conduit-bench-nginx)
run http "upstream direct" $UP
# shellcheck disable=SC2086
run http "nginx (NPM-style)" $NPM $NPM_PIDS
for scenario in http https h2 handshake; do
  port=$NGX; cport=$CON
  [ "$scenario" != http ] && port=$NGX_TLS && cport=$CON_TLS
  # shellcheck disable=SC2086
  run "$scenario" nginx $port $NGX_PIDS
  run "$scenario" conduit $cport "$conduit_pid"
done

PEAK=$(python3 -c '
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1])]
print(int(next(r["rps"] for r in rows if r["proxy"] == "conduit" and r["scenario"] == "http")))
' "$work/results.jsonl")
# The fixed rates come from the baseline when there is one, so p99 is compared at the same
# offered load: a faster conduit raises its own peak, and 70% of a higher peak is a heavier
# test (near 55k req/s this machine itself saturates, and nginx's p99 jumps as well).
# Without a baseline, or when recording one, they are a share of this run's peak.
baseline_rate() {
  python3 -c '
import json, sys
print(int(round(json.load(open(sys.argv[1]))[sys.argv[2]]["rps"])))
' "$BASELINE" "$1" 2>/dev/null
}
for pct in 50 70; do
  RATE=$((PEAK * pct / 100)) from="$pct% of conduit's HTTP peak"
  if [ "${UPDATE_BASELINE:-0}" != 1 ] && b=$(baseline_rate "http$pct") && [ -n "$b" ]; then
    RATE=$b from="the baseline's rate; $pct% of this run's peak is $((PEAK * pct / 100))"
  fi
  echo "http$pct: fixed $RATE req/s ($from) for ${RATE_SECONDS}s, $RATE_ROUNDS rounds"
  for _ in $(seq "$RATE_ROUNDS"); do
    run "http$pct" "upstream direct" $UP
    # shellcheck disable=SC2086
    run "http$pct" nginx $NGX $NGX_PIDS
    run "http$pct" conduit $CON "$conduit_pid"
  done
done

sleep 2  # the writer flushes once a second
lines=$(wc -l < "$work/logs/access.log")
dropped=$(curl -s "127.0.0.1:$ADMIN/metrics" | awk '/^conduit_access_log_dropped_total/ {print $2}')
echo "conduit access log: $lines lines, ${dropped:-?} dropped; RSS now $(awk '/VmRSS/ {print $2/1024 " MB"}' "/proc/$conduit_pid/status")"

mkdir -p "$(dirname "$BASELINE")"
cp "$work/results.jsonl" "$(dirname "$BASELINE")/latest.jsonl"
python3 - "$work/results.jsonl" "$BASELINE" "$TOLERANCE" "${UPDATE_BASELINE:-0}" "$P99_OVERHEAD_MS" \
  "$P99_TOLERANCE" <<'PY'
import json, os, sys
from statistics import median
results, baseline, tol, update = sys.argv[1], sys.argv[2], float(sys.argv[3]) / 100, sys.argv[4] == "1"
target = float(sys.argv[5])
p99_tol = float(sys.argv[6]) / 100
FIXED_RATE = ("http50", "http70")
# Repeated rounds of a scenario (the fixed rates) count as their median; errors add up.
groups = {}
for r in map(json.loads, open(results)):
    groups.setdefault((r["scenario"], r["proxy"]), []).append(r)
rows = []
for (s, name), rs in groups.items():
    row = dict(rs[0], errors=sum(r["errors"] for r in rs), rounds=len(rs))
    for k in ("rps", "p50_ms", "p99_ms", "cpu_s_per_10k"):
        vals = [r[k] for r in rs if r[k] is not None]
        row[k] = median(vals) if vals else None
    row["peak_mb"] = max((r["peak_mb"] or 0 for r in rs), default=0) or None
    rows.append(row)
    if len(rs) > 1:
        print(f"{s:<10} {name:<18} median of {len(rs)}: p50 {row['p50_ms']:.2f} ms, "
              f"p99 {row['p99_ms']:.2f} ms (rounds: "
              + ", ".join(f"{r['p99_ms']:.2f}" for r in rs) + ")")
mine = {r["scenario"]: r for r in rows if r["proxy"] == "conduit"}
nginx = {r["scenario"]: r for r in rows if r["proxy"] == "nginx"}
failed = []
for s, r in mine.items():
    if r["errors"]:
        failed.append(f"{s}: {r['errors']} failed requests")
    n = nginx.get(s)
    if n:
        print(f"{s:<10} conduit/nginx: req/s {r['rps'] / n['rps']:.2f}x, "
              f"p99 {r['p99_ms'] / n['p99_ms']:.2f}x, cpu {r['cpu_s_per_10k'] / n['cpu_s_per_10k']:.2f}x")
for s in ("http50", "http70"):
    direct = next((r for r in rows if r["scenario"] == s and r["proxy"] == "upstream direct"), None)
    if not direct:
        continue
    for name, r in (("nginx", nginx.get(s)), ("conduit", mine.get(s))):
        if r:
            print(f"{s:<10} {name} adds p50 {r['p50_ms'] - direct['p50_ms']:+.2f} ms, "
                  f"p99 {r['p99_ms'] - direct['p99_ms']:+.2f} ms over the upstream alone")
    added = mine[s]["p99_ms"] - direct["p99_ms"] if s in mine else None
    if s == "http50" and added is not None and added > target:
        failed.append(f"http50: conduit adds {added:.2f} ms to p99 at 50% load (target {target} ms)")
for s, r in mine.items():
    n = nginx.get(s)
    if n and s not in FIXED_RATE:
        r["rps_vs_nginx"] = r["rps"] / n["rps"]
if update:
    json.dump(mine, open(baseline, "w"), indent=1)
    print(f"baseline saved to {baseline}")
elif os.path.exists(baseline):
    base = json.load(open(baseline))
    for s, b in base.items():
        r = mine.get(s)
        if r is None:
            continue
        # At a fixed rate req/s is the offered load, not a result; only latency counts.
        if s in FIXED_RATE:
            pass
        elif "rps_vs_nginx" in b and "rps_vs_nginx" in r:
            if r["rps_vs_nginx"] < b["rps_vs_nginx"] * (1 - tol):
                failed.append(f"{s}: req/s {r['rps_vs_nginx']:.3f}x nginx < baseline "
                              f"{b['rps_vs_nginx']:.3f}x")
        elif r["rps"] < b["rps"] * (1 - tol):
            failed.append(f"{s}: req/s {r['rps']:.0f} < baseline {b['rps']:.0f}")
        if r["p99_ms"] > b["p99_ms"] * (1 + p99_tol) and r["p99_ms"] - b["p99_ms"] > 0.2:
            failed.append(f"{s}: p99 {r['p99_ms']:.2f} ms > baseline {b['p99_ms']:.2f} ms")
    print(f"compared with {baseline} (req/s vs nginx -{tol:.0%}, p99 +{p99_tol:.0%})")
else:
    print(f"no baseline at {baseline}; run with UPDATE_BASELINE=1 to create one")
if failed:
    print("REGRESSION:\n  " + "\n  ".join(failed))
    sys.exit(1)
PY
