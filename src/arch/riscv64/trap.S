.altmacro
.globl mtrap, strap
.globl trap_switch_to_s_level

.macro SAVE n
    sd x\n, \n * 8(sp)
.endm

.macro LOAD n
    ld x\n, \n * 8(sp)
.endm

.align 2
mtrap:
    addi sp, sp, -32 * 8
    .set n, 1
    .rept 31
        SAVE %n
        .set n, n + 1
    .endr
    mv a0, sp
    call mtrap_handler
    mv sp, a0
    .set n, 1
    .rept 31
        LOAD %n
        .set n, n + 1
    .endr
    addi sp, sp, 32 * 8
    mret

.align 2
strap:
    addi sp, sp, -32 * 8
    .set n, 1
    .rept 31
        SAVE %n
        .set n, n + 1
    .endr
    mv a0, sp
    call strap_handler
    mv sp, a0
    .set n, 1
    .rept 31
        LOAD %n
        .set n, n + 1
    .endr
    addi sp, sp, 32 * 8
    sret

.align 2
trap_switch_to_s_level:
    csrw mtvec, t0

    csrr t0, mstatus
    li t1, ~0x1800
    and t0, t0, t1
    # set MPP to S
    li t1, 0x800
    or t0, t0, t1
    csrw mstatus, t0

    li t0, 0x1f
    csrw pmpcfg0, t0
    li t0, -1
    csrw pmpaddr0, t0

    csrw satp, zero

    # set return address
    csrr t0, mepc
    addi t0, t0, 4
    csrw mepc, t0

    mret
