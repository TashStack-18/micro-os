# Micro-OS Linux: component checks per stage

Each stage booted in the UEFI/Secure Boot VM with a virtio NIC on QEMU's NAT (the "LAN");
`microos-verify` ran inside the guest. A VM has no Wi-Fi radio, so Wi-Fi is checked as
driver + firmware + tools here and must be confirmed on the laptop (`sudo microos-verify`).

## S0 — minimal base (kernel, systemd) - no package manager, no network

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  base: no APT yet (expected)
```

Result: passed=3 failed=0

## S1 — + LAN networking (NetworkManager, Realtek Ethernet firmware)

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  base: no APT yet (expected)
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
```

Result: passed=8 failed=0

## S2 — + Wi-Fi (wpa_supplicant, iw, regulatory database; RTL8852BE driver + firmware)

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  base: no APT yet (expected)
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
PASS  wifi: RTL8852BE driver (rtw89_8852be)  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/wireless/realtek/rtw89/rtw89_8852be.k)
PASS  wifi: RTL8852B firmware  (/lib/firmware/rtw89/rtw8852b_fw-1.bin)
PASS  wifi: wpa_supplicant  (wpa_supplicant v2.10)
PASS  wifi: NetworkManager Wi-Fi  (enabled)
PASS  wifi: regulatory database  (/lib/firmware/regulatory.db)
SKIP  wifi: no Wi-Fi adapter here (a VM has none; run this on the laptop)
```

Result: passed=13 failed=0

## S3 — + laptop hardware (Intel microcode, graphics, audio, Bluetooth firmware)

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  base: no APT yet (expected)
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
PASS  wifi: RTL8852BE driver (rtw89_8852be)  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/wireless/realtek/rtw89/rtw89_8852be.k)
PASS  wifi: RTL8852B firmware  (/lib/firmware/rtw89/rtw8852b_fw-1.bin)
PASS  wifi: wpa_supplicant  (wpa_supplicant v2.10)
PASS  wifi: NetworkManager Wi-Fi  (enabled)
PASS  wifi: regulatory database  (/lib/firmware/regulatory.db)
SKIP  wifi: no Wi-Fi adapter here (a VM has none; run this on the laptop)
PASS  hw: Intel CPU microcode  (126 microcode files)
PASS  hw: Intel graphics firmware  (/lib/firmware/i915)
PASS  hw: audio (SOF) firmware  (/lib/firmware/intel/sof)
PASS  hw: Bluetooth stack  (bluetoothctl: 5.82)
PASS  hw: Realtek Bluetooth firmware  (/lib/firmware/rtl_bt/rtl8852btu_config.bin)
PASS  hw: PCI devices visible  (7 PCI devices)
```

Result: passed=19 failed=0

## S4 — + APT and aptitude package managers (Debian repositories)

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
PASS  wifi: RTL8852BE driver (rtw89_8852be)  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/wireless/realtek/rtw89/rtw89_8852be.k)
PASS  wifi: RTL8852B firmware  (/lib/firmware/rtw89/rtw8852b_fw-1.bin)
PASS  wifi: wpa_supplicant  (wpa_supplicant v2.10)
PASS  wifi: NetworkManager Wi-Fi  (enabled)
PASS  wifi: regulatory database  (/lib/firmware/regulatory.db)
SKIP  wifi: no Wi-Fi adapter here (a VM has none; run this on the laptop)
PASS  hw: Intel CPU microcode  (126 microcode files)
PASS  hw: Intel graphics firmware  (/lib/firmware/i915)
PASS  hw: audio (SOF) firmware  (/lib/firmware/intel/sof)
PASS  hw: Bluetooth stack  (bluetoothctl: 5.82)
PASS  hw: Realtek Bluetooth firmware  (/lib/firmware/rtl_bt/rtl8852btu_config.bin)
PASS  hw: PCI devices visible  (7 PCI devices)
PASS  apt: apt-get update  (package lists downloaded)
PASS  apt: install a package (hello)  (Hello, world!)
PASS  apt: aptitude works  (i  hello - example package based on GNU hello)
PASS  apt: package count  (194 packages installed)
```

Result: passed=22 failed=0

## S5 — + snapd (the Snap package system)

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
PASS  wifi: RTL8852BE driver (rtw89_8852be)  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/wireless/realtek/rtw89/rtw89_8852be.k)
PASS  wifi: RTL8852B firmware  (/lib/firmware/rtw89/rtw8852b_fw-1.bin)
PASS  wifi: wpa_supplicant  (wpa_supplicant v2.10)
PASS  wifi: NetworkManager Wi-Fi  (enabled)
PASS  wifi: regulatory database  (/lib/firmware/regulatory.db)
SKIP  wifi: no Wi-Fi adapter here (a VM has none; run this on the laptop)
PASS  hw: Intel CPU microcode  (126 microcode files)
PASS  hw: Intel graphics firmware  (/lib/firmware/i915)
PASS  hw: audio (SOF) firmware  (/lib/firmware/intel/sof)
PASS  hw: Bluetooth stack  (bluetoothctl: 5.82)
PASS  hw: Realtek Bluetooth firmware  (/lib/firmware/rtl_bt/rtl8852btu_config.bin)
PASS  hw: PCI devices visible  (7 PCI devices)
PASS  apt: apt-get update  (package lists downloaded)
PASS  apt: install a package (hello)  (Hello, world!)
PASS  apt: aptitude works  (i  hello - example package based on GNU hello)
PASS  apt: package count  (208 packages installed)
PASS  snap: snapd socket listening  (active)
PASS  snap: snapd answers  (snapd   2.68.3-3+deb13u1+b7)
PASS  snap: seeded  (seeded)
PASS  snap: store reachable (find)  (hello-world                      6.4                  canonical**       -      The 'hello-)
PASS  snap: install from store  (Hello World!)
```

Result: passed=27 failed=0

## S6 — + desktop (Xfce) and the Snap Store app

```
PASS  base: kernel booted  (Linux 6.12.116+deb13-amd64)
PASS  base: systemd running  (systemd 257 (257.13-1~deb13u1))
PASS  lan: NetworkManager active  (active)
PASS  lan: wired link connected
PASS  lan: Ethernet device  (enp0s2:ethernet:connected)
PASS  lan: Realtek r8169 driver  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/ethernet/realtek/r8169.ko.xz)
PASS  lan: internet over HTTPS  (HTTP 200)
PASS  wifi: RTL8852BE driver (rtw89_8852be)  (/lib/modules/6.12.116+deb13-amd64/kernel/drivers/net/wireless/realtek/rtw89/rtw89_8852be.k)
PASS  wifi: RTL8852B firmware  (/lib/firmware/rtw89/rtw8852b_fw-1.bin)
PASS  wifi: wpa_supplicant  (wpa_supplicant v2.10)
PASS  wifi: NetworkManager Wi-Fi  (enabled)
PASS  wifi: regulatory database  (/lib/firmware/regulatory.db)
SKIP  wifi: no Wi-Fi adapter here (a VM has none; run this on the laptop)
PASS  hw: Intel CPU microcode  (126 microcode files)
PASS  hw: Intel graphics firmware  (/lib/firmware/i915)
PASS  hw: audio (SOF) firmware  (/lib/firmware/intel/sof)
PASS  hw: Bluetooth stack  (bluetoothctl: 5.82)
PASS  hw: Realtek Bluetooth firmware  (/lib/firmware/rtl_bt/rtl8852btu_config.bin)
PASS  hw: PCI devices visible  (7 PCI devices)
PASS  apt: apt-get update  (package lists downloaded)
PASS  apt: install a package (hello)  (Hello, world!)
PASS  apt: aptitude works  (i  hello - example package based on GNU hello)
PASS  apt: package count  (515 packages installed)
PASS  snap: snapd socket listening  (active)
PASS  snap: snapd answers  (snapd         2.78)
PASS  snap: seeded  (seeded)
PASS  snap: store reachable (find)  (hello-world                      6.4                  canonical**       -      The 'hello-)
PASS  snap: install from store  (Hello World!)
PASS  desktop: login screen (LightDM)  (active)
PASS  store: Snap Store installed  (snap-store  0+git.5534399d  1468  2/stable  canonical**  -)
PASS  store: Snap Store launcher  (snap-store_packagekit-session-installer.desktop)
```

Result: passed=30 failed=0

