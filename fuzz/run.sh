#!/usr/bin/env bash
# Run every fuzz target for SECONDS (default 300) in parallel. New inputs go to corpus/,
# crashes to artifacts/, logs to target/fuzz-<target>.log. Needs nightly and cargo-fuzz
# (`rustup toolchain install nightly`, `cargo install cargo-fuzz`).
set -eu
cd "$(dirname "$0")"
secs=${1:-300}
targets=(config path access_log request route body_replace block_exploits)
cargo +nightly fuzz build -O
pids=()
for t in "${targets[@]}"; do
    mkdir -p "corpus/$t"
    cargo +nightly fuzz run -O "$t" "corpus/$t" "seeds/$t" -- \
        -max_total_time="$secs" -timeout=10 -rss_limit_mb=2048 >"target/fuzz-$t.log" 2>&1 &
    pids+=($!)
done
fail=0
for p in "${pids[@]}"; do wait "$p" || fail=1; done
for t in "${targets[@]}"; do
    printf '%-11s %s\n' "$t" "$(grep -E '^Done|panicked' "target/fuzz-$t.log" | head -1)"
done
exit $fail
