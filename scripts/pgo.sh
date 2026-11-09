#!/usr/bin/env bash
# Profile-guided build of conduit. Builds an instrumented binary, runs scripts/pgo-train.sh
# against it, merges the profile, and rebuilds with it, so LLVM lays out and inlines code by
# what the proxy actually runs most instead of by guesses.
#
#   scripts/pgo.sh [--out PATH]      (default target/pgo/conduit)
#
# Needs an llvm-profdata from the same LLVM major version as rustc: `rustup component add
# llvm-tools` provides one, or point LLVM_PROFDATA at it. Builds use --target so RUSTFLAGS
# reach conduit and its dependencies but not build scripts and proc macros, and their own
# target directories, so they never invalidate the ordinary release build.
# Environment: LLVM_PROFDATA, PGO_SCALE and PGO_PORT_BASE (passed to pgo-train.sh).
set -euo pipefail
cd "$(dirname "$0")/.."

out=target/pgo/conduit
case "${1:-}" in
  --out) out=${2:?--out needs a path} ;;
  "") ;;
  *) sed -n '2,12p' "$0"; exit 2 ;;
esac

host=$(rustc -vV | awk '/^host:/ { print $2 }')
llvm=$(rustc -vV | awk '/^LLVM version:/ { split($3, v, "."); print v[1] }')
profdata=
for cand in "${LLVM_PROFDATA:-}" "$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-profdata" \
  $(command -v "llvm-profdata-$llvm" llvm-profdata 2>/dev/null); do
  [ -n "$cand" ] && [ -x "$cand" ] || continue
  if "$cand" --version | grep -q "LLVM version $llvm\."; then profdata=$cand; break; fi
done
[ -n "$profdata" ] || {
  echo "pgo: no llvm-profdata for LLVM $llvm (rustc's); run: rustup component add llvm-tools" >&2
  exit 1
}

dir=$PWD/target/pgo
rm -rf "$dir/raw" "$dir/conduit.profdata"
mkdir -p "$dir/raw"
# build <rustc flag> <target dir>: the flag carries a path, which may contain spaces, so it
# goes through CARGO_ENCODED_RUSTFLAGS (0x1f-separated) along with any RUSTFLAGS given.
build() {
  local flags=()
  read -ra flags <<< "${RUSTFLAGS:-}"
  flags+=("$1")
  CARGO_ENCODED_RUSTFLAGS=$(IFS=$'\x1f'; printf '%s' "${flags[*]}") \
    cargo build --release --locked --target "$host" --target-dir "$2"
}

echo "pgo: instrumented build"
build "-Cprofile-generate=$dir/raw" "$dir/gen"
echo "pgo: training"
LLVM_PROFILE_FILE="$dir/raw/conduit-%p-%m.profraw" \
  scripts/pgo-train.sh "$dir/gen/$host/release/conduit"
compgen -G "$dir/raw/*.profraw" >/dev/null || { echo "pgo: no profile was written" >&2; exit 1; }
"$profdata" merge -o "$dir/conduit.profdata" "$dir/raw"
echo "pgo: profile $(du -h "$dir/conduit.profdata" | cut -f1) from $(ls "$dir/raw" | wc -l) run(s)"

echo "pgo: optimized build"
build "-Cprofile-use=$dir/conduit.profdata" "$dir/use"
mkdir -p "$(dirname "$out")"
cp "$dir/use/$host/release/conduit" "$out"
"$out" --version
echo "pgo: wrote $out"
