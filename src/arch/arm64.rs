pub mod cpu;
pub mod page;

pub fn get_sys_time() -> u64 {
    let ticks: u64;
    unsafe { core::arch::asm!("mrs {}, cntvct_el0" , out(reg) ticks) };

    let freq: u64;
    unsafe { core::arch::asm!("mrs {}, cntfrq_el0" , out(reg) freq) };

    ticks * 1_000_000_000 / freq
}
