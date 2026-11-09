/* Memory layout of the Micro-OS on QEMU `virt` (RAM starts at 0x8000_0000).
 *
 *   0x8000_0000  OpenSBI firmware (M-mode, PMP-protected from the kernel)
 *   0x8020_0000  kernel image: .text .rodata .data, boot stack, .bss
 *   __heap_start kernel heap (HEAP_SIZE bytes, see src/memory/mod.rs)
 */
OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    __kernel_start = .;

    .text : {
        *(.text.entry)          /* _start must be first: OpenSBI jumps here */
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    .stack (NOLOAD) : {
        __boot_stack_bottom = .;
        . += 64K;
        __boot_stack_top = .;
    }

    .bss (NOLOAD) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        . = ALIGN(8);
        __bss_end = .;
    }

    . = ALIGN(4K);
    __kernel_end = .;
    __heap_start = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
    }
}
