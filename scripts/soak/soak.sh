#!/usr/bin/env bash
# Soak test, run on the test host by deploy.sh. conduit serves mixed traffic for SOAK_HOURS
# while backends flap, the config reloads, the certificate and the access log rotate, and
# the binary upgrades itself in place. Everything listens on 127.0.0.1.
#
# Load (≈650 req/s plus WebSocket), each oha stream in 5-minute windows:
#   h1     GET  /small  HTTP/1.1 keep-alive   250/s
#   big    GET  /big    256 KiB, gzip          20/s
#   post   POST /post   4 KiB body             50/s
#   tls    GET  /small  HTTPS keep-alive      150/s
#   h2     GET  /small  HTTP/2, 8 streams     150/s
#   churn  GET  /small  HTTPS, new TLS connection per request 30/s
#   ws.py  40 long-lived WebSockets (half over TLS) echoing every second, plus a short-lived
#          one every 0.5 s.
# Events: reload every 5 min, backend :9002 refused for 30 s every 10 min, access log rotated
# every 30 min, certificate rotated every hour, graceful upgrades at SOAK_UPGRADE_AT hours
# (default 2 and 6, so the last process runs 18 hours). Flaps fall in the middle of every
# other oha window and upgrades in the middle of one in between, so report.py can tell which
# of the two caused a failure.
# Samples every 30 s: RSS, fds, threads, CPU, connections, conduit metrics. Access log lines
# with an error or a 4xx/5xx status are kept in access-errors.log. Results in $OUT;
# report.py summarizes them.
set -uo pipefail
cd "$(dirname "$0")"
HERE=$PWD
HOURS=${SOAK_HOURS:-24}
# Divides every event interval and the oha window, for a quick smoke run of the harness.
SCALE=${SOAK_SCALE:-1}
UPGRADE_AT=${SOAK_UPGRADE_AT:-2 6}
OUT=${OUT:-$HERE/out}
RUN=$HERE/run
BIN=$HERE/conduit
OHA=$HERE/oha
CFG=$RUN/conduit.yaml
every() { echo $(($1 / SCALE > 5 ? $1 / SCALE : 5)); }
WINDOW=$(every 300)
START=$(date +%s)
END=$((START + ${SOAK_SECONDS:-$((HOURS * 3600))}))
mkdir -p "$OUT/oha" "$RUN/www" "$RUN/nginx"
event() { echo "$(date +%s) $*" >>"$OUT/events.log"; }

cert() {
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 \
        -subj /CN=soak.test -addext "subjectAltName=DNS:soak.test,DNS:ws.soak.test" \
        -keyout "$RUN/new.key" -out "$RUN/new.crt" 2>/dev/null
    # certbot order: key first, then the certificate.
    mv "$RUN/new.key" "$RUN/soak.key"
    mv "$RUN/new.crt" "$RUN/soak.crt"
}
cert
head -c 196608 /dev/urandom | base64 -w 100 >"$RUN/www/big"
head -c 4096 /dev/zero | tr '\0' 'x' >"$RUN/post.body"

cat >"$RUN/nginx.conf" <<NGINX
worker_processes 1;
daemon off;
pid $RUN/nginx.pid;
error_log $RUN/nginx-error.log warn;
events { worker_connections 4096; }
http {
    access_log off;
    default_type text/plain;
    client_body_temp_path $RUN/nginx/body;
    proxy_temp_path $RUN/nginx/proxy;
    fastcgi_temp_path $RUN/nginx/fastcgi;
    uwsgi_temp_path $RUN/nginx/uwsgi;
    scgi_temp_path $RUN/nginx/scgi;
    server {
        listen 127.0.0.1:9001;
        listen 127.0.0.1:9002;
        location = /small { return 200 "ok\n"; }
        location = /health { return 200; }
        location = /big { root $RUN/www; }
        location = /post { client_max_body_size 1m; return 200 "posted\n"; }
    }
}
NGINX

cat >"$CFG" <<YAML
server:
  threads: 2
  shutdown_grace: 10s
  pid_file: $RUN/conduit.pid
  upgrade_sock: $RUN/upgrade.sock
  error_log: $OUT/conduit-stderr.log
listen:
  http: ["127.0.0.1:8080"]
  https: ["127.0.0.1:8443"]
admin:
  listen: "127.0.0.1:9180"
access_log:
  path: $RUN/access.log
tls:
  watch_interval: 5s
  certs:
    - { name: soak, cert: $RUN/soak.crt, key: $RUN/soak.key }
upstreams:
  web:
    backends: [http://127.0.0.1:9001, http://127.0.0.1:9002]
    retries: 1
    health_check: { path: /health, interval: 2s, timeout: 1s, rise: 2, fall: 2 }
    passive: { failures: 3, eject: 10s }
sites:
  - id: web
    hosts: [soak.test]
    upstream: web
  - id: ws
    hosts: [ws.soak.test]
    upstream: http://127.0.0.1:9100
    upgrade_idle_timeout: 120s
YAML

sudo -n nft delete table inet soak 2>/dev/null
sudo -n nft add table inet soak
sudo -n nft add chain inet soak input '{ type filter hook input priority 0; }'

nginx -c "$RUN/nginx.conf" -p "$RUN/nginx" 2>>"$OUT/nginx.log" &
NGINX_PID=$!
python3 -I "$HERE/ws.py" server 9100 2>>"$OUT/ws-server.log" &
WS_SERVER=$!
# As deploy/conduit.service runs it: daemonized, returning once the child is ready.
"$BIN" run -d -c "$CFG" || { event "conduit did not start"; exit 1; }
event "start pid=$(cat "$RUN/conduit.pid") hours=$HOURS upgrade_at=${UPGRADE_AT// /,} sha256=$(sha256sum "$BIN" | cut -c1-16)"

stream() { # name, oha arguments...
    local name=$1
    shift
    while [ "$(date +%s)" -lt $((END - 30)) ]; do
        local left=$((END - $(date +%s)))
        local dur=$((left < WINDOW ? left : WINDOW))
        "$OHA" --no-tui --output-format json -w -z "${dur}s" "$@" \
            >"$OUT/oha/$name-$(date +%s).json" 2>>"$OUT/oha-stderr.log"
    done
}
C="--connect-to soak.test:8080:127.0.0.1:8080 --connect-to soak.test:8443:127.0.0.1:8443"
stream h1 $C -q 250 -c 16 http://soak.test:8080/small &
stream big $C -q 20 -c 4 -H "Accept-Encoding: gzip" http://soak.test:8080/big &
stream post $C -q 50 -c 4 -m POST -D "$RUN/post.body" http://soak.test:8080/post &
stream tls $C -q 150 -c 8 --insecure https://soak.test:8443/small &
stream h2 $C -q 150 -c 2 -p 8 --http2 --insecure https://soak.test:8443/small &
stream churn $C -q 30 -c 4 --disable-keepalive --insecure https://soak.test:8443/small &
python3 -I "$HERE/ws.py" client 8080 8443 "$END" "$OUT/ws.csv" "$OUT/ws-events.log" \
    2>>"$OUT/ws-client.log" &
LOAD=$(jobs -p | tail -n 7)

sample() {
    local pid
    pid=$(cat "$RUN/conduit.pid" 2>/dev/null) || return
    [ -d "/proc/$pid" ] || { echo "$(date +%s),$pid,dead" >>"$OUT/samples.csv"; return; }
    local rss fds thr cpu up ws cli tw m
    rss=$(awk '/^VmRSS/ {print $2}' "/proc/$pid/status")
    fds=$(ls "/proc/$pid/fd" | wc -l)
    thr=$(ls "/proc/$pid/task" | wc -l)
    cpu=$(awk '{print $14 + $15}' "/proc/$pid/stat")
    up=$(ss -Htn state established '( dport = :9001 or dport = :9002 )' | wc -l)
    ws=$(ss -Htn state established '( dport = :9100 )' | wc -l)
    cli=$(ss -Htn state established '( sport = :8080 or sport = :8443 )' | wc -l)
    tw=$(ss -Htn state time-wait | wc -l)
    m=$(curl -sf --max-time 5 http://127.0.0.1:9180/metrics | awk '
        $1 == "conduit_access_log_dropped_total" {d = $2}
        $1 == "conduit_upstream_reuse_lost_total" {r = $2}
        END {printf "%s,%s", d + 0, r + 0}')
    echo "$(date +%s),$pid,$rss,$fds,$thr,$cpu,$up,$ws,$cli,$tw,$m" >>"$OUT/samples.csv"
}
[ -s "$OUT/samples.csv" ] || echo "ts,pid,rss_kb,fds,threads,cpu_ticks,upstream_conns,ws_conns,client_conns,time_wait,log_dropped,reuse_lost" >"$OUT/samples.csv"

# Counts the lines of a finished access log and keeps the ones with an error or a 4xx/5xx
# status, so failed requests can be traced on conduit's side.
keep_log() {
    echo "$(date +%s) $(wc -l <"$1")" >>"$OUT/logcount.txt"
    grep -E '"error":|"status":[45]' "$1" >>"$OUT/access-errors.log"
}

rotate_log() {
    mv "$RUN/access.log" "$RUN/access.log.1" || return
    event "access log rotated"
    sleep 12 # the writer notices within 5 s and flushes every 20 ms
    keep_log "$RUN/access.log.1"
    rm -f "$RUN/access.log.1"
}

flap() {
    sudo -n nft add rule inet soak input iif lo tcp dport 9002 reject with tcp reset
    event "flap 9002 down"
    sleep "$(every 30)"
    sudo -n nft flush chain inet soak input
    event "flap 9002 up"
}

upgrade() {
    local old
    old=$(cat "$RUN/conduit.pid")
    # systemctl reload in deploy/conduit.service: the new daemon returns once it waits for
    # the listeners, then the old process hands them over on SIGQUIT.
    "$BIN" run -d -u -c "$CFG" || { event "upgrade: new process failed"; return 1; }
    # The event is written once the old process is gone; quit= is when the upgrade began,
    # exit_s how long the old process took to stop.
    local quit=$EPOCHREALTIME
    kill -QUIT "$old"
    for _ in $(seq 300); do [ -d "/proc/$old" ] || break; sleep 0.1; done
    local exit_s served
    exit_s=$(awk -v a="$quit" -v b="$EPOCHREALTIME" 'BEGIN {printf "%.1f", b - a}')
    served=$(curl -sf --max-time 5 http://127.0.0.1:9180/status |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["pid"])')
    event "upgrade old=$old new=$(cat "$RUN/conduit.pid") served_by=$served old_gone=$([ -d "/proc/$old" ] && echo no || echo yes) quit=${quit%.*} exit_s=$exit_s"
    if [ -d "/proc/$old" ] || [ "$served" != "$(cat "$RUN/conduit.pid")" ]; then
        event "upgrade: handover failed"
        return 1
    fi
}

next_sample=$START
next_flap=$((START + $(every 600) + $(every 150)))
next_reload=$((START + $(every 150)))
next_log=$((START + $(every 900)))
next_cert=$((START + $(every 1800)))
# Upgrade times: a third into the second window after a whole hour, the one without a flap,
# and away from the reloads (which come half a window in).
upgrades=()
for h in $UPGRADE_AT; do
    upgrades+=("$(awk -v s="$START" -v h="$h" -v k="$SCALE" -v w="$WINDOW" \
        'BEGIN {printf "%d", s + h * 3600 / k + w * 4 / 3}')")
done
while [ "$(date +%s)" -lt "$END" ]; do
    now=$(date +%s)
    if [ "$now" -ge "$next_sample" ]; then
        sample
        next_sample=$((now + 30))
    fi
    if [ "$now" -ge "$next_flap" ]; then
        flap &
        next_flap=$((next_flap + $(every 600)))
    fi
    if [ "$now" -ge "$next_reload" ]; then
        kill -HUP "$(cat "$RUN/conduit.pid")"
        event "reload"
        next_reload=$((next_reload + $(every 300)))
    fi
    if [ "$now" -ge "$next_log" ]; then
        rotate_log &
        next_log=$((next_log + $(every 1800)))
    fi
    if [ "$now" -ge "$next_cert" ]; then
        cert
        event "certificate rotated"
        next_cert=$((next_cert + $(every 3600)))
    fi
    if [ "${#upgrades[@]}" -gt 0 ] && [ "$now" -ge "${upgrades[0]}" ]; then
        upgrade || { event "abort upgrade failed"; exit 1; }
        upgrades=("${upgrades[@]:1}")
    fi
    sleep 2
done

# shellcheck disable=SC2086
wait $LOAD
sleep 15 # a flap or log rotation still running
sample
keep_log "$RUN/access.log"
kill -TERM "$(cat "$RUN/conduit.pid")" "$NGINX_PID" "$WS_SERVER" 2>/dev/null
sudo -n nft delete table inet soak
event "end"
python3 -I "$HERE/report.py" "$OUT" >"$OUT/report.txt" 2>&1
