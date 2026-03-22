use crate::page::PageManagement;
use alloc::{
    boxed::Box,
    {vec, vec::Vec},
};
use core::mem::MaybeUninit;

pub static mut SCHEDULER: MaybeUninit<Scheduler> = MaybeUninit::uninit();

#[derive(Default)]
pub struct Scheduler {
    tasks: Vec<Task>,
    current_tsk_idx: usize,
    max_pid: usize,
}

const KERNEL_PID: usize = 0;
const NICE_DEFAULT: isize = 0;
const NICE_MAX: isize = 19;
const NICE_MIN: isize = -20;

pub struct Task {
    pub pid: usize,
    pub ppid: usize,
    pub page: Box<dyn PageManagement>,
    pub nice: isize,
}

unsafe impl Sync for Task {}

impl Task {
    pub fn renice(&mut self, nice: isize) {
        if (NICE_MIN..=NICE_MAX).contains(&nice) {
            self.nice = nice;
        }
    }
}

pub unsafe fn task_init() {
    #[cfg(target_arch = "riscv64")]
    let mut kernel_page = unsafe { Box::new(crate::arch::riscv64::page::PageManager::new()) };
    unsafe {
        kernel_page.map_kernel_region();
        kernel_page.switch_to();
        kernel_page.refresh();
    }

    let kernel_task = Task {
        page: kernel_page,
        pid: KERNEL_PID,
        ppid: 0,
        nice: NICE_DEFAULT,
    };

    unsafe {
        SCHEDULER = MaybeUninit::new(Scheduler {
            tasks: vec![kernel_task],
            ..Default::default()
        })
    };
}

pub unsafe fn kernel_fork() {
    unsafe {
        let scheduler = (*(&raw mut SCHEDULER)).assume_init_mut();
        let new_task = Task {
            pid: scheduler.max_pid + 1,
            page: Box::new(crate::arch::riscv64::page::PageManager::new()),
            ppid: scheduler.tasks[scheduler.current_tsk_idx].pid,
            nice: scheduler.tasks[scheduler.current_tsk_idx].nice,
        };
        scheduler.tasks.push(new_task);
        scheduler.max_pid += 1;
    }
}
