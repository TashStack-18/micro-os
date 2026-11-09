#!/usr/bin/env bash
# Turn the `ra=0x...` addresses of a panic backtrace into function names
# and source lines.
#
#   scripts/symbolize.sh traces/fault.log      # from a saved log
#   scripts/symbolize.sh 0x80217c0a 0x80217a22 # or explicit addresses
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ELF="$ROOT/kernel/target/riscv64gc-unknown-none-elf/release/microos"
ADDR2LINE="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-addr2line"
[ -x "$ADDR2LINE" ] || ADDR2LINE=addr2line

if [ $# -eq 1 ] && [ -f "$1" ]; then
    set -- $(grep -aoE 'ra=0x[0-9a-f]+' "$1" | cut -d= -f2)
fi
# A return address points just after the call; step back one instruction.
for a in "$@"; do
    printf '%s  ' "$a"
    "$ADDR2LINE" -f -C -i -e "$ELF" "$(printf '0x%x' $((a - 2)))" | paste -sd' '
done
