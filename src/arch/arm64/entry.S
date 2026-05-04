.equ VIRT_ADDR, 0xffffffc040000000
.equ TYPE_TABLE_ENTRY, 0b11
.equ TYPE_BLOCK_ENTRY, 0b01

.altmacro
.macro PUSH n
    sub sp, sp, 8
    str n, [sp]
.endm

.macro POP n
    ldr n, [sp]
    add sp, sp, 8
.endm

.section .text.entry
.globl _start
_start:
    ldr x9, =boot_stack_top
    mov sp, x9
    PUSH x0
    bl set_boot_pt
    POP x0

    ldr x9, =boot_l0_page
    msr TTBR0_EL1, x9
    msr TTBR1_EL1, x9
    dsb ish
    ldr x9, =0x85100510
    msr TCR_EL1, x9
    /* enable mmu */
    mov x9, 0x1001
    msr SCTLR_EL1, x9
    isb

    ldr x9, =kernel_stack_top
    mov sp, x9
    b kernel_main

set_boot_pt:
    PUSH x30
    /* set_l0_page(VIRT_ADDR, boot_l1_page); */
    ldr x0, =VIRT_ADDR
    ldr x1, =boot_l1_page
    bl set_l0_page

    /* set_l1_page(VIRT_ADDR, boot_l1_page, boot_l2_page); */
    ldr x0, =VIRT_ADDR
    ldr x1, =boot_l1_page
    ldr x2, =boot_l2_page
    bl set_l1_page

    /* set_l2_page(VIRT_ADDR, _start, boot_l2_page); */
    ldr x0, =VIRT_ADDR
    ldr x1, =_start
    ldr x2, =boot_l2_page
    bl set_l2_page

    /* set_l0_page(_start, boot_l1_page + 4096); */
    ldr x0, =_start
    ldr x1, =boot_l1_page
    add x1, x1, 4096
    bl set_l0_page

    /* set_l1_page(VIRT_ADDR, boot_l1_page + 4096, boot_l2_page + 4096); */
    ldr x0, =_start
    ldr x1, =boot_l1_page
    add x1, x1, 4096
    ldr x2, =boot_l2_page
    add x2, x2, 4096
    bl set_l1_page

    /* set_l2_page(V_start, _start, boot_l2_page + 4096); */
    ldr x0, =_start
    ldr x1, =_start
    ldr x2, =boot_l2_page
    add x2, x2, 4096
    bl set_l2_page
    POP x30
    ret

/*
 * Set level 3 table
 * 
 * void set_l0_page(uint64_t va, uint64_t l1_page);
 */
set_l0_page:
    /* x0 = l0 index */
    lsr x0, x0, 39
    and x0, x0, 0x1ff

    /* x0 = entry address */
    ldr x9, =boot_l0_page
    lsl x0, x0, 3
    add x0, x9, x0

    /* set l0 entry */
    orr x1, x1, TYPE_TABLE_ENTRY
    str x1, [x0]
    ret

/*
 * Set level 2 table
 * 
 * void set_l1_page(uint64_t va, uint64_t l1_page, uint64_t l2_page);
 */
set_l1_page:
    /* x0 = l1 index */
    lsr x0, x0, 30
    and x0, x0, 0x1ff

    /* x0 = entry address */
    lsl x0, x0, 3
    add x0, x1, x0

    /* set l1 entry */
    orr x2, x2, TYPE_TABLE_ENTRY
    str x2, [x0]
    ret

/*
 * Set level 1 table
 * 
 * void set_l2_page(uint64_t va, uint64_t pa, uint64_t l2_page);
 */
set_l2_page:
    /* x0 = l2 index */
    lsr x0, x0, 21
    and x0, x0, 0x1ff

    /* x0 = entry address */
    lsl x0, x0, 3
    add x0, x2, x0

    /* set l2 entry */
    orr x1, x1, TYPE_BLOCK_ENTRY
    orr x1, x1, 0x400 // AF
    str x1, [x0]
    ret

.section .data.entry
.align 12
boot_l0_page:
    .zero 4096
boot_l1_page:
    .zero 2 * 4096
boot_l2_page:
    .zero 2 * 4096

    .space 4096
boot_stack_top:

.section .bss.stack
.globl kernel_stack_top
    .space 16 * 4096
kernel_stack_top:
