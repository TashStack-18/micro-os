# Micro-OS Linux: boot time after each component

Measured by `systemd-analyze` inside the guest. VM: KVM, 4 vCPUs, 4 GiB RAM, virtio disk and NIC, UEFI with Secure Boot (shim → GRUB → signed kernel). Mean ± standard deviation of the normal boots; *first boot* is a fresh image's first start-up (one-time setup). systemd cannot time the firmware and GRUB phases (that needs systemd-boot), so *total* = kernel (incl. initramfs) + userspace.

| stage | component added | packages | root size | kernel (s) | userspace (s) | **total (s)** | change | first boot (s) | ready target (s) | slowest unit |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| S0 | minimal base (kernel, systemd) - no package manager, no network | 106 | 331 MiB | 2.28 | 1.79 | **4.08 ± 0.58** |  | 4.44 | multi-user.target 1.14 | 2.048s dev-vda2.device |
| S1 | + LAN networking (NetworkManager, Realtek Ethernet firmware) | 159 | 398 MiB | 2.42 | 1.76 | **4.18 ± 0.14** | +0.10 | 3.90 | multi-user.target 1.42 | 559ms dev-vda2.device |
| S2 | + Wi-Fi (wpa_supplicant, iw, regulatory database; RTL8852BE driver + firmware) | 167 | 403 MiB | 2.52 | 1.92 | **4.44 ± 0.16** | +0.26 | 4.25 | multi-user.target 1.57 | 593ms dev-vda2.device |
| S3 | + laptop hardware (Intel microcode, graphics, audio, Bluetooth firmware) | 182 | 530 MiB | 2.32 | 1.80 | **4.13 ± 0.19** | -0.32 | 7.31 | multi-user.target 1.43 | 543ms dev-vda2.device |
| S4 | + APT and aptitude package managers (Debian repositories) | 193 | 563 MiB | 2.50 | 1.70 | **4.20 ± 0.55** | +0.08 | 4.89 | multi-user.target 1.27 | 457ms dev-vda2.device |
| S5 | + snapd (the Snap package system) | 207 | 648 MiB | 2.33 | 2.35 | **4.69 ± 0.62** | +0.48 | 6.25 | multi-user.target 2.35 | 796ms snapd.apparmor.service |
| S6 | + desktop (Xfce) and the Snap Store app | 514 | 2535 MiB | 2.15 | 4.42 | **6.57 ± 0.38** | +1.89 | 48.94 | graphical.target 4.42 | 2.303s snapd.apparmor.service |

Kernel: 6.12.116+deb13-amd64. 5 normal boots per stage.
