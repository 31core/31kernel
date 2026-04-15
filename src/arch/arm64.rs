pub mod cpu;
pub mod page;
mod syscall;
mod trap;

use core::arch::asm;

pub fn get_sys_time() -> u64 {
    let ticks: u64;
    unsafe { asm!("mrs {}, CNTVCT_EL0" , out(reg) ticks) };

    let freq: u64;
    unsafe { asm!("mrs {}, CNTFRQ_EL0" , out(reg) freq) };

    ticks * 1_000_000_000 / freq
}

#[inline(always)]
pub unsafe fn enable_interrupts() {
    unsafe { asm!("msr DAIFClr, #2") };
}

#[inline(always)]
pub unsafe fn disable_interrupts() {
    unsafe { asm!("msr DAIFSet, #2") };
}
