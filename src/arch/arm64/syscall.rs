use super::cpu::Context;
use crate::syscall::*;

pub unsafe fn syscall(ctx: *mut Context) {
    let syscall_num = unsafe { (*ctx).x[8] };

    #[allow(clippy::single_match)]
    match syscall_num {
        SYSCALL_EXIT => {
            unsafe { super::trap::kill_task(ctx) };
        }
        _ => {}
    }
}
