#!/usr/bin/env bash
# Builds dist/conduit for Ubuntu 22.04 (glibc 2.35) in Docker and checks its dependencies.
set -euo pipefail
cd "$(dirname "$0")/.."

docker buildx build --output type=local,dest=dist .
bin=dist/conduit
version=$("$bin" --version)

# Highest glibc symbol version the binary needs; the production host has 2.35.
need=$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
libs=$(objdump -p "$bin" | awk '/NEEDED/ {print $2}' | paste -sd, -)

echo "$version"
echo "size:     $(du -h "$bin" | cut -f1)"
echo "glibc:    needs $need"
echo "libs:     $libs"
case "$libs" in
  *ssl*|*crypto*|*libz*) echo "error: expected OpenSSL and zlib to be linked statically" >&2; exit 1 ;;
esac
sha256sum "$bin" | tee "$bin.sha256"
