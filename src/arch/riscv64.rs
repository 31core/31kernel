pub mod cpu;
pub mod page;
mod trap;

use core::arch::asm;

pub const CLINT: u64 = 0x2000000;
pub const CLINT_MTIME: u64 = CLINT + 0xbff8;
pub const CLINT_MTIMECMP: u64 = CLINT + 0x4000;

const TIMER_INTERVAL: u64 = 1000;

pub fn get_sys_time() -> u64 {
    unsafe { (CLINT_MTIME as *const u64).read_volatile() }
}

pub unsafe fn enable_timer() {
    set_timer(TIMER_INTERVAL);

    /* set MIE flag */
    let mut mstatus: u64;
    asm!("csrr {}, mstatus", out(reg) mstatus);
    mstatus |= 1 << 3;
    asm!("csrw mstatus, {}", in(reg) mstatus);

    /* set MTIE flag */
    let mut mtie: u16;
    asm!("csrr {}, mie", out(reg) mtie);
    mtie |= 1 << 7;
    asm!("csrw mie, {}", in(reg) mtie);
}

pub fn set_timer(interval: u64) {
    unsafe {
        (CLINT_MTIMECMP as *mut u64).write_volatile(get_sys_time() + interval);
    }
}
