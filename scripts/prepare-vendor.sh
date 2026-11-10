#!/usr/bin/env bash
# Idempotently apply the HTTP/2 pool fix over the pinned Pingora submodule.
set -euo pipefail
cd "$(dirname "$0")/.."

tree=vendor/pingora
backport=vendor/pingora-h2-reuse.patch
if [ ! -f "$tree/pingora-core/src/connectors/http/v2.rs" ]; then
  echo 'Pingora source missing; run git submodule update --init --recursive.' >&2
  exit 1
fi
if patch --batch --forward --dry-run -p1 -d "$tree" < "$backport" >/dev/null 2>&1; then
  patch --batch --forward -p1 -d "$tree" < "$backport"
elif patch --batch --reverse --dry-run -p1 -d "$tree" < "$backport" >/dev/null 2>&1; then
  : # Already applied.
else
  echo 'Pingora HTTP/2 backport does not match this source; inspect vendor/pingora-h2-reuse.patch.' >&2
  exit 1
fi
