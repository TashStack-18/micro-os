//! Layer 1 — boot: the assembly entry stub and reading the kernel command line.

use microos_core::{args::BootArgs, fdt};

core::arch::global_asm!(include_str!("entry.S"));

/// Read `/chosen/bootargs` from the device tree the firmware handed us.
pub fn boot_args(dtb: usize) -> BootArgs<'static> {
    if dtb == 0 {
        return BootArgs::new("");
    }
    // SAFETY: OpenSBI passes the address of a valid DTB in RAM. We read the
    // fixed-size header first and only then a slice of the declared size;
    // the DTB is never written, so a 'static shared borrow is sound.
    let blob = unsafe {
        let header = core::slice::from_raw_parts(dtb as *const u8, fdt::header_len());
        match fdt::total_size(header) {
            Some(size) => core::slice::from_raw_parts(dtb as *const u8, size),
            None => return BootArgs::new(""),
        }
    };
    BootArgs::new(fdt::find_bootargs(blob).unwrap_or(""))
}
