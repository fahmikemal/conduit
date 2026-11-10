#!/usr/bin/env bash
# Start a native release soak on a disposable host; refuses to replace an active run.
# SOAK_BIN, OHA_BIN and SOAK_PYTHON_LIB select locally staged build/fixture artifacts.
# scripts/soak/deploy.sh HOST [HOURS]
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 scripts/soak/deploy.py "$@"
