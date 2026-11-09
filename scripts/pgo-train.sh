#!/usr/bin/env bash
# Training workload for profile-guided optimization (see scripts/pgo.sh). Runs the given
# instrumented conduit against a self-contained setup and stops it cleanly, which is when
# the profile is written. Traffic follows what the production host sees: mostly HTTP/1.1 keep-alive
# through the proxy (Cloudflare to origin), then HTTPS, HTTP/2, new TLS handshakes, plugins,
# the response cache, the file server, compression, redirects and errors.
#
#   scripts/pgo-train.sh <conduit binary>
#
# conduit's own file server is the upstream, so nothing but curl and openssl is needed.
# Environment: PGO_PORT_BASE (19080; uses this port and the next two), PGO_SCALE (1: request
# counts are multiplied by it).
set -euo pipefail

BIN=$(realpath "$1")
BASE=${PGO_PORT_BASE:-19080}
SCALE=${PGO_SCALE:-1}
HTTP=$BASE HTTPS=$((BASE + 1)) ADMIN=$((BASE + 2))
work=$(mktemp -d)
pid=
cleanup() {
  [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

mkdir -p "$work/www/assets" "$work/logs"
head -c 2048 /dev/zero | tr '\0' 'x' > "$work/www/index.html"
seq 1 4000 | sed 's/.*/console.log("line &");/' > "$work/www/assets/app.js"
seq 1 300 | sed 's/.*/.c& { color: #333; margin: 0 }/' > "$work/www/assets/style.css"
gzip -k -9 "$work/www/assets/style.css"
head -c 300000 /dev/urandom > "$work/www/assets/video.bin"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
  -subj /CN=train.local -addext "subjectAltName=DNS:app.train,DNS:secure.train,DNS:cdn.train" \
  -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null

cat > "$work/conduit.yaml" <<EOF
server: { threads: 2, pid_file: $work/conduit.pid, upgrade_sock: $work/upgrade.sock, shutdown_grace: 1s, shutdown_timeout: 2s }
listen: { http: ["127.0.0.1:$HTTP"], https: ["127.0.0.1:$HTTPS"] }
admin: { listen: "127.0.0.1:$ADMIN" }
access_log: { path: $work/logs/access.log }
real_ip: { header: CF-Connecting-IP, trusted: [127.0.0.1/32] }
tls:
  certs: [{ name: train, cert: $work/cert.pem, key: $work/key.pem }]
upstreams:
  origin:
    backends: ["http://127.0.0.1:$HTTP"]
    health_check: { type: http, path: /index.html, host: origin.train, interval: 1s }
sites:
  - id: origin
    hosts: [origin.train]
    files: { root: $work/www, precompressed: [gzip] }
  - id: app
    hosts: [app.train]
    upstream: origin
    upstream_host: origin.train
    hsts: "max-age=63072000"
    plugins:
      - headers: { response: { set: { X-Frame-Options: SAMEORIGIN }, remove: [X-Powered-By] } }
      - rules:
          - match: { path: { prefix: /admin } }
            action: { deny: 403 }
  - id: api
    hosts: [api.train]
    upstream: { backends: ["http://127.0.0.1:$HTTP"] }
    upstream_host: origin.train
    plugins:
      - ip_filter: { allow: [127.0.0.0/8, 10.0.0.0/8] }
      - rate_limit: { requests: 100000000, per: 1s }
      - concurrency: { max: 100000 }
  - id: cdn
    hosts: [cdn.train]
    upstream: origin
    upstream_host: origin.train
    cache: { default_ttl: 1m }
  - id: secure
    hosts: [secure.train]
    force_https: true
    upstream: origin
    upstream_host: origin.train
EOF

CONDUIT_LOG=warn "$BIN" run -c "$work/conduit.yaml" 2>"$work/conduit.err" &
pid=$!
for _ in $(seq 150); do
  (exec 3<>"/dev/tcp/127.0.0.1/$ADMIN") 2>/dev/null && break
  sleep 0.1
done

# urls <file> <scheme://host:port> <count> path... : <count> requests cycling over the paths.
urls() {
  local file=$1 origin=$2 count=$3; shift 3
  local paths=("$@") i
  : > "$file"
  for ((i = 0; i < count * SCALE; i++)); do
    printf 'url = "%s%s"\noutput = "/dev/null"\n' "$origin" "${paths[i % ${#paths[@]}]}" >> "$file"
  done
}
# fetch <parallel> <curl options...> -- (reads the URL list from $work/u.cfg)
fetch() {
  local par=$1; shift
  curl -sS -Z --parallel-max "$par" -K "$work/u.cfg" "$@" || true
}
plain="http://127.0.0.1:$HTTP"
tls="https://app.train:$HTTPS"
resolve=(--resolve "app.train:$HTTPS:127.0.0.1" --resolve "secure.train:$HTTPS:127.0.0.1"
  --resolve "cdn.train:$HTTPS:127.0.0.1" -k)
cf=(-H "CF-Connecting-IP: 198.51.100.7")
began=$(date +%s)

# HTTP/1.1 keep-alive through the proxy, as from Cloudflare: the bulk of the traffic.
urls "$work/u.cfg" "$plain" 24000 /index.html /index.html /index.html /index.html \
  /assets/style.css /index.html?page=2 /missing /admin/login
fetch 16 -H "Host: app.train" "${cf[@]}"
# Text that conduit compresses, and ranges of a large file.
urls "$work/u.cfg" "$plain" 1500 /assets/app.js /assets/style.css
fetch 8 -H "Host: app.train" -H "Accept-Encoding: gzip" "${cf[@]}"
urls "$work/u.cfg" "$plain" 300 /assets/video.bin
fetch 4 -H "Host: app.train" -r 0-65535
# Plugins on the API site, with request bodies.
urls "$work/u.cfg" "$plain" 3000 /v1/items /v1/items?page=3 /v1/orders
fetch 8 -H "Host: api.train" "${cf[@]}"
urls "$work/u.cfg" "$plain" 1000 /v1/items
fetch 8 -H "Host: api.train" -H "Content-Type: application/json" -d '{"name":"x","qty":1}'
# Cache hits.
urls "$work/u.cfg" "$plain" 4000 /index.html /assets/style.css /assets/app.js
fetch 8 -H "Host: cdn.train" "${cf[@]}"
# The file server directly, with validators and precompressed files.
urls "$work/u.cfg" "$plain" 2000 /index.html /assets/style.css /assets/ /nope
fetch 8 -H "Host: origin.train" -H "Accept-Encoding: br, gzip"
urls "$work/u.cfg" "$plain" 500 /index.html
fetch 4 -H "Host: origin.train" -H "If-Modified-Since: Sun, 01 Nov 2099 00:00:00 GMT"
# Redirects to HTTPS and unknown hosts.
urls "$work/u.cfg" "$plain" 500 / /login
fetch 4 -H "Host: secure.train"
fetch 4 -H "Host: unknown.train"
# HTTPS keep-alive (HTTP/1.1).
urls "$work/u.cfg" "$tls" 8000 /index.html /index.html /assets/style.css /admin
fetch 8 --http1.1 "${resolve[@]}"
# HTTP/2.
urls "$work/u.cfg" "$tls" 6000 /index.html /assets/style.css /assets/app.js
fetch 32 --http2 "${resolve[@]}" --compressed
urls "$work/u.cfg" "https://cdn.train:$HTTPS" 1500 /index.html /assets/app.js
fetch 16 --http2 "${resolve[@]}"
# New TLS handshakes: resumed (curl keeps sessions) and full.
urls "$work/u.cfg" "$tls" 400 /index.html
fetch 4 --http1.1 "${resolve[@]}" -H "Connection: close"
fetch 4 --http1.1 "${resolve[@]}" -H "Connection: close" --no-sessionid
# HEAD and a reload in the middle of traffic.
urls "$work/u.cfg" "$plain" 500 /index.html
fetch 4 -I -H "Host: app.train"
kill -HUP "$pid"
sleep 0.5
urls "$work/u.cfg" "$plain" 2000 /index.html
fetch 8 -H "Host: app.train" "${cf[@]}"

metrics=$(curl -s "http://127.0.0.1:$ADMIN/metrics")
served=$(awk '/^conduit_requests_total/ { n += $NF } END { printf "%d", n }' <<< "$metrics")
classes=$(awk '/^conduit_requests_total/ { match($0, /code="[^"]*"/); c = substr($0, RSTART + 6, RLENGTH - 7); n[c] += $NF }
  END { for (c in n) printf "%s %d  ", c, n[c] }' <<< "$metrics")
kill -TERM "$pid"
wait "$pid" 2>/dev/null || true
pid=
echo "pgo-train: $served requests in $(( $(date +%s) - began ))s ($classes)"
if grep -qE ' (ERROR|WARN) ' "$work/conduit.err"; then
  grep -E ' (ERROR|WARN) ' "$work/conduit.err" | sort | uniq -c | sort -rn | head -5 >&2
fi
[ "$served" -gt $((40000 * SCALE)) ] || { echo "pgo-train: too few requests served" >&2; exit 1; }
