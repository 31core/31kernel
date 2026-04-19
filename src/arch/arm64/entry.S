    .section .text.entry
    .globl _start
_start:
    ldr x9, =boot_stack_top
    mov sp, x9
    b kernel_main

    .section .bss.stack
    .space 16 * 4096
    .globl boot_stack_top
boot_stack_top:
