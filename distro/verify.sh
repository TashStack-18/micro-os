#!/usr/bin/env bash
# Boot each stage in the VM and run its component checks (microos-verify).
#
#   distro/verify.sh            stages 0-6
#   distro/verify.sh 4 6        selected stages
#
# Uses a throw-away copy of each image (the checks install packages).
# Results: results/distro_verify.md
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
STAGES=("$@")
[ ${#STAGES[@]} -eq 0 ] && STAGES=(0 1 2 3 4 5 6)
OUTMD="$HERE/../results/distro_verify.md"
mkdir -p "$(dirname "$OUTMD")" "$HERE/out/verify"

{
    echo "# Micro-OS Linux: component checks per stage"
    echo
    echo "Each stage booted in the UEFI/Secure Boot VM with a virtio NIC on QEMU's NAT (the \"LAN\");"
    echo "\`microos-verify\` ran inside the guest. A VM has no Wi-Fi radio, so Wi-Fi is checked as"
    echo "driver + firmware + tools here and must be confirmed on the laptop (\`sudo microos-verify\`)."
    echo
} > "$OUTMD"

for s in "${STAGES[@]}"; do
    src="$HERE/out/microos-s$s.img"
    [ -f "$src" ] || { echo "skip s$s: not built"; continue; }
    work="$HERE/out/verify-s$s.img"
    cp --sparse=always "$src" "$work"
    rm -f "$HERE/out/vars/verify-s$s.fd"
    log=$(IMG="$work" VARS_NAME="verify-s$s" bash "$HERE/vm.sh" "$s" verify 1200 | tail -1)
    cp "$log" "$HERE/out/verify/s$s.log"
    block=$(tr -d '\r' < "$log" | sed -n '/MICROOS-VERIFY-BEGIN/,/MICROOS-VERIFY-END/p')
    summary=$(echo "$block" | grep -o 'passed=[0-9]* failed=[0-9]*' || echo 'no result')
    echo "s$s: $summary"
    {
        echo "## S$s — $(sed -n 's/^title=//p' "$HERE/out/microos-s$s.info" | cut -d' ' -f2-)"
        echo
        echo '```'
        echo "$block" | grep -E '^(PASS|FAIL|SKIP)' || echo "(no result: see distro/out/verify/s$s.log)"
        echo '```'
        echo
        echo "Result: $summary"
        echo
    } >> "$OUTMD"
    rm -f "$work"
done
echo "wrote $OUTMD"
