use core::arch::asm;

use asm_wrap::*;

/**
 * Wrapping for RISC-V 64 assembly instructions
*/
pub mod asm_wrap {
    use core::arch::asm;

    #[inline(always)]
    pub unsafe fn mie_r() -> u64 {
        let mut mie: u64;
        unsafe { asm!("csrr {}, mie", out(reg) mie) };
        mie
    }

    #[inline(always)]
    pub unsafe fn mie_w(mie: u64) {
        unsafe { asm!("csrw mie, {}", in(reg) mie) };
    }

    #[inline(always)]
    pub unsafe fn sie_r() -> u64 {
        let mut sie: u64;
        unsafe { asm!("csrr {}, sie", out(reg) sie) };
        sie
    }

    #[inline(always)]
    pub unsafe fn sie_w(sie: u64) {
        unsafe { asm!("csrw sie, {}", in(reg) sie) };
    }

    #[inline(always)]
    pub unsafe fn sstatus_r() -> u64 {
        let mut sstatus: u64;
        unsafe { asm!("csrr {}, sstatus", out(reg) sstatus) };
        sstatus
    }

    #[inline(always)]
    pub unsafe fn sstatus_w(sstatus: u64) {
        unsafe { asm!("csrw sstatus, {}", in(reg) sstatus) };
    }

    #[inline(always)]
    pub unsafe fn mepc_r() -> u64 {
        let mut mepc: u64;
        unsafe { asm!("csrr {}, mepc", out(reg) mepc) };
        mepc
    }

    #[inline(always)]
    pub unsafe fn mepc_w(mepc: u64) {
        unsafe { asm!("csrw mepc, {}", in(reg) mepc) };
    }

    #[inline(always)]
    pub unsafe fn sepc_r() -> u64 {
        let mut sepc: u64;
        unsafe { asm!("csrr {}, sepc", out(reg) sepc) };
        sepc
    }

    #[inline(always)]
    pub unsafe fn sepc_w(sepc: u64) {
        unsafe {
            asm!("csrw sepc, {}", in(reg) sepc);
        };
    }

    #[inline(always)]
    pub unsafe fn mtvec_r() -> u64 {
        let mut mtvec: u64;
        unsafe { asm!("csrr {}, mtvec", out(reg) mtvec) };
        mtvec
    }

    #[inline(always)]
    pub unsafe fn mtvec_w(mtvec: u64) {
        unsafe { asm!("csrw mtvec, {}", in(reg) mtvec) };
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Context {
    regs: [u64; 32],
}

#[unsafe(link_section = ".text.strap")]
unsafe fn trap_switch_to_s_level() {
    unsafe {
        let mut mstatus: u64;
        asm!("csrr {}, mstatus", out(reg) mstatus);
        mstatus &= !0x1800;
        mstatus |= 0x800; // set MPP to S
        asm!("csrw mstatus, {}", in(reg) mstatus);
        asm!("csrw pmpcfg0, 0x1f");
        asm!("csrw pmpaddr0, {}", in(reg) -1);
        asm!("csrw satp, 0");

        mepc_w(mepc_r() + 4); // set return address
        asm!("mret");
    }
}

pub unsafe fn switch_to_s_level() {
    unsafe {
        let mtvec = mtvec_r();
        mtvec_w(trap_switch_to_s_level as *const usize as usize as u64);
        asm!("ecall");
        mtvec_w(mtvec);
    }
}
