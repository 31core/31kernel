use super::cpu::Context;
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

    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };

    match syscall_num {
        SYSCALL_EXIT => unsafe {
            super::trap::kill_task(scheduler, ctx);
            return;
        },
        SYSCALL_OPEN => unsafe {
            let path = scheduler
                .current_task()
                .copy_user_string(syscall_arg0 as usize);
            let current_task = scheduler.current_task_mut();
            (*ctx).x[9] = syscall_open(current_task, &path) as u64;
        },
        SYSCALL_READ => unsafe {
            let mut buf = alloc::vec![0; syscall_arg2 as usize];
            let current_task = scheduler.current_task_mut();
            (*ctx).x[9] = syscall_read(current_task, syscall_arg0, &mut buf) as u64;
            current_task.copy_to_user(syscall_arg1 as usize, &buf);
        },
        SYSCALL_WRITE => unsafe {
            let mut buf = alloc::vec![0; syscall_arg2 as usize];
            let current_task = scheduler.current_task_mut();
            current_task.copy_from_user(syscall_arg1 as usize, &mut buf);
            (*ctx).x[9] = syscall_write(current_task, syscall_arg0, &buf) as u64;
        },
        SYSCALL_LSEEK => unsafe {
            let current_task = scheduler.current_task_mut();
            (*ctx).x[9] = syscall_lseek(current_task, syscall_arg0, syscall_arg1) as u64;
        },
        SYSCALL_CLOSE => unsafe {
            let mut buf = alloc::vec![0; syscall_arg2 as usize];
            let current_task = scheduler.current_task_mut();
            current_task.copy_from_user(syscall_arg1 as usize, &mut buf);
            (*ctx).x[9] = syscall_close(current_task, syscall_arg0) as u64;
        },
        SYSCALL_SLEEP => unsafe {
            let current_task = scheduler.current_task_mut();
            syscall_sleep(current_task, syscall_arg0);
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
