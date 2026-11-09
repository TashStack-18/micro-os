#!/usr/bin/env bash
# Build the kernel and boot it in QEMU (RISC-V virt board, OpenSBI firmware).
#
#   scripts/run.sh [test] [extra boot args...]
#   scripts/run.sh tc2 policy=mlfq quantum=2
#
# The serial log is printed and saved to traces/<test>.log (override: LOG=path).
# Exit status: 0 = all checks passed, N = N checks failed, 101 = kernel panic,
# 124 = timeout.
#
# Environment:
#   ICOUNT=0    real-time mode (default: deterministic instruction counting,
#               so repeated runs produce identical traces)
#   TIMEOUT=s   wall-clock limit (default 300)
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEST="${1:-all}"
shift || true
ELF="$ROOT/kernel/target/riscv64gc-unknown-none-elf/release/microos"
mkdir -p "$ROOT/traces"
LOG="${LOG:-$ROOT/traces/$TEST.log}"

(cd "$ROOT/kernel" && cargo build --release --quiet)

QEMU_ARGS=(
    -machine virt -cpu rv64 -smp 1 -m 128M
    -nographic -bios default -no-reboot
    -kernel "$ELF"
    -append "test=$TEST $*"
)
if [ "${ICOUNT:-1}" != 0 ]; then
    # Virtual time advances 8 ns per instruction; idle time is skipped.
    QEMU_ARGS+=(-icount shift=3,align=off,sleep=off)
fi

set +e
timeout "${TIMEOUT:-300}" qemu-system-riscv64 "${QEMU_ARGS[@]}" </dev/null | tee "$LOG"
qemu_status=${PIPESTATUS[0]}
set -e

# Take the result from the kernel's own final trace line: it separates a
# panic from a timeout or a kernel that stopped without reporting.
if grep -aq ' PANIC' "$LOG"; then
    status=101
elif line=$(grep -a ' SHUTDOWN failed=' "$LOG" | tail -1) && [ -n "$line" ]; then
    status=$(echo "$line" | sed -E 's/.*failed=([0-9]+).*/\1/')
elif [ "$qemu_status" = 124 ]; then
    status=124
else
    status=125 # kernel stopped without reporting a result
fi
echo "[run.sh] log saved to $LOG (result $status)" >&2
exit "$status"
