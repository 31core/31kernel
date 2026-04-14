use super::{
    cpu::{Context, set_timer},
    page::{refresh_tlb, set_tlbbrx},
};
use crate::{
    page::KERNEL_PT,
    task::{KERNEL_PID, SCHEDULER},
};
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));

#[unsafe(no_mangle)]
pub unsafe extern "C" fn el1_irq_trap_handler(ctx: *mut Context) {
    unsafe { set_timer() };

    /* switch to kernel page table */
    let tbbrx_el1 = unsafe { (*(&raw mut KERNEL_PT)).assume_init() as u64 };
    unsafe {
        set_tlbbrx(tbbrx_el1);
        refresh_tlb();
    }

    let scheduler = unsafe { (*(&raw mut SCHEDULER)).assume_init_mut() };
    if scheduler.current_task().pid != KERNEL_PID {
        unsafe { asm!("mrs {}, SP_EL0", out(reg)(*ctx).sp) };
    }
    scheduler.current_task_mut().context = unsafe { ctx.read() };
    scheduler.schedule();
    let next_task = scheduler.current_task();
    let next_ctx = next_task.context.clone();
    unsafe { ctx.write(next_ctx) };
    if next_task.pid != KERNEL_PID {
        unsafe { asm!("msr SP_EL0, {}", in(reg) (*ctx).sp) };
    }

    unsafe {
        next_task.page.switch_to();
        refresh_tlb();
    }
}
