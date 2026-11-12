# Shared helpers for the Micro-OS tools.

STAGE="$(cat /etc/microos-stage 2>/dev/null || echo '?')"
FWCFG=/sys/firmware/qemu_fw_cfg/by_name/opt/microos

load_fwcfg() { modprobe -q qemu_fw_cfg 2>/dev/null || true; }

# True when the QEMU test harness asked for a mode (an opt/microos/<name>
# fw_cfg entry). Never true on real hardware, so a laptop is never powered off.
vm_flag() {
    load_fwcfg
    [ -e "$FWCFG/$1/raw" ]
}

# Inside the test harness, results go to the serial port it captures.
# The serial login (agetty) resets the port when it starts and flushes any
# output still queued, which cut reports short; so stop it first, write the
# whole text at once, and wait for the port to drain before going on.
to_console() {
    load_fwcfg
    if [ -d "$FWCFG" ] && [ -w /dev/ttyS0 ]; then
        systemctl stop serial-getty@ttyS0.service 2>/dev/null || true
        buf=$(cat)
        printf '%s\n' "$buf" > /dev/ttyS0
        stty -F /dev/ttyS0 >/dev/null 2>&1 || true  # tcdrain-like barrier
        sleep 2
    else
        cat
    fi
}

# Wait until systemd has finished starting units, but never forever: a unit
# stuck in "activating" would otherwise hang the caller. Returns 1 on timeout.
wait_for_boot() {
    timeout "${1:-300}" systemctl is-system-running --wait >/dev/null 2>&1
    [ $? -ne 124 ]
}
