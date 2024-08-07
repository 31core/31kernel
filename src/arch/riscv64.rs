pub mod cpu;
pub mod page;
mod trap;

pub use cpu::asm_wrap::*;

use core::arch::asm;

const TIMER_INTERVAL: u64 = 1000;

pub fn get_sys_time() -> u64 {
    let mut time: u64;
    unsafe {
        asm!("csrr {}, time", out(reg) time);
    }
    time
}

pub unsafe fn enable_timer() {
    set_timer(TIMER_INTERVAL);

    mie_w(mie_r() | 1 << 5); // set STIE flag for mie
    sie_w(sie_r() | 1 << 5); // set STIE flag for sie

    sstatus_w(sstatus_r() | 2); // set SIE flag

    /* enable sstc extension */
    let mut menvcfg: u64;
    asm!("csrr {}, menvcfg", out(reg) menvcfg);
    menvcfg |= 1 << 63;
    asm!("csrw menvcfg, {}", in(reg) menvcfg);

    let mut mcounteren: u64;
    asm!("csrr {}, mcounteren", out(reg) mcounteren);
    mcounteren |= 2;
    asm!("csrw mcounteren, {}", in(reg) mcounteren);

    asm!("csrw mideleg, {}", in(reg) 0xffff);
}

pub fn set_timer(interval: u64) {
    unsafe {
        asm!("csrw stimecmp, {}", in(reg) get_sys_time() + interval);
    }
}
