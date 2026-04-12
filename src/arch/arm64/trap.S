.altmacro
.macro SAVE n
    str x\n, [sp, \n * 8]
.endm

.macro LOAD n
    ldr x\n, [sp, \n * 8]
.endm

.extern el1_irq_trap_handler

.global vector_table

.section .text
.align 11
vector_table:
    /* Synchronous EL1t */
    b trap_unimplemented
    /* IRQ EL1t */
    .align 7
    b trap_unimplemented
    /* FIQ EL1t */
    .align 7
    b trap_unimplemented
    /* Error EL1t */
    .align 7
    b trap_unimplemented

    /* Synchronous EL1h */
    .align 7
    b trap_unimplemented
    /* IRQ EL1h */
    .align 7
    b el1_irq_trap
    /* FIQ EL1h */
    .align 7
    b trap_unimplemented
    /* Error EL1h */
    .align 7
    b trap_unimplemented

el1_irq_trap:
    sub sp, sp, 33 * 8
    .set n, 0
    .rept 31
        SAVE %n
        .set n, n + 1
    .endr

    mov x0, sp
    bl el1_irq_trap_handler

    .set n, 0
    .rept 31
        LOAD %n
        .set n, n + 1
    .endr
    add sp, sp, 33 * 8
    eret

trap_unimplemented:
    wfe
    b trap_unimplemented
