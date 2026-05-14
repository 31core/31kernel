use super::{cpu::Context, page::refresh_tlb};
use crate::{
    page::Paging,
    syscall::*,
    task::{KERNEL_PID, SCHEDULER},
};
use core::arch::asm;

unsafe fn syscall_sleep(ctx: *mut Context, timestamp: u64) {
    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    if scheduler.current_task().pid != KERNEL_PID {
        unsafe { asm!("mrs {}, SP_EL0", out(reg)(*ctx).sp) };
    }

    let next_time = crate::time::get_sys_time() + timestamp;
    scheduler.current_task_mut().next_schedule = Some(next_time);
    let next_task = scheduler.switch_task(ctx);

    if next_task.pid != KERNEL_PID {
        unsafe { asm!("msr SP_EL0, {}", in(reg) (*ctx).sp) };
    }

    unsafe {
        next_task.page.switch_to();
        refresh_tlb();
    }
}

unsafe fn syscall_fork(ctx: *mut Context) {
    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    if scheduler.current_task().pid != KERNEL_PID {
        unsafe { asm!("mrs {}, SP_EL0", out(reg)(*ctx).sp) };
    }

    unsafe {
        scheduler.current_task_mut().context = ctx.read();
    }
    let child_pid = scheduler.fork();
    unsafe {
        (*ctx).x[0] = child_pid as u64;
        scheduler.tasks.get_mut(&child_pid).unwrap().context.x[0] = 0; // child process returns 0
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

pub unsafe fn syscall(ctx: *mut Context) {
    let syscall_num = unsafe { (*ctx).x[8] };
    let syscall_arg0 = unsafe { (*ctx).x[0] };

    match syscall_num {
        SYSCALL_EXIT => {
            unsafe { super::trap::kill_task(ctx) };
        }
        SYSCALL_SLEEP => {
            unsafe { syscall_sleep(ctx, syscall_arg0) };
        }
        SYSCALL_FORK => {
            unsafe { syscall_fork(ctx) };
        }
        _ => {}
    }
}
