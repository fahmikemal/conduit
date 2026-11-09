#!/usr/bin/env bash
# Starts the soak test on HOST (a disposable machine: it installs nginx and adds an nftables
# table) as the transient systemd unit conduit-soak, with dist/conduit and the local oha.
#   scripts/soak/deploy.sh HOST [HOURS]       start (SOAK_SCALE / SOAK_SECONDS pass through)
#   ssh HOST systemctl status conduit-soak    progress; results in ~/soak/out, report.txt last
# A running soak on HOST is stopped first, and its results are deleted.
set -euo pipefail
host=$1
hours=${2:-24}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
oha=$(mise which oha)
ssh "$host" 'set -e
    sudo -n systemctl stop conduit-soak 2>/dev/null || true
    sudo -n systemctl reset-failed conduit-soak 2>/dev/null || true
    if ! command -v nginx >/dev/null; then
        sudo -n apt-get update -qq
        sudo -n DEBIAN_FRONTEND=noninteractive apt-get install -y -qq nginx >/dev/null
    fi
    # Only the soak'"'"'s own nginx runs, from its own config.
    sudo -n systemctl disable --now nginx >/dev/null 2>&1 || true
    rm -rf ~/soak/out ~/soak/run
    mkdir -p ~/soak'
scp -q "$root/dist/conduit" "$oha" "$here/soak.sh" "$here/ws.py" "$here/report.py" "$host:soak/"
ssh "$host" "sudo -n systemd-run --quiet --unit conduit-soak --uid \$(id -u) --gid \$(id -g) \
    -p WorkingDirectory=\$HOME/soak -p LimitNOFILE=65536 \
    --setenv SOAK_HOURS=$hours --setenv SOAK_SCALE=${SOAK_SCALE:-1} \
    ${SOAK_SECONDS:+--setenv SOAK_SECONDS=$SOAK_SECONDS} \
    /bin/bash \$HOME/soak/soak.sh && systemctl is-active conduit-soak"
