use core::arch::asm;

#[link_section = ".text.strap"]
unsafe fn trap_switch_to_s_level() {
    let mut mstatus: u64;
    asm!("csrr {}, mstatus", out(reg) mstatus);
    mstatus &= !0x1800;
    mstatus |= 0x800; // set MPP to S
    asm!("csrw mstatus, {}", in(reg) mstatus);
    asm!("csrw pmpcfg0, 0x1f");
    asm!("csrw pmpaddr0, {}", in(reg) u64::MAX);
    asm!("csrw satp, 0");

    /* set return address */
    let mut mepc: u64;
    asm!("csrr {}, mepc", out(reg) mepc);
    mepc += 4;
    asm!("csrw mepc, {}", in(reg) mepc);
    asm!("mret");
}

pub unsafe fn switch_to_s_level() {
    let mut mtvec: u64;
    asm!("csrr {}, mtvec", out(reg) mtvec);
    asm!("csrw mtvec, {}", in(reg) trap_switch_to_s_level);
    asm!("ecall");
    asm!("csrw mtvec, {}", in(reg) mtvec);
}
