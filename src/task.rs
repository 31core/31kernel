use crate::page::PageManagement;
use alloc::boxed::Box;
use alloc::{vec, vec::Vec};

pub static mut TASKS: Option<Vec<Task>> = None;

const KERNEL_PID: usize = 0;

pub struct Task {
    pub pid: usize,
    pub page: Box<dyn PageManagement>,
}

unsafe impl Sync for Task {}

pub unsafe fn task_init() {
    #[cfg(target_arch = "riscv64")]
    let kernel_page = Box::new(crate::arch::riscv64::page::PageManager::new());
    kernel_page.map_kernel_region();
    kernel_page.switch_to();

    let kernel_task = Task {
        page: kernel_page,
        pid: KERNEL_PID,
    };
    TASKS = Some(vec![kernel_task]);
}

pub unsafe fn kernel_fork() {
    let new_task = Task {
        pid: TASKS.as_ref().unwrap().len() + 1,
        page: Box::new(crate::arch::riscv64::page::PageManager::new()),
    };
    TASKS.as_mut().unwrap().push(new_task);
}
