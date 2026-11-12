#!/usr/bin/env bash
# Boot a Micro-OS Linux image in a UEFI virtual machine (KVM, Secure Boot on).
#
#   distro/vm.sh STAGE [MODE] [TIMEOUT_S]
#     MODE: measure  - record boot time, then power off (default)
#           verify   - run the component checks, then power off
#           console  - interactive serial console (log in as student/microos)
#
# The serial log goes to distro/out/logs/. Network: QEMU user-mode NAT on a
# virtio NIC ("LAN"); a VM has no Wi-Fi adapter.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
STAGE="${1:?usage: vm.sh STAGE [measure|verify|console] [timeout]}"
MODE="${2:-measure}"
TIMEOUT="${3:-240}"
IMG="${IMG:-$HERE/out/microos-s$STAGE.img}"   # IMG=... boots a copy instead
[ -f "$IMG" ] || { echo "no image $IMG (run build.sh $STAGE)" >&2; exit 1; }
mkdir -p "$HERE/out/logs" "$HERE/out/vars"
VARS="$HERE/out/vars/${VARS_NAME:-s$STAGE}.fd"
# Keep the UEFI variable store per image, like a real machine's NVRAM.
[ -f "$VARS" ] || cp /usr/share/OVMF/OVMF_VARS_4M.ms.fd "$VARS"

QEMU=(qemu-system-x86_64 -enable-kvm -cpu host -smp 4 -m 4096
      -machine q35,smm=on -global driver=cfi.pflash01,property=secure,value=on
      -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.ms.fd
      -drive if=pflash,format=raw,unit=1,file="$VARS"
      -drive file="$IMG",if=virtio,format=raw,cache=unsafe
      -nic user,model=virtio-net-pci
      -display none -monitor none -no-reboot)

case "$MODE" in
console)
    exec "${QEMU[@]}" -serial mon:stdio
    ;;
measure|verify)
    LOG="$HERE/out/logs/s$STAGE-$MODE-$(date +%Y%m%d-%H%M%S).log"
    set +e
    timeout "$TIMEOUT" "${QEMU[@]}" -serial "file:$LOG" -fw_cfg "name=opt/microos/$MODE,string=1"
    rc=$?
    set -e
    echo "$LOG"
    [ "$rc" = 124 ] && { echo "timeout after ${TIMEOUT}s" >&2; exit 124; }
    exit 0
    ;;
*)
    echo "unknown mode $MODE" >&2; exit 2 ;;
esac
