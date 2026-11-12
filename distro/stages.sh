# Micro-OS Linux: one component per stage. Each stage contains everything
# from the stages before it, so boot time can be measured after every step.
# Sourced by build.sh. Recommends are NOT installed, so each list is exactly
# what that component brings (plus its hard dependencies).

STAGE_NAMES=(
    "base"            # S0
    "lan"             # S1
    "wifi"            # S2
    "laptop-hardware" # S3
    "apt"             # S4
    "snapd"           # S5
    "snap-store"      # S6
)

STAGE_TITLES=(
    "S0 minimal base (kernel, systemd) - no package manager, no network"
    "S1 + LAN networking (NetworkManager, Realtek Ethernet firmware)"
    "S2 + Wi-Fi (wpa_supplicant, iw, regulatory database; RTL8852BE driver + firmware)"
    "S3 + laptop hardware (Intel microcode, graphics, audio, Bluetooth firmware)"
    "S4 + APT and aptitude package managers (Debian repositories)"
    "S5 + snapd (the Snap package system)"
    "S6 + desktop (Xfce) and the Snap Store app"
)

# S0 -- what every Linux system needs to boot and log in.
# The Linux kernel and systemd; 'essential' Debian packages come with the
# build variant. APT is deliberately absent so adding it (S4) is a real step.
PKG_0="linux-image-amd64 initramfs-tools zstd systemd systemd-sysv udev kmod dbus e2fsprogs passwd login sudo procps less"

# S1 -- LAN. NetworkManager configures Ethernet by DHCP; r8169 (the laptop's
# Realtek Gaming GbE driver) is in the kernel, its firmware in firmware-realtek.
# firmware-realtek also carries the RTL8852BE Wi-Fi firmware used in S2.
PKG_1="network-manager iproute2 iputils-ping ca-certificates curl firmware-realtek"

# S2 -- Wi-Fi. The rtw89_8852be driver is in kernel 6.12; these add WPA2/WPA3
# authentication, the wireless tools and the regulatory database.
PKG_2="wpasupplicant iw wireless-regdb rfkill"

# S3 -- the HP Victus 15-fa2xxx: i7-13620H microcode, Intel UHD graphics and
# audio (SOF) firmware, Bluetooth stack, hardware inspection tools.
# NVIDIA RTX 5050: Debian 13 ships driver 550, which predates this GPU (needs
# 570+), so it is not included; the Intel GPU drives the display.
PKG_3="intel-microcode firmware-intel-graphics firmware-intel-sound firmware-sof-signed firmware-misc-nonfree bluez pciutils usbutils"

# S4 -- APT ("Advanced Package Tool") and aptitude, its text-mode front end.
PKG_4="apt aptitude apt-utils"

# S5 -- snapd: installs and runs snaps from the Snap Store (command line).
PKG_5="snapd"

# S6 -- the Snap Store is a graphical app, so it needs a desktop: Xfce (light),
# LightDM login screen, Xorg, the network applet for Wi-Fi, audio, and the
# desktop portals snaps use. The snap-store snap itself is pre-seeded.
PKG_6="xfce4 xfce4-terminal lightdm lightdm-gtk-greeter xserver-xorg-core xserver-xorg-input-libinput dbus-x11 \
libgl1-mesa-dri fonts-dejavu-core adwaita-icon-theme network-manager-applet polkitd lxpolkit \
xdg-desktop-portal xdg-desktop-portal-gtk pipewire pipewire-pulse wireplumber"

# Snaps baked into the S6 image (dependencies are added by snap prepare-image).
# snap-store, its base and the content snaps it declares as default providers;
# prepare-image refuses a snap whose base is not listed.
SEED_SNAPS_6="snapd bare core24 gtk-common-themes gnome-46-2404 mesa-2404 snap-store"

# All packages for stage $1 (cumulative).
packages_for() {
    local s=$1 i out=""
    for ((i = 0; i <= s; i++)); do
        local v="PKG_$i"
        out="$out ${!v}"
    done
    echo $out | tr ' ' ','
}
