#!/usr/bin/env bash
# mmdebstrap customize hook: configure the freshly built root file system.
# Called as: customize.sh ROOTDIR   (environment set by build.sh)
set -euo pipefail
R="$1"
S="$MICROOS_STAGE"
log() { echo "[customize s$S] $*"; }

# Identity --------------------------------------------------------------------------
echo microos > "$R/etc/hostname"
cat > "$R/etc/hosts" <<'EOF'
127.0.0.1   localhost
127.0.1.1   microos
::1         localhost ip6-localhost ip6-loopback
EOF
cat > "$R/etc/microos-release" <<EOF
NAME="Micro-OS Linux"
BASED_ON="Debian 13 (trixie)"
STAGE=$S
STAGE_NAME=$MICROOS_STAGE_NAME
STAGE_TITLE="$MICROOS_TITLE"
EOF
echo "$S" > "$R/etc/microos-stage"
rm -f "$R/etc/resolv.conf"   # never ship the build host's resolver

# File systems ------------------------------------------------------------------------
cat > "$R/etc/fstab" <<EOF
# <file system>                            <mount>    <type> <options>          <dump> <pass>
UUID=$MICROOS_ROOT_UUID  /          ext4   errors=remount-ro  0      1
UUID=$MICROOS_ESP_UUID                             /boot/efi  vfat   umask=0077,nofail  0      2
EOF
mkdir -p "$R/boot/efi"

# Users: 'student' (password 'microos', may use sudo); root login disabled ------------
chroot "$R" useradd -m -s /bin/bash -G sudo student
echo 'student:microos' | chroot "$R" chpasswd
chroot "$R" passwd -l root >/dev/null
for g in netdev bluetooth audio video plugdev; do
    chroot "$R" getent group "$g" >/dev/null && chroot "$R" usermod -aG "$g" student
done

# Micro-OS tools and services ------------------------------------------------------------
cp -a "$MICROOS_DISTRO/overlay/." "$R/"
chmod 0755 "$R"/usr/local/sbin/microos-*
chroot "$R" systemctl enable -q microos-boottime.service microos-verify.service

# Package manager configuration --------------------------------------------------------
if [ -x "$R/usr/bin/apt-get" ]; then
    log "configuring APT sources (Debian 13 + security + updates)"
    rm -f "$R/etc/apt/sources.list"
    cat > "$R/etc/apt/sources.list.d/debian.sources" <<'EOF'
Types: deb
URIs: http://deb.debian.org/debian
Suites: trixie trixie-updates
Components: main contrib non-free non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg

Types: deb
URIs: http://security.debian.org/debian-security
Suites: trixie-security
Components: main contrib non-free non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
EOF
    chroot "$R" apt-get clean
    rm -rf "$R"/var/lib/apt/lists/*
else
    log "no APT in this stage: removing APT configuration left by the build"
    rm -rf "$R/etc/apt" "$R/var/lib/apt" "$R/var/cache/apt"
fi

# Desktop + Snap Store (S6) --------------------------------------------------------------
if [ "$S" -ge 6 ]; then
    chroot "$R" systemctl set-default -q graphical.target
    # Debian's snapd will not seed the Snap Store without this (see the unit).
    chroot "$R" systemctl enable -q microos-snap-features.service
    log "seeding snaps: $SEED_SNAPS"
    snap known --remote model series=16 brand-id=generic model=generic-classic > "$MICROOS_WORK/model.assert"
    args=()
    for s in $SEED_SNAPS; do args+=(--snap "$s"); done
    snap prepare-image --classic --arch=amd64 "${args[@]}" "$MICROOS_WORK/model.assert" "$R"
    # Snap desktop launchers live under /var/lib/snapd/desktop; Xfce finds them
    # through XDG_DATA_DIRS, which snapd's profile.d script sets.
else
    chroot "$R" systemctl set-default -q multi-user.target
fi

# Each copy of the image gets its own machine id on first boot.
: > "$R/etc/machine-id"
rm -f "$R/var/lib/dbus/machine-id"
log "done"
