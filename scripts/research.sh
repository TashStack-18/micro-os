#!/usr/bin/env bash
# Re-run the R3 study ("Fairness vs Throughput in CPU Scheduling") on the
# Micro-OS kernel, then build the paper-style tables and figures.
#
#   scripts/research.sh                    3 seeds, 2000 ms window (~6 min)
#   scripts/research.sh r3_seeds=10        more seeds, tighter confidence intervals
#   scripts/research.sh r3_window=5000     the paper's 5000 ms window
#
# Output: traces/research.log, results/r3_*.csv, results/r3_*.png,
#         results/research_sync.md
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
status=0
TIMEOUT="${TIMEOUT:-3600}" bash scripts/run.sh research "$@" > /dev/null || status=$?
grep -a -E '^\[.*(H1|H2|H3|PASS|FAIL|checks:)' traces/research.log | tr -d '\r' || true
python3 analysis/research.py traces/research.log -o results
exit "$status"
