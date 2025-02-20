use crate::page::PageManagement;
use alloc::{
    boxed::Box,
    {vec, vec::Vec},
};
use core::mem::MaybeUninit;

pub static mut TASKS: MaybeUninit<Vec<Task>> = MaybeUninit::uninit();

const KERNEL_PID: usize = 0;

pub struct Task {
    pub pid: usize,
    pub page: Box<dyn PageManagement>,
}

unsafe impl Sync for Task {}

pub unsafe fn task_init() {
    #[cfg(target_arch = "riscv64")]
    let mut kernel_page = Box::new(crate::arch::riscv64::page::PageManager::new());
    kernel_page.map_kernel_region();
    kernel_page.switch_to();

    let kernel_task = Task {
        page: kernel_page,
        pid: KERNEL_PID,
    };
    TASKS = MaybeUninit::new(vec![kernel_task]);
}

pub unsafe fn kernel_fork() {
    let new_task = Task {
        pid: (*(&raw mut TASKS)).assume_init_mut().len() + 1,
        page: Box::new(crate::arch::riscv64::page::PageManager::new()),
    };
    (*(&raw mut TASKS)).assume_init_mut().push(new_task);
}
