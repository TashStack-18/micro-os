#!/usr/bin/env bash
# Start QEMU paused, waiting for a GDB connection on port 1234.
#
#   terminal 1:  scripts/debug.sh tc1
#   terminal 2:  gdb-multiarch kernel/target/riscv64gc-unknown-none-elf/release/microos \
#                  -ex 'target remote :1234' -ex 'break kmain' -ex 'continue'
# (install gdb with: sudo apt install gdb-multiarch)
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEST="${1:-tc1}"
shift || true
(cd "$ROOT/kernel" && cargo build --release --quiet)
exec qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 128M -nographic -bios default \
    -kernel "$ROOT/kernel/target/riscv64gc-unknown-none-elf/release/microos" \
    -append "test=$TEST $*" -s -S
