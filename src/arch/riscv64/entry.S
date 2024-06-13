    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top
    la t1, trap
    csrw mtvec, t1
    csrw stvec, t1
    call kernel_main

    .section .bss.stack
    .space 16 * 4096
    .globl boot_stack_top
boot_stack_top:
