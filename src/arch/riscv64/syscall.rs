use super::cpu::Context;
use crate::{
    page::Paging,
    syscall::*,
    task::{SCHEDULER, Scheduler},
};
use core::arch::asm;

unsafe fn syscall_fork(scheduler: &mut Scheduler, ctx: *mut Context) {
    unsafe { scheduler.current_task_mut().context = ctx.read() };
    let child_pid = scheduler.fork();
    unsafe {
        (*ctx).x[9] = child_pid as u64;
        scheduler.tasks.get_mut(&child_pid).unwrap().context.x[9] = 0; // child process returns 0
    }
}

pub unsafe fn syscall(ctx: *mut Context) {
    let syscall_num = unsafe { (*ctx).x[16] };
    let syscall_arg0 = unsafe { (*ctx).x[9] };
    let syscall_arg1 = unsafe { (*ctx).x[10] };
    let syscall_arg2 = unsafe { (*ctx).x[11] };
    let syscall_arg3 = unsafe { (*ctx).x[12] };

    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };
    let current_task = scheduler.current_task_mut();
    if let Some(ret) = dispatch_with_task(
        current_task,
        syscall_num,
        syscall_arg0,
        syscall_arg1,
        syscall_arg2,
        syscall_arg3,
    ) {
        unsafe { (*ctx).x[9] = ret };
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
    super::trap::switch_privilege_level(next_task);

    unsafe {
        next_task.page.switch_to();
        asm!("sfence.vma");
    }
}
