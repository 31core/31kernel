use super::{cpu::Context, page::refresh_tlb};
use crate::{
    page::Paging,
    syscall::*,
    task::{SCHEDULER, Scheduler},
};
use core::arch::asm;

unsafe fn syscall_fork<P>(scheduler: &mut Scheduler<P>, ctx: *mut Context)
where
    P: Paging + Send,
{
    unsafe {
        scheduler.current_task_mut().context = ctx.read();
    }
    let child_pid = scheduler.fork();
    unsafe {
        (*ctx).x[0] = child_pid as u64;
        scheduler.tasks.get_mut(&child_pid).unwrap().context.x[0] = 0; // child process returns 0
    }
}

pub unsafe fn syscall(ctx: *mut Context) {
    let syscall_num = unsafe { (*ctx).x[8] };
    let syscall_arg0 = unsafe { (*ctx).x[0] };
    let syscall_arg1 = unsafe { (*ctx).x[1] };
    let syscall_arg2 = unsafe { (*ctx).x[2] };
    let syscall_arg3 = unsafe { (*ctx).x[3] };

    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    if !scheduler.current_task().is_kernel() {
        unsafe { asm!("mrs {}, SP_EL0", out(reg)(*ctx).sp) };
    }

    let current_task = scheduler.current_task_mut();
    if let Some(ret) = dispatch_with_task(
        current_task,
        syscall_num,
        syscall_arg0,
        syscall_arg1,
        syscall_arg2,
        syscall_arg3,
    ) {
        unsafe { (*ctx).x[0] = ret };
    }
    match syscall_num {
        SYSCALL_EXIT => unsafe {
            super::trap::kill_task(scheduler, ctx);
            return;
        },
        SYSCALL_FORK => unsafe {
            syscall_fork(scheduler, ctx);
        },
        _ => {}
    }

    let next_task = scheduler.switch_task(ctx);
    if !next_task.is_kernel() {
        unsafe { asm!("msr SP_EL0, {}", in(reg) (*ctx).sp) };
    }

    unsafe {
        next_task.page.switch_to();
        refresh_tlb();
    }
}
