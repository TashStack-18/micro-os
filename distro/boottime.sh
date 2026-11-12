#!/usr/bin/env bash
# Measure the boot time of every stage of Micro-OS Linux.
#
#   distro/boottime.sh            5 repeat boots per stage, stages 0-6
#   distro/boottime.sh 10 3 4     10 repeat boots, stages 3 and 4 only
#
# For each stage a fresh copy of the image is booted once (first boot: the
# one-time setup a new install does) and then N more times (normal boots).
# Each boot is timed by systemd-analyze inside the guest. Same VM every time:
# KVM, 4 vCPUs, 4 GiB RAM, virtio disk and NIC, UEFI with Secure Boot.
# Results: results/distro_boottime.{csv,md,png}
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
N="${1:-5}"
shift || true
STAGES=("$@")
[ ${#STAGES[@]} -eq 0 ] && STAGES=(0 1 2 3 4 5 6)
RAW="$HERE/out/boottime"
mkdir -p "$RAW"

for s in "${STAGES[@]}"; do
    src="$HERE/out/microos-s$s.img"
    [ -f "$src" ] || { echo "skip s$s: not built"; continue; }
    work="$HERE/out/measure-s$s.img"
    cp --sparse=always "$src" "$work"
    rm -f "$HERE/out/vars/measure-s$s.fd" "$RAW"/s$s-boot*.log
    for i in $(seq 0 "$N"); do
        log=$(IMG="$work" VARS_NAME="measure-s$s" bash "$HERE/vm.sh" "$s" measure 900 | tail -1)
        cp "$log" "$RAW/s$s-boot$i.log"
        line=$(tr -d '\r' < "$log" | grep -m1 'Startup finished' || echo 'no result')
        printf 's%s boot %s%s: %s\n' "$s" "$i" "$([ "$i" = 0 ] && echo ' (first)')" "$line"
    done
    rm -f "$work"
done
python3 "$HERE/report.py" "$RAW" "$HERE/out" "$HERE/../results"
