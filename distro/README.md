# Micro-OS Linux

A minimal operating system built from Debian 13 "trixie" packages, assembled
**one component at a time** so the boot-time cost of each component can be
measured. It complements the Rust Micro-OS kernel in this repository: the
kernel shows how OS mechanisms work inside; Micro-OS Linux delivers the
working-system requirements (networking, real laptop hardware, package
management, an app store).

| Stage | Adds | Image |
|---|---|---|
| S0 | minimal base: Linux 6.12 kernel, systemd, login. **No package manager, no network** | `out/microos-s0.img` |
| S1 | LAN: NetworkManager (DHCP), Realtek Ethernet firmware | `out/microos-s1.img` |
| S2 | Wi-Fi: wpa_supplicant (WPA2/WPA3), iw, regulatory database; RTL8852BE driver + firmware | `out/microos-s2.img` |
| S3 | laptop hardware: Intel microcode, Intel graphics and audio (SOF) firmware, Bluetooth | `out/microos-s3.img` |
| S4 | **APT** and **aptitude** with the Debian repositories | `out/microos-s4.img` |
| S5 | **snapd** (Snap packages from the Snap Store) | `out/microos-s5.img` |
| S6 | Xfce desktop and the **Snap Store** app (pre-installed) | `out/microos-s6.img` |

Each stage contains everything before it. Results:
[`results/distro_boottime.md`](../results/distro_boottime.md) (boot time per
stage) and [`results/distro_verify.md`](../results/distro_verify.md)
(component checks).

## Build and test (inside WSL, as root)

```bash
cd ~/micro-os/distro
bash build.sh all          # all seven images (S6 needs ~10 minutes)
bash boottime.sh           # boot time of every stage, 1 first + 5 normal boots
bash verify.sh             # component checks in the VM
bash vm.sh 6 console       # use a stage interactively (serial console)
```

Packages are downloaded through the local `apt-cacher-ng` cache, so
rebuilding is fast. Build tools: `mmdebstrap`, `qemu-system-x86`, `ovmf`,
`dosfstools`, `mtools`, `snapd` (for `snap prepare-image`).

Login: user **student**, password **microos** (has `sudo`; root login is
disabled). Change it with `passwd` after the first login.

## How an image is built

1. `mmdebstrap --variant=essential` installs only the stage's packages
   (no recommended extras) into a directory.
2. `customize.sh` sets the hostname, user, APT sources (S4+), the default
   target, and adds the Micro-OS tools; for S6 `snap prepare-image` pre-loads
   ("seeds") the Snap Store and its dependencies.
3. The disk image gets a GPT with an EFI system partition holding Debian's
   **Secure Boot signed** shim and GRUB, and an ext4 root file system.

## Tools inside the OS

| command | does |
|---|---|
| `microos-boottime` | boot time of this boot; `--history` lists every boot recorded on this machine |
| `sudo microos-verify` | checks each component of the stage (network, Wi-Fi, firmware, APT, Snap) |
| `microos-hwcheck` | which hardware is present and which driver handles it |

## Running on the HP Victus laptop from a USB stick

This does **not** change anything on the laptop's internal disk or Windows.
Everything on the USB stick is erased.

1. Copy the image to Windows, e.g. in PowerShell:
   `wsl -d Ubuntu -- cp /root/micro-os/distro/out/microos-s6.img /mnt/c/Users/Tashvi/Downloads/`
2. Write it to a USB stick of **16 GB or more** with
   [Rufus](https://rufus.ie) (choose the image; when asked, pick **DD image**
   mode) or balenaEtcher. Double-check the selected drive.
3. Restart the laptop and press **F9** repeatedly to open HP's boot menu;
   choose the USB stick. Secure Boot can stay on (the boot chain is signed).
4. Log in as `student` / `microos`, then:
   * Wi-Fi: click the network icon (S6), or `nmcli device wifi connect "NAME" --ask`
   * `microos-hwcheck` to see the Wi-Fi, Ethernet, graphics, audio and Bluetooth drivers
   * `sudo microos-verify` to run the component checks on real hardware
   * `microos-boottime` for this boot's time (real hardware, USB speed)

### Known hardware limitation

The **NVIDIA RTX 5050** needs NVIDIA driver 570 or newer; Debian 13 ships
550, which predates it, and the open-source `nouveau` driver in Linux 6.12
does not support it either. The **Intel UHD** graphics drive the laptop
screen, so the desktop works; the NVIDIA GPU stays unused.
