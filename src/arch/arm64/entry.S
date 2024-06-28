    .section .text.entry
    .globl _start
_start:
    ldr x0, =boot_stack_top
    mov sp, x0
    bl kernel_main

    .section .bss.stack
    .space 16 * 4096
    .globl boot_stack_top
boot_stack_top:
