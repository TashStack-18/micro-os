#!/usr/bin/env bash
# Full validation: host unit tests, kernel suite, cross-boot repeatability,
# shell smoke test, and analysis (tables + plots in results/).
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== 1. unit tests (host) =="
(cd core && cargo test --quiet)

echo "== 2. kernel suite in QEMU =="
status=0
scripts/run.sh all > /dev/null || status=$?
grep -a -E "\[FAIL\]|checks:" traces/all.log || true

echo "== 3. repeatability across boots (TC6) =="
LOG=traces/all_repeat.log scripts/run.sh all > /dev/null || true
if cmp -s traces/all.log traces/all_repeat.log; then
    echo "two boots produced byte-identical logs"
else
    echo "logs differ:"
    diff -a traces/all.log traces/all_repeat.log | head -5 || true
    status=1
fi

echo "== 4. shell smoke test =="
scripts/shell_smoke.sh > /dev/null && echo "shell OK" || { echo "shell smoke test FAILED"; status=1; }

echo "== 5. analysis =="
python3 analysis/analyze.py traces/all.log -o results

echo "== overall: $([ "$status" = 0 ] && echo PASS || echo "FAIL ($status)") =="
exit "$status"
