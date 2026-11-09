#!/usr/bin/env bash
# Boot the Micro-OS straight into its interactive shell (real-time clock).
#
#   scripts/shell.sh                     Round Robin, quantum 4
#   scripts/shell.sh policy=mlfq         or prio / aging / rr
#   scripts/shell.sh quantum=2 trace=1   extra kernel command-line options
#
# Leave with the shell's `exit` command, or press Ctrl-A then X.
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ELF="$ROOT/kernel/target/riscv64gc-unknown-none-elf/release/microos"

(cd "$ROOT/kernel" && cargo build --release --quiet)

exec qemu-system-riscv64 \
    -machine virt -cpu rv64 -smp 1 -m 128M \
    -nographic -bios default -no-reboot \
    -kernel "$ELF" \
    -append "test=shell $*"
