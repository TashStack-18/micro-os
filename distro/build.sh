#!/usr/bin/env bash
# Build one stage of Micro-OS Linux into a bootable UEFI disk image.
#
#   distro/build.sh 3          -> distro/out/microos-s3.img
#   distro/build.sh all        -> stages 0..6
#
# The image is a GPT disk with an EFI system partition (Debian's Secure Boot
# signed shim + GRUB) and an ext4 root file system built by mmdebstrap from
# Debian 13 "trixie". It boots in a UEFI VM and from a USB stick on real PCs.
# Packages are fetched through the local apt-cacher-ng (port 3142).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=stages.sh
source "$HERE/stages.sh"
[ "$(id -u)" = 0 ] || { echo "build.sh must run as root" >&2; exit 1; }

if [ "${1:-}" = all ]; then
    for s in 0 1 2 3 4 5 6; do "$0" "$s"; done
    exit 0
fi
STAGE="${1:?usage: build.sh STAGE (0-6) | all}"
[[ "$STAGE" =~ ^[0-6]$ ]] || { echo "stage must be 0-6" >&2; exit 1; }

SUITE=trixie
COMPONENTS="main contrib non-free non-free-firmware"
CACHE="${CACHE:-http://127.0.0.1:3142}"
MIRROR="$CACHE/deb.debian.org/debian"
SECURITY="$CACHE/security.debian.org/debian-security"
OUT="$HERE/out"
WORK="$HERE/build/s$STAGE"
IMG="$OUT/microos-s$STAGE.img"
case "$STAGE" in
    0|1|2|3|4) DISK_MB=4096 ;;
    5) DISK_MB=6144 ;;
    6) DISK_MB=12288 ;;
esac
ESP_MB=512

mkdir -p "$OUT"
rm -rf "$WORK"
mkdir -p "$WORK"
ROOT_UUID="$(uuidgen)"
ESP_ID="$(uuidgen | tr -d '-' | cut -c1-8 | tr a-f A-F)"
ESP_UUID="${ESP_ID:0:4}-${ESP_ID:4:4}"
START=$(date +%s)
log() { echo "[build s$STAGE] $*"; }

log "${STAGE_TITLES[$STAGE]}"
log "packages: $(packages_for "$STAGE" | tr ',' ' ')"

# 1. Root file system -----------------------------------------------------------
export MICROOS_STAGE="$STAGE" MICROOS_STAGE_NAME="${STAGE_NAMES[$STAGE]}" MICROOS_TITLE="${STAGE_TITLES[$STAGE]}"
export MICROOS_ROOT_UUID="$ROOT_UUID" MICROOS_ESP_UUID="$ESP_UUID" MICROOS_DISTRO="$HERE" MICROOS_WORK="$WORK"
SEED_VAR="SEED_SNAPS_$STAGE"
export SEED_SNAPS="${!SEED_VAR:-}"
mmdebstrap \
    --mode=root \
    --variant=essential \
    --components="$COMPONENTS" \
    --aptopt='APT::Install-Recommends "false"' \
    --keyring=/usr/share/keyrings/debian-archive-keyring.gpg \
    --include="$(packages_for "$STAGE")" \
    --customize-hook="\"$HERE/customize.sh\" \"\$1\"" \
    "$SUITE" "$WORK/rootfs" \
    "deb $MIRROR $SUITE $COMPONENTS" \
    "deb $MIRROR $SUITE-updates $COMPONENTS" \
    "deb $SECURITY $SUITE-security $COMPONENTS"

# 2. Signed boot loader (shim -> GRUB), taken from the same Debian release ------
log "fetching signed shim and GRUB"
BOOT="$WORK/bootdebs"
mkdir -p "$BOOT"/{etc/apt,lists/partial,archives/partial,dpkg} "$BOOT/x"
touch "$BOOT/dpkg/status"
echo "deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] $MIRROR $SUITE main" > "$BOOT/etc/apt/sources.list"
APTB=(-o "Dir::Etc=$BOOT/etc/apt" -o "Dir::State::Lists=$BOOT/lists" -o "Dir::State::status=$BOOT/dpkg/status"
      -o "Dir::Cache::archives=$BOOT/archives" -o APT::Architecture=amd64 -o Debug::NoLocking=1
      -o APT::Sandbox::User=root)
apt-get "${APTB[@]}" update -qq
(cd "$BOOT" && apt-get "${APTB[@]}" download -qq shim-signed shim-helpers-amd64-signed grub-efi-amd64-signed)
for deb in "$BOOT"/*.deb; do dpkg-deb -x "$deb" "$BOOT/x"; done
SHIM="$BOOT/x/usr/lib/shim/shimx64.efi.signed"
MOK="$BOOT/x/usr/lib/shim/mmx64.efi.signed"
GRUB="$BOOT/x/usr/lib/grub/x86_64-efi-signed/grubx64.efi.signed"
for f in "$SHIM" "$MOK" "$GRUB"; do [ -f "$f" ] || { echo "missing $f" >&2; exit 1; }; done

TIMEOUT=0
[ "$STAGE" = 6 ] && TIMEOUT=1
cat > "$WORK/grub.cfg" <<EOF
# Micro-OS Linux ${STAGE_NAMES[$STAGE]} (stage $STAGE)
set timeout=$TIMEOUT
set default=0
search --no-floppy --fs-uuid --set=root $ROOT_UUID
menuentry "Micro-OS Linux - ${STAGE_TITLES[$STAGE]%% (*}" {
    linux /vmlinuz root=UUID=$ROOT_UUID ro quiet console=ttyS0,115200 console=tty0
    initrd /initrd.img
}
EOF

# 3. Partitions -------------------------------------------------------------------
log "creating file systems"
ROOT_MB=$((DISK_MB - ESP_MB - 2))
mkfs.vfat -C -n MICROOS-EFI -i "$ESP_ID" "$WORK/esp.img" $((ESP_MB * 1024)) >/dev/null
mmd -i "$WORK/esp.img" ::EFI ::EFI/BOOT ::EFI/debian
mcopy -i "$WORK/esp.img" "$SHIM" ::EFI/BOOT/BOOTX64.EFI
mcopy -i "$WORK/esp.img" "$GRUB" ::EFI/BOOT/grubx64.efi
mcopy -i "$WORK/esp.img" "$MOK" ::EFI/BOOT/mmx64.efi
mcopy -i "$WORK/esp.img" "$WORK/grub.cfg" ::EFI/BOOT/grub.cfg
mcopy -i "$WORK/esp.img" "$WORK/grub.cfg" ::EFI/debian/grub.cfg
mkfs.ext4 -q -F -L microos-root -U "$ROOT_UUID" -d "$WORK/rootfs" "$WORK/root.ext4" "${ROOT_MB}M"

# 4. Disk image: GPT, 1 MiB alignment ------------------------------------------------
log "assembling $IMG (${DISK_MB} MiB, sparse)"
rm -f "$IMG"
truncate -s "${DISK_MB}M" "$IMG"
sfdisk --quiet "$IMG" <<EOF
label: gpt
start=1MiB, size=${ESP_MB}MiB, type=uefi, name="EFI system"
start=$((1 + ESP_MB))MiB, size=${ROOT_MB}MiB, type=linux, name="Micro-OS root"
EOF
dd if="$WORK/esp.img" of="$IMG" bs=1M seek=1 conv=notrunc,sparse status=none
dd if="$WORK/root.ext4" of="$IMG" bs=1M seek=$((1 + ESP_MB)) conv=notrunc,sparse status=none

ROOT_USED=$(du -sm --exclude=proc --exclude=sys "$WORK/rootfs" | cut -f1)
PKGS_INSTALLED=$(chroot "$WORK/rootfs" dpkg-query -f '.\n' -W | wc -l)
cat > "$OUT/microos-s$STAGE.info" <<EOF
stage=$STAGE
name=${STAGE_NAMES[$STAGE]}
title=${STAGE_TITLES[$STAGE]}
root_used_mb=$ROOT_USED
packages_installed=$PKGS_INSTALLED
kernel=$(ls "$WORK/rootfs/boot" | sed -n 's/^vmlinuz-//p' | head -1)
built=$(date -u +%Y-%m-%dT%H:%M:%SZ)
build_seconds=$(( $(date +%s) - START ))
EOF
rm -rf "$WORK/esp.img" "$WORK/root.ext4" "$BOOT"
log "done: $IMG ($ROOT_USED MiB used, $PKGS_INSTALLED packages, $(( $(date +%s) - START )) s)"
