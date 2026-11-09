#!/usr/bin/env bash
# Run the Micro-OS scheduling workloads on Linux and compare (CA-6 §20).
# Uses the Micro-OS results in traces/all.log, so run scripts/test_all.sh
# (or scripts/run.sh all) first.
#
#   scripts/linux_compare.sh            5 repetitions
#   scripts/linux_compare.sh --reps 10
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [ ! -f traces/all.log ]; then
    echo "traces/all.log missing: running the Micro-OS suite first"
    bash scripts/run.sh all > /dev/null || true
    python3 analysis/analyze.py traces/all.log -o results > /dev/null
fi
if [ "$(id -u)" -ne 0 ]; then
    exec sudo python3 linux/compare.py "$@"
fi
exec python3 linux/compare.py "$@"
