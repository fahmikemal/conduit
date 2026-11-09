#!/usr/bin/env bash
# A/B of conduit builds or settings: each variant runs the same scenarios against the same
# upstream, in alternating order over several rounds, and the medians are compared. One
# benchmark run on a desktop machine swings more than most optimizations are worth; this
# is the tool for deciding whether a change helps.
#
#   scripts/ab.sh NAME=BINARY[,server_key=value...] NAME=BINARY[,...] ...
#
#   scripts/ab.sh base=target/ab/conduit-base new=target/release/conduit
#   scripts/ab.sh steal=target/release/conduit nosteal=target/release/conduit,work_stealing=false
#
# key=value pairs are added to the `server:` section of that variant's config.
# Environment: ROUNDS (5), N (requests per scenario, 40000; handshake gets N/5), C (64),
#   SIZE (body bytes, 2048), SCENARIOS ("http https h2 handshake"; also "rate": plain HTTP
#   held at RATE req/s for N/RATE seconds, latency counted from each request's scheduled
#   start, so queueing shows; and "gzip": plain HTTP with Accept-Encoding: gzip, so conduit
#   compresses each response), RATE (30000), PROXY_CPUS (0,2: two
#   physical cores, like the production host's 2 vCPUs), UP_CPUS (4,5), LOAD_CPUS (6-11).
# Needs docker (nginx as the upstream), openssl, and oha from mise.toml.
set -euo pipefail
cd "$(dirname "$0")/.."

[ $# -ge 2 ] || { sed -n '2,20p' "$0"; exit 2; }
ROUNDS=${ROUNDS:-5}
N=${N:-40000}
C=${C:-64}
SIZE=${SIZE:-2048}
SCENARIOS=${SCENARIOS:-http https h2 handshake}
PROXY_CPUS=${PROXY_CPUS:-0,2}
UP_CPUS=${UP_CPUS:-4,5}
LOAD_CPUS=${LOAD_CPUS:-6-11}
RATE=${RATE:-30000}
NGINX_IMAGE=${NGINX_IMAGE:-nginx:1.28-alpine}
UP=18181 CON=18184 CON_TLS=18544 ADMIN=18190

work=$(mktemp -d)
conduit_pid=
cleanup() {
  [ -n "$conduit_pid" ] && kill "$conduit_pid" 2>/dev/null && wait "$conduit_pid" 2>/dev/null || true
  docker rm -f conduit-ab-up >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

names=() bins=() extras=()
for spec in "$@"; do
  name=${spec%%=*} rest=${spec#*=}
  bin=${rest%%,*} extra=
  [ "$rest" != "$bin" ] && extra=${rest#*,}
  [ -x "$bin" ] || { echo "$name: $bin is not an executable" >&2; exit 2; }
  names+=("$name") bins+=("$(realpath "$bin")") extras+=("$extra")
done

head -c "$SIZE" /dev/zero | tr '\0' 'x' > "$work/body.txt"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
  -subj /CN=bench.local -addext subjectAltName=DNS:bench.local \
  -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
cat > "$work/up.conf" <<EOF
worker_processes 2;
events { worker_connections 8192; }
http {
  access_log off;
  keepalive_requests 1000000;
  server { listen 127.0.0.1:$UP; location / { root /srv; try_files /body.txt =404; } }
}
EOF
docker run -d --rm --name conduit-ab-up --network host --cpuset-cpus "$UP_CPUS" \
  -v "$work/up.conf:/etc/nginx/nginx.conf:ro" -v "$work/body.txt:/srv/body.txt:ro" \
  "$NGINX_IMAGE" >/dev/null

wait_port() {
  for _ in $(seq 150); do (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return; sleep 0.1; done
  echo "port $1 never opened" >&2; exit 1
}
wait_port "$UP"

# config <variant index>: the bench site of scripts/bench.sh, access log on.
config() {
  local i=$1 extra=${extras[$1]} server="threads: 2, pid_file: $work/conduit.pid, upgrade_sock: $work/upgrade.sock, shutdown_grace: 1s, shutdown_timeout: 1s"
  local IFS=,
  for kv in $extra; do server+=", ${kv%%=*}: ${kv#*=}"; done
  cat > "$work/conduit-$i.yaml" <<EOF
server: { $server }
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
}

load() {
  local scenario=$1 n=$2
  local common=(-n "$n" -c "$C" --no-tui --disable-compression --output-format json)
  case $scenario in
    http) taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" --host bench.local "http://127.0.0.1:$CON/" ;;
    gzip) taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" -H "Accept-Encoding: gzip" \
        --host bench.local "http://127.0.0.1:$CON/" ;;
    rate) taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" -q "$RATE" --latency-correction \
        --host bench.local "http://127.0.0.1:$CON/" ;;
    https|h2|handshake)
      local extra=()
      [ "$scenario" = h2 ] && extra=(--http2)
      [ "$scenario" = handshake ] && extra=(--disable-keepalive)
      taskset -c "$LOAD_CPUS" mise exec -- oha "${common[@]}" "${extra[@]}" --insecure \
        --connect-to "bench.local:$CON_TLS:127.0.0.1:$CON_TLS" "https://bench.local:$CON_TLS/" ;;
  esac
}

cpu_ticks() { awk '{print $14 + $15}' "/proc/$1/stat"; }

: > "$work/results.jsonl"
log_bad=0
echo "$ROUNDS rounds, N=$N C=$C body=${SIZE}B, proxy CPUs $PROXY_CPUS, scenarios: $SCENARIOS"
for round in $(seq "$ROUNDS"); do
  count=${#names[@]}
  for k in $(seq 0 $((count - 1))); do
    i=$(( (k + round - 1) % count ))   # rotate the order every round
    config "$i"
    rm -rf "$work/logs"
    CONDUIT_LOG=warn taskset -c "$PROXY_CPUS" "${bins[$i]}" run -c "$work/conduit-$i.yaml" \
      2>"$work/conduit.err" &
    conduit_pid=$!
    wait_port "$CON"; wait_port "$CON_TLS"
    sent=0
    for scenario in $SCENARIOS; do
      n=$N
      [ "$scenario" = handshake ] && n=$((N / 5))
      [ "$scenario" = rate ] && n=$((N * 10 / 4))   # 100k at the default N: ~3s at 30k req/s
      sent=$((sent + 2000 + n))
      load "$scenario" 2000 >/dev/null
      before=$(cpu_ticks "$conduit_pid")
      load "$scenario" "$n" > "$work/out.json"
      ticks=$(( $(cpu_ticks "$conduit_pid") - before ))
      python3 - "$work/out.json" "$work/results.jsonl" "${names[$i]}" "$scenario" "$round" \
        "$ticks" "$(getconf CLK_TCK)" "$n" <<'PY'
import json, sys
out, results, name, scenario, rnd, ticks, hz, n = sys.argv[1:]
d = json.load(open(out))
bad = sum(v for k, v in d["statusCodeDistribution"].items() if k != "200") \
    + sum(d["errorDistribution"].values())
row = {"variant": name, "scenario": scenario, "round": int(rnd),
       "rps": d["summary"]["requestsPerSec"], "p99_ms": d["latencyPercentiles"]["p99"] * 1000,
       "cpu_ms_per_1k": int(ticks) / int(hz) / int(n) * 1e6, "errors": bad}
open(results, "a").write(json.dumps(row) + "\n")
print(f"  round {rnd} {name:<12} {scenario:<10} {row['rps']:9.0f} req/s  p99 {row['p99_ms']:6.2f} ms"
      f"  cpu {row['cpu_ms_per_1k']:6.1f} ms/1k  errors {bad}")
PY
    done
    # A variant that skipped log lines would look faster than it is: every request must
    # have its line, none dropped.
    dropped=$(curl -s "127.0.0.1:$ADMIN/metrics" | awk '/^conduit_access_log_dropped_total/ {print $2}')
    kill "$conduit_pid"; wait "$conduit_pid" 2>/dev/null || true; conduit_pid=
    logged=$(cat "$work"/logs/access.log* 2>/dev/null | wc -l)
    if [ "$logged" != "$sent" ] || [ "${dropped:-0}" != 0 ]; then
      echo "  round $round ${names[$i]}: access log has $logged lines for $sent requests (${dropped:-?} dropped)"
      log_bad=1
    fi
  done
done

status=0
python3 - "$work/results.jsonl" "${names[@]}" <<'PY' || status=$?
import json, statistics as st, sys
rows = [json.loads(l) for l in open(sys.argv[1])]
names = sys.argv[2:]
scenarios = list(dict.fromkeys(r["scenario"] for r in rows))
med = lambda v, s, k: st.median(r[k] for r in rows if r["variant"] == v and r["scenario"] == s)
print(f"\nmedians over {max(r['round'] for r in rows)} rounds; change against {names[0]}")
print(f"{'scenario':<10} {'variant':<12} {'req/s':>9} {'':>7} {'p99 ms':>7} {'':>7} {'cpu ms/1k':>9} {'':>7}")
for s in scenarios:
    base = {k: med(names[0], s, k) for k in ("rps", "p99_ms", "cpu_ms_per_1k")}
    for v in names:
        cells = []
        for k, w in (("rps", 9), ("p99_ms", 7), ("cpu_ms_per_1k", 9)):
            m = med(v, s, k)
            d = "" if v == names[0] else f"{(m / base[k] - 1) * 100:+6.1f}%"
            cells.append(f"{m:>{w}.{0 if k == 'rps' else 2}f} {d:>7}")
        print(f"{s:<10} {v:<12} " + " ".join(cells))
errors = sum(r["errors"] for r in rows)
print(f"errors: {errors}")
sys.exit(1 if errors else 0)
PY
[ "$log_bad" = 0 ] || { echo "access log lines missing or dropped (see above)"; status=1; }
exit $status
