# Supervisor trap entry and exit.
#
# Kernel tasks run in S-mode on their own kernel stack. A task may also drop
# to U-mode (user mode, see src/user/) on a separate user stack; a trap from
# U-mode must then switch to the task's kernel stack before saving anything.
#
# sscratch tells the two cases apart:
#   sscratch == 0       the trap came from S-mode: keep using sp
#   sscratch == ksp     the trap came from U-mode: ksp is this task's kernel
#                       stack top, set by the return-to-user path below
#
# TrapFrame pushed on the kernel stack:
#   offset  0..256  x0..x31   (x0 slot unused, x2 = sp before the trap)
#   offset  256     sstatus
#   offset  264     sepc
#   size    288     (rounded up to keep sp 16-byte aligned)
#
# sstatus and sepc are saved per frame because trap_handler may switch to
# another task (preemption); when this task is resumed later the hart's
# CSRs belong to someone else and must be restored from here before sret.

    .section .text
    .globl __trap_entry
    .align 4
__trap_entry:
    csrrw   sp, sscratch, sp        # sp = sscratch, sscratch = old sp
    bnez    sp, 1f                  # non-zero: came from U-mode, sp = kernel stack
    csrrw   sp, sscratch, sp        # zero: came from S-mode, swap back (sscratch = 0)
1:
    addi    sp, sp, -288
    sd      x1, 8(sp)
    sd      x3, 24(sp)
    sd      x4, 32(sp)
    sd      x5, 40(sp)
    sd      x6, 48(sp)
    sd      x7, 56(sp)
    sd      x8, 64(sp)
    sd      x9, 72(sp)
    sd      x10, 80(sp)
    sd      x11, 88(sp)
    sd      x12, 96(sp)
    sd      x13, 104(sp)
    sd      x14, 112(sp)
    sd      x15, 120(sp)
    sd      x16, 128(sp)
    sd      x17, 136(sp)
    sd      x18, 144(sp)
    sd      x19, 152(sp)
    sd      x20, 160(sp)
    sd      x21, 168(sp)
    sd      x22, 176(sp)
    sd      x23, 184(sp)
    sd      x24, 192(sp)
    sd      x25, 200(sp)
    sd      x26, 208(sp)
    sd      x27, 216(sp)
    sd      x28, 224(sp)
    sd      x29, 232(sp)
    sd      x30, 240(sp)
    sd      x31, 248(sp)
    # Interrupted sp: the user sp (left in sscratch) or, from S-mode, the
    # kernel sp before this frame. sscratch becomes 0 while in the kernel.
    csrrw   t0, sscratch, zero
    bnez    t0, 2f
    addi    t0, sp, 288
2:
    sd      t0, 16(sp)
    csrr    t0, sstatus
    sd      t0, 256(sp)
    csrr    t0, sepc
    sd      t0, 264(sp)

    mv      a0, sp
    call    trap_handler

    .globl __trap_return
__trap_return:
    ld      t0, 256(sp)
    csrw    sstatus, t0
    ld      t0, 264(sp)
    csrw    sepc, t0
    # Returning to U-mode (sstatus.SPP = 0): the next trap must find this
    # task's kernel stack top in sscratch.
    csrr    t0, sstatus
    andi    t0, t0, 0x100
    bnez    t0, 3f
    addi    t0, sp, 288
    csrw    sscratch, t0
3:
    ld      x1, 8(sp)
    ld      x3, 24(sp)
    ld      x4, 32(sp)
    ld      x6, 48(sp)
    ld      x7, 56(sp)
    ld      x8, 64(sp)
    ld      x9, 72(sp)
    ld      x10, 80(sp)
    ld      x11, 88(sp)
    ld      x12, 96(sp)
    ld      x13, 104(sp)
    ld      x14, 112(sp)
    ld      x15, 120(sp)
    ld      x16, 128(sp)
    ld      x17, 136(sp)
    ld      x18, 144(sp)
    ld      x19, 152(sp)
    ld      x20, 160(sp)
    ld      x21, 168(sp)
    ld      x22, 176(sp)
    ld      x23, 184(sp)
    ld      x24, 192(sp)
    ld      x25, 200(sp)
    ld      x26, 208(sp)
    ld      x27, 216(sp)
    ld      x28, 224(sp)
    ld      x29, 232(sp)
    ld      x30, 240(sp)
    ld      x31, 248(sp)
    csrr    t0, sstatus
    andi    t0, t0, 0x100
    bnez    t0, 4f
    ld      t0, 40(sp)
    ld      sp, 16(sp)              # to U-mode: switch to the user stack
    sret
4:
    ld      t0, 40(sp)
    addi    sp, sp, 288             # to S-mode: pop the frame
    sret

# enter_user(entry: a0, user_sp: a1, arg: a2) -> !
# Leave the calling kernel task for U-mode at `entry` on `user_sp`, with
# `arg` in a0. Interrupts are masked first: from here until sret, sscratch
# is non-zero while still in S-mode, which a trap must never see.
    .globl __enter_user
__enter_user:
    csrci   sstatus, 2              # SIE = 0
    csrw    sepc, a0
    li      t0, 0x100
    csrc    sstatus, t0             # SPP = 0: sret goes to U-mode
    li      t0, 0x20
    csrs    sstatus, t0             # SPIE = 1: interrupts on in U-mode
    csrw    sscratch, sp            # kernel stack for the first trap
    mv      sp, a1
    mv      a0, a2
    mv      ra, zero
    sret
