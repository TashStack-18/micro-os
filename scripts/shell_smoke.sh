#!/usr/bin/env bash
# Non-interactive smoke test of the shell: types a list of commands into the
# kernel's serial port and saves the session to traces/shell_smoke.log.
set -euo pipefail
# Make cargo/rustc available even from a non-login shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ELF="$ROOT/kernel/target/riscv64gc-unknown-none-elf/release/microos"
LOG="$ROOT/traces/shell_smoke.log"
mkdir -p "$ROOT/traces"
(cd "$ROOT/kernel" && cargo build --release --quiet)

commands=(
    "help" "progs" "hello world 42" "fib 20" "counter 3 200"
    "spin 2 &" "spin 2 yield &" "ps" "primes 30000" "sched" "mem"
    "memtest 512" "sysinfo" "run nosuch" "kill 99"
)

type_commands() {
    sleep 1.5 # let the kernel boot
    for c in "${commands[@]}"; do
        printf '%s\r' "$c"
        sleep 0.9
    done
    sleep 2.5
    printf 'ps\r'
    sleep 0.5
    printf 'exit\r'
    sleep 1
}

type_commands | timeout 60 qemu-system-riscv64 \
    -machine virt -cpu rv64 -smp 1 -m 128M -nographic -bios default -no-reboot \
    -kernel "$ELF" -append "test=shell" 2>&1 | tr -d '\r' > "$LOG" || true

sed -n '/BOOT/,$p' "$LOG"
grep -q '^bye$' "$LOG" && echo "[shell_smoke] OK" || { echo "[shell_smoke] shell did not exit cleanly"; exit 1; }
