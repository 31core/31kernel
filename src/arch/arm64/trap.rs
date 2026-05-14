use super::{
    cpu::{Context, set_timer},
    page::{refresh_tlb, set_ttbrx},
};
use crate::{
    arch::arm64::gic::*,
    page::{KERNEL_PT, Paging},
    task::{KERNEL_PID, SCHEDULER},
};
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));

const ESR_EC_OFFSET: u64 = 26;
const ESR_EC_MASK: u64 = 0b111111;
const ESR_EC_SVC64: u64 = 0x15;

/** switch to kernel page table */
unsafe fn to_kernel_pt() {
    let tbbrx_el1 = unsafe { (*(&raw mut KERNEL_PT)).assume_init() as u64 };
    unsafe {
        set_ttbrx(tbbrx_el1);
        refresh_tlb();
    }
}

pub unsafe fn kill_task(ctx: *mut Context) {
    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    let current_pid = scheduler.current_task().pid;
    scheduler.kill(current_pid);
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn el1_sync_trap_handler(ctx: *mut Context) {
    unsafe {
        let esr_el1: u64;
        asm!("mrs {}, ESR_EL1", out(reg) esr_el1);

        to_kernel_pt();
        let ec = (esr_el1 >> ESR_EC_OFFSET) & ESR_EC_MASK;
        if ec == ESR_EC_SVC64 {
            super::syscall::syscall(ctx);
        }
    };
}

/**
 * Switch to kernel page table and execute the given function, and then restore the previous page table.
 */
fn kernel_pt_do(func: impl Fn()) {
    let ttbrx_el1;
    unsafe {
        asm!("mrs {}, TTBR0_EL1", out(reg) ttbrx_el1);
        to_kernel_pt();
        func();
        set_ttbrx(ttbrx_el1);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn el1_irq_trap_handler(ctx: *mut Context) {
    unsafe { to_kernel_pt() };

    let irq = unsafe { gicc_mmio_read(GICC_IAR) };
    if irq == INTID_VTIMER {
        crate::time::timer();
        set_timer();
        task_switch(ctx);
        kernel_pt_do(|| unsafe {
            gicc_mmio_write(GICC_EOIR, irq);
        });
    }
}

fn task_switch(ctx: *mut Context) {
    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    if scheduler.current_task().pid != KERNEL_PID {
        unsafe { asm!("mrs {}, SP_EL0", out(reg)(*ctx).sp) };
    }

    let next_task = scheduler.switch_task(ctx);
    if next_task.pid != KERNEL_PID {
        unsafe { asm!("msr SP_EL0, {}", in(reg) (*ctx).sp) };
    }

    unsafe {
        next_task.page.switch_to();
        refresh_tlb();
    }
}
