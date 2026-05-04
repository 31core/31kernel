.equ VIRT_ADDR, 0xffffffc040000000

.altmacro
.macro PUSH n
    addi sp, sp, -8
    sd n, (sp)
.endm

.macro POP n
    ld n, (sp)
    addi sp, sp, 8
.endm

.section .text.entry
.globl _start
_start:
    ld t0, 4f
    csrw stvec, t0
    mv a0, a1

    la sp, boot_stack_top
    PUSH a0
    call set_boot_pt
    POP a0

    /* set satp */
    la t0, boot_l2_page
    srli t0, t0, 12
    li t1, 8 << 60
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    ld sp, 1f

    ld ra, 2f
    jr ra

/* void set_boot_pt(); */
set_boot_pt:
    PUSH ra
    la a0, VIRT_ADDR
    la a1, boot_l1_page
    call set_boot_pt_l2

    la a0, VIRT_ADDR
    la a1, _start
    la a2, boot_l1_page
    call set_boot_pt_l1

    la a0, _start
    la a1, boot_l1_page
    li t0, 4096
    add a1, a1, t0
    mv s0, a1
    call set_boot_pt_l2

    la a0, _start
    la a1, _start
    mv a2, s0
    call set_boot_pt_l1
    POP ra
    ret

/*
 * Set level 2 table
 * 
 * void set_boot_pt_l2(uint64_t va, uint64_t l1_page);
 */
set_boot_pt_l2:
    /* a0 = l2 index */
    srli a0, a0, 30
    andi a0, a0, 0x1ff

    /* a0 = entry address */
    la t0, boot_l2_page
    slli a0, a0, 3
    add a0, t0, a0

    /* set l2 entry */
    srli a1, a1, 2
    ori a1, a1, 0b1 // V flag
    sd a1, (a0)
    ret

/*
 * Set level 1 table
 * 
 * void set_boot_pt_l1(uint64_t va, uint64_t pa, uint64_t l1_page);
 */
set_boot_pt_l1:
    /* a0 = l1 index */
    srli a0, a0, 21
    andi a0, a0, 0x1ff

    /* a0 = entry address */
    slli a0, a0, 3
    add a0, a2, a0

    /* set l1 entry */
    srli a1, a1, 2
    li t0, 0b1111
    or a1, a1, t0 // XWRV flag
    sd a1, (a0)
    ret

.align 3
1:  .quad kernel_stack_top
2:  .quad kernel_main
3:  .quad mtrap
4:  .quad strap

.section .data.entry
.align 12
boot_l2_page:
    .zero 4096
boot_l1_page:
    .zero 2 * 4096

    .space 4096
boot_stack_top:

.globl kernel_stack_top
.section .bss.stack
    .space 16 * 4096
kernel_stack_top:
