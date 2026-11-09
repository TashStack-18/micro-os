//! Exit QEMU with a status code through the `sifive_test` device, so test
//! scripts can tell a passing run from a failing one.

use core::ptr::write_volatile;

use crate::{arch, drivers::sbi};

const VIRT_TEST: usize = 0x10_0000;
const FINISHER_PASS: u32 = 0x5555;
const FINISHER_FAIL: u32 = 0x3333;

pub fn exit(code: u32) -> ! {
    let value = if code == 0 { FINISHER_PASS } else { (code.min(0xffff) << 16) | FINISHER_FAIL };
    // SAFETY: VIRT_TEST is the MMIO address of QEMU virt's test finisher;
    // writing to it terminates the emulator.
    unsafe { write_volatile(VIRT_TEST as *mut u32, value) };
    sbi::system_reset(code != 0);
    loop {
        arch::wfi();
    }
}
