# Kernel entry point. OpenSBI jumps here in S-mode with:
#   a0 = hart id
#   a1 = physical address of the flattened device tree
# Paging is off, interrupts are off, and there is no stack yet.

    .section .text.entry
    .globl _start
_start:
    la      sp, __boot_stack_top

    # Zero .bss so Rust statics start in their declared state.
    # The boot stack lives in its own section, so this cannot clobber it.
    la      t0, __bss_start
    la      t1, __bss_end
1:
    bgeu    t0, t1, 2f
    sd      zero, 0(t0)
    addi    t0, t0, 8
    j       1b
2:
    # Null frame pointer terminates the panic handler's backtrace walk.
    mv      fp, zero
    # a0/a1 are untouched, so they arrive as kmain(hartid, dtb).
    call    kmain
3:
    wfi
    j       3b
