use super::cpu::Context;
use crate::{
    arch::riscv64::{page::MODE_SV39, *},
    page::KERNEL_PT,
    task::{KERNEL_PID, SCHEDULER},
};
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));

const INTERRUPT_FLAG: u64 = 1 << 63;

const MCAUSE_ECALL_U: u64 = 8;
const MCAUSE_ECALL_S: u64 = 9;
const MCAUSE_ECALL_M: u64 = 11;

const SCAUSE_ILLEGAL_INS: u64 = 2;
const SCAUSE_TIMER_S: u64 = 5 | INTERRUPT_FLAG;
const SCAUSE_ECALL_U: u64 = 8;
const SCAUSE_ECALL_S: u64 = 9;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mtrap_handler(ctx: &mut Context) -> &mut Context {
    let mut mcause: u64;
    unsafe { asm!("csrr {}, mcause", out(reg) mcause) };

    if mcause == MCAUSE_ECALL_M || mcause == MCAUSE_ECALL_S || mcause == MCAUSE_ECALL_U {
        unsafe { mepc_w(mepc_r() + 4) }; // set return address
    }
    ctx
}

unsafe fn to_kernel_pt() {
    let kernel_ppn = unsafe { (*(&raw mut KERNEL_PT)).assume_init() as u64 };
    let satp = kernel_ppn | (MODE_SV39 << 60);
    unsafe {
        asm!("csrw satp, {}", in(reg) satp);
        asm!("sfence.vma");
    }
}

/**
 * Kill a task, it is called by trap, and it does:
 * * Remove the task from scheduler.
 * * Set up the next task's conext.
 * * Switch to the next task's page table.
 */
pub unsafe fn kill_task(ctx: *mut Context) {
    let scheduler = unsafe { (*(&raw mut SCHEDULER)).assume_init_mut() };
    scheduler.kill(scheduler.current_task().pid);
    scheduler.schedule();
    let next_task = scheduler.current_task();
    let next_ctx = next_task.context.clone();
    unsafe { ctx.write(next_ctx) };

    if next_task.pid != KERNEL_PID {
        unsafe { asm!("csrc sstatus, {}", in(reg) 1 << 8) }; // set SPP to user mode
    } else {
        unsafe { asm!("csrs sstatus, {}", in(reg) 1 << 8) }; // set SPP to supervisor mode
    }

    unsafe {
        next_task.page.switch_to();
        asm!("sfence.vma");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strap_handler(ctx: *mut Context) {
    let mut scause: u64;
    unsafe { asm!("csrr {}, scause", out(reg) scause) };

    if scause == SCAUSE_ECALL_U || scause == SCAUSE_ECALL_S {
        unsafe { (*ctx).epc += 4 }; // set return address
        if scause == SCAUSE_ECALL_U {
            unsafe {
                to_kernel_pt();
                super::syscall::syscall(ctx)
            };
        }
    } else if scause == SCAUSE_TIMER_S {
        crate::time::timer();
        super::set_timer(super::TIMER_INTERVAL);

        unsafe { to_kernel_pt() };

        let scheduler = unsafe { (*(&raw mut SCHEDULER)).assume_init_mut() };
        scheduler.current_task_mut().context = unsafe { ctx.read() };
        scheduler.schedule();
        let next_task = scheduler.current_task();
        let next_ctx = next_task.context.clone();
        unsafe { ctx.write(next_ctx) };

        if next_task.pid != KERNEL_PID {
            unsafe { asm!("csrc sstatus, {}", in(reg) 1 << 8) }; // set SPP to user mode
        } else {
            unsafe { asm!("csrs sstatus, {}", in(reg) 1 << 8) }; // set SPP to supervisor mode
        }

        unsafe {
            next_task.page.switch_to();
            asm!("sfence.vma");
        }
    } else if scause == SCAUSE_ILLEGAL_INS {
        unsafe {
            to_kernel_pt();
            kill_task(ctx);
        }
    }
}
