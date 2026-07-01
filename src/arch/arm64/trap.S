.altmacro
/* Save a pair of registers */
.macro SAVEP n1, n2
    stp x\n1, x\n2, [sp, #(\n1 * 8)]
.endm

.macro SAVEALL
    sub sp, sp, #(34 * 8)
    .set n1, 0
    .set n2, 1
    .rept 15
        SAVEP %n1, %n2
        .set n1, n1 + 2
        .set n2, n2 + 2
    .endr
    str x30, [sp, #(29 * 8)]

    /* save ELR_EL1 */
    mrs x9, ELR_EL1
    str x9, [sp, #(32 * 8)]

    /* save pstate */
    mrs x9, SPSR_EL1
    str x9, [sp, #(33 * 8)]
.endm

/* Load a pair of registers */
.macro LOADP n1, n2
    ldp x\n1, x\n2, [sp, #(\n1 * 8)]
.endm

.macro LOADALL
    /* restore ELR_EL1 */
    ldr x9, [sp, #(32 * 8)]
    msr ELR_EL1, x9

    /* restore pstate */
    ldr x9, [sp, #(33 * 8)]
    msr SPSR_EL1, x9
    .set n1, 0
    .set n2, 1
    .rept 15
        LOADP %n1, %n2
        .set n1, n1 + 2
        .set n2, n2 + 2
    .endr
    ldr x30, [sp, #(29 * 8)]
    add sp, sp, #(34 * 8)
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

    /* Synchronous EL0 */
    .align 7
    b el1_sync_trap
    /* IRQ EL0 */
    .align 7
    b el1_irq_trap
    /* FIQ EL0 */
    .align 7
    b trap_unimplemented
    /* Error EL0 */
    .align 7
    b trap_unimplemented

el1_sync_trap:
    SAVEALL

    mov x0, sp
    bl el1_sync_trap_handler

    LOADALL
    eret

el1_irq_trap:
    SAVEALL

    mov x0, sp
    bl el1_irq_trap_handler

    LOADALL
    eret

trap_unimplemented:
    wfi
    b trap_unimplemented
