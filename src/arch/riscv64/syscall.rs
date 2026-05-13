use super::cpu::Context;
use crate::{page::Paging, syscall::*, task::SCHEDULER};

unsafe fn syscall_sleep(ctx: *mut Context, timestamp: u64) {
    let mut scheduler_guard = SCHEDULER.lock();
    let scheduler = unsafe { scheduler_guard.assume_init_mut() };

    let next_time = crate::time::get_sys_time() + timestamp;
    scheduler.current_task_mut().next_schedule = Some(next_time);
    let next_task = scheduler.switch_task(ctx);

    unsafe {
        next_task.page.switch_to();
    }
}

pub unsafe fn syscall(ctx: *mut Context) {
    let syscall_num = unsafe { (*ctx).x[16] };
    let syscall_arg0 = unsafe { (*ctx).x[9] };

    match syscall_num {
        SYSCALL_EXIT => {
            unsafe { super::trap::kill_task(ctx) };
        }
        SYSCALL_SLEEP => {
            unsafe { syscall_sleep(ctx, syscall_arg0) };
        }
        _ => {}
    }
}
