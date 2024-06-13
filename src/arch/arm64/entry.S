    .section .text.entry
    .globl _start
_start:
    mov sp, boot_stack_top
    bl kernel_main

    .section .bss.stack
    .space 16 * 4096
    .globl boot_stack_top
boot_stack_top:
