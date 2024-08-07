.altmacro
.globl mtrap, strap

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
