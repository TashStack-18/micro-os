//! Interrupt and exception layer.

use crate::{arch, scheduler};

core::arch::global_asm!(include_str!("trap.S"));

extern "C" {
    fn __trap_entry();
}

/// Register state saved by `trap.S`; layout must match the assembly.
#[repr(C)]
pub struct TrapFrame {
    pub regs: [usize; 32],
    pub sstatus: usize,
    pub sepc: usize,
}

pub fn init() {
    // SAFETY: __trap_entry is the 16-byte-aligned routine in trap.S that
    // saves and restores the full register state around trap_handler.
    unsafe { arch::write_stvec(__trap_entry as unsafe extern "C" fn() as usize) };
}

fn exception_name(code: usize) -> &'static str {
    match code {
        0 => "instruction address misaligned",
        1 => "instruction access fault",
        2 => "illegal instruction",
        3 => "breakpoint",
        4 => "load address misaligned",
        5 => "load access fault",
        6 => "store address misaligned",
        7 => "store access fault",
        8 => "environment call from U-mode",
        9 => "environment call from S-mode",
        12 => "instruction page fault",
        13 => "load page fault",
        15 => "store page fault",
        _ => "unknown exception",
    }
}

/// Called by `trap.S` with interrupts disabled (the hart clears sstatus.SIE
/// on trap entry). A timer interrupt may switch to another task inside
/// `on_timer_interrupt`; this function then returns only when the
/// interrupted task is scheduled again.
#[no_mangle]
extern "C" fn trap_handler(tf: &mut TrapFrame) {
    let scause = arch::read_scause();
    let code = scause & !arch::SCAUSE_INTERRUPT;
    if scause & arch::SCAUSE_INTERRUPT != 0 {
        match code {
            arch::IRQ_S_TIMER => scheduler::on_timer_interrupt(),
            _ => kwarn!("ignoring unexpected interrupt {}", code),
        }
    } else {
        panic!(
            "unhandled exception: {} (scause={:#x}) at sepc={:#x}, stval={:#x}",
            exception_name(code),
            scause,
            tf.sepc,
            arch::read_stval()
        );
    }
}
