    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top
    la t0, mtrap
    csrw mtvec, t0

    la t0, strap
    csrw stvec, t0
    call kernel_main

    .section .bss.stack
    .space 16 * 4096
    .globl boot_stack_top
boot_stack_top:
