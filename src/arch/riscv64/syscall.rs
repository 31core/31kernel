use super::cpu::Context;
use crate::{
    page::Paging,
    syscall::*,
    task::{SCHEDULER, Scheduler},
};
use core::arch::asm;

unsafe fn syscall_sleep<P>(scheduler: &mut Scheduler<P>, timestamp: u64)
where
    P: Paging + Send,
{
    let next_time = crate::time::get_sys_time() + timestamp;
    scheduler.current_task_mut().next_schedule = Some(next_time);
}

unsafe fn syscall_fork<P>(scheduler: &mut Scheduler<P>, ctx: *mut Context)
where
    P: Paging + Send,
{
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

    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };

    match syscall_num {
        SYSCALL_EXIT => unsafe {
            super::trap::kill_task(scheduler, ctx);
            return;
        },
        SYSCALL_SLEEP => {
            unsafe { syscall_sleep(scheduler, syscall_arg0) };
        }
        SYSCALL_FORK => {
            unsafe { syscall_fork(scheduler, ctx) };
        }
        _ => {}
    }

    let next_task = scheduler.switch_task(ctx);
    super::trap::switch_privilege_level(next_task);

    unsafe {
        next_task.page.switch_to();
        asm!("sfence.vma");
    }
}
