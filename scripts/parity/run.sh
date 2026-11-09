#!/usr/bin/env bash
# Local parity test, nothing leaves this machine: NPM's own nginx (the NPM image, nginx only,
# without its backend) serving a copy of NPM's host configs, against conduit serving what
# `conduit import-npm` makes of the same copy. Both forward to stub.py, which answers with the
# request it received, so scripts/parity.py compares responses and forwarded requests.
#
#   scripts/parity/run.sh NPM_COPY CERTS [OVERLAY] [CASES]
#
# NPM_COPY  a copy of NPM's root (/opt/npm) holding at least data/nginx/{proxy_host,default_host}
#           and, when used, data/nginx/default_www and data/customfiles
# CERTS     test certificates npm-<id>.crt + npm-<id>.key for the certificates the hosts use
# OVERLAY   optional YAML map: site id -> lines added to that site in the imported config
#           (the hand translation of NPM advanced configs); `{tree}` is the test NPM root
# CASES     optional extra requests for scripts/parity.py --cases
#
# Needs Docker and a release build (target/release/conduit). Output in target/parity/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
npm=$(realpath "$1")
certs=$(realpath "$2")
overlay=${3:+$(realpath "$3")}
cases=${4:+$(realpath "$4")}
out=$root/target/parity
tree=$out/tree
bin=$root/target/release/conduit
image=jc21/nginx-proxy-manager:2.14.0
name=conduit-parity-npm
npm_http=18080 npm_https=18443 c_http=28080 c_https=28443 admin=29180

pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    # nginx in the container writes as root; hand its files back before removing it.
    docker exec "$name" chown -R "$(id -u):$(id -g)" /data /etc/letsencrypt 2>/dev/null || true
    docker rm -f "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT

wait_port() {
    for _ in $(seq 100); do
        (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && return 0
        sleep 0.1
    done
    echo "port $1 did not open" >&2
    return 1
}

docker rm -f "$name" >/dev/null 2>&1 || true
# Files nginx wrote as root in an interrupted run need root to remove.
rm -rf "$out" 2>/dev/null || docker run --rm -v "$root/target:/t" --entrypoint rm "$image" -rf /t/parity
mkdir -p "$tree/data/logs" "$tree/data/custom_ssl" "$tree/data/letsencrypt-acme-challenge" \
    "$tree/letsencrypt/live" "$out/run"
cp -r "$npm/data/nginx" "$tree/data/"
if [ -d "$npm/data/customfiles" ]; then cp -r "$npm/data/customfiles" "$tree/data/"; fi
for crt in "$certs"/npm-*.crt; do
    id=$(basename "$crt" .crt)
    for d in "$tree/letsencrypt/live/$id" "$tree/data/custom_ssl/$id"; do
        mkdir -p "$d"
        cp "$crt" "$d/fullchain.pem"
        cp "$certs/$id.key" "$d/privkey.pem"
    done
done

# Every upstream (address, port) gets a stub port. The stub runs inside the NPM container
# (a host firewall may refuse the Docker bridge); conduit reaches it through published ports.
"$bin" import-npm --npm-root "$npm" --paths-root "$tree" >"$out/import.yaml" 2>"$out/import.log"
python3 -I - "$tree" "$out" "$overlay" <<'EOF'
import json, re, sys
from pathlib import Path
tree, out, overlay = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
ports = {}
def stub(addr, port):
    return ports.setdefault((addr, port), 20000 + len(ports))
for conf in sorted((tree / "data/nginx/proxy_host").glob("*.conf")):
    text = conf.read_text()
    addr = re.search(r'set \$server\s+"([^"]+)";', text).group(1)
    port = re.search(r"set \$port\s+(\d+);", text).group(1)
    p = stub(addr, port)
    text = re.sub(r'set \$server\s+"[^"]+";', f'set $server "127.0.0.1";', text)
    text = re.sub(r"set \$port\s+\d+;", f"set $port {p};", text)
    conf.write_text(text)

y = (out / "import.yaml").read_text()
def upstream(m):
    return f'upstream: "http://127.0.0.1:{stub(m.group(1), m.group(2))}"'
y = re.sub(r'upstream: "http://([^:"]+):(\d+)"', upstream, y)
y = re.sub(r'  http: \[.*\]', '  http: ["127.0.0.1:28080"]', y)
y = re.sub(r'  https: \[.*\]', '  https: ["127.0.0.1:28443"]', y)
y = re.sub(r'  listen: "127.0.0.1:9180"', '  listen: "127.0.0.1:29180"', y)
y = re.sub(r"(access_log:\n  path: ).*", rf"\g<1>{out}/access.log", y)
y = y.replace("server:\n", f"server:\n  pid_file: {out}/run/conduit.pid\n"
              f"  upgrade_sock: {out}/run/upgrade.sock\n  shutdown_grace: 1s\n", 1)
if overlay != "":
    import yaml  # PyYAML, only to read the overlay's block strings
    extra = {str(k): v for k, v in yaml.safe_load(open(overlay)).items()}
    head, *sites = re.split(r"\n(?=  - id: )", y)
    for i, block in enumerate(sites):
        sid = re.match(r'  - id: "?([^"\n]+)"?', block).group(1)
        if sid in extra:
            add = extra.pop(sid).replace("{tree}", str(tree)).rstrip().split("\n")
            body = block.rstrip("\n")
            # import-npm ends a site with its plugins (block_exploits): the overlay's
            # plugins join that list, its other settings go before it.
            if "\n    plugins:\n" in body and "plugins:" in add:
                at = add.index("plugins:")
                items = [l for l in add[at + 1:] if l.startswith(" ")]
                rest = add[:at] + add[at + 1 + len(items):]
                head_, plugins = body.split("\n    plugins:\n", 1)
                body = head_ + "".join(f"\n    {l}" for l in rest) + "\n    plugins:\n" + plugins
                add = items
            sites[i] = body + "\n" + "".join(f"    {l}\n" for l in add)
    if extra:
        sys.exit(f"overlay names unknown sites: {sorted(extra)}")
    y = "\n".join([head] + [s.rstrip("\n") for s in sites]) + "\n"
(out / "conduit.yaml").write_text(y)
(out / "stub-ports.txt").write_text(" ".join(str(p) for p in ports.values()))
json.dump({f"{a}:{p}": s for (a, p), s in ports.items()}, open(out / "stub-ports.json", "w"), indent=1)
EOF
"$bin" check -c "$out/conduit.yaml" >"$out/check.log"

stub_ports=$(cat "$out/stub-ports.txt")
first=${stub_ports%% *}
last=${stub_ports##* }
"$bin" run -c "$out/conduit.yaml" >"$out/conduit.log" 2>&1 &
pids+=($!)
docker run -d --name "$name" -p "127.0.0.1:$npm_http:80" -p "127.0.0.1:$npm_https:443" \
    -p "127.0.0.1:$first-$last:$first-$last" -e "STUB_PORTS=$stub_ports" \
    -v "$here/stub.py:/stub.py:ro" -v "$tree/data:/data" -v "$tree/letsencrypt:/etc/letsencrypt" \
    --entrypoint sh "$image" -c '
        echo "resolver 127.0.0.11 valid=10s;" > /etc/nginx/conf.d/include/resolvers.conf
        sed -i "s/^user npm;/user root;/" /etc/nginx/nginx.conf
        mkdir -p /run/nginx /tmp/nginx/body /var/lib/nginx/cache/public /var/lib/nginx/cache/private \
            /var/log/nginx /var/cache/nginx
        /opt/certbot/bin/python3 -I /stub.py $STUB_PORTS >/data/logs/stub.log 2>&1 &
        exec nginx' >/dev/null
for p in $npm_http $npm_https $c_http $c_https $admin $first $last; do wait_port "$p"; done
sleep 1
if ! docker exec "$name" nginx -t >/dev/null 2>&1; then docker logs "$name" | tail -20; exit 1; fi

set +e
python3 -I "$root/scripts/parity.py" "$out/conduit.yaml" 127.0.0.1 127.0.0.1 \
    --http-port "$npm_http,$c_http" --https-port "$npm_https,$c_https" --echo \
    ${cases:+--cases "$cases"} --full "$out/diff.txt"
status=$?
echo "every difference: $out/diff.txt"
exit $status
