use crate::{PAGE_SIZE, buddy_allocator::BUDDY_ALLOCATOR, page::PageManagement};
use alloc::{
    boxed::Box,
    {vec, vec::Vec},
};
use core::mem::MaybeUninit;
use elf::{Elf, ElfError, PFlags, PType};

#[cfg(target_arch = "aarch64")]
use crate::arch::arm64::cpu::Context;
#[cfg(target_arch = "riscv64")]
use crate::arch::riscv64::cpu::Context;

#[cfg(target_arch = "aarch64")]
use crate::arch::arm64::page::PageManager;
#[cfg(target_arch = "riscv64")]
use crate::arch::riscv64::page::PageManager;

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
    pub context: Context,
}

unsafe impl Sync for Task {}

impl Task {
    pub fn create_from_elf(elf_bytes: &[u8]) -> Result<Self, ElfError> {
        let elf = Elf::parse(elf_bytes)?;
        let scheduler = unsafe { (*(&raw mut SCHEDULER)).assume_init_mut() };
        let mut page = unsafe { PageManager::new() };
        unsafe { page.map_kernel_region() };

        for prog in &elf.p_headers {
            if let PType::Load = prog.p_type {
                let v_page = prog.v_addr / PAGE_SIZE;
                let v_pages = prog.v_addr.div_ceil(PAGE_SIZE);

                let p_page = unsafe { (*(&raw mut BUDDY_ALLOCATOR)).alloc_pages(v_pages) };
                if prog.p_flags.contains(&PFlags::Exec) {
                    unsafe { page.map_text(v_page, p_page, v_pages) };
                } else if prog.p_flags.contains(&PFlags::Write) {
                    unsafe { page.map_data(v_page, p_page, v_pages) };
                } else {
                    unsafe { page.map_rodata(v_page, p_page, v_pages) };
                }

                let p_off = prog.v_addr % PAGE_SIZE; // offset to start of the page
                unsafe {
                    core::ptr::copy(
                        elf_bytes[prog.p_offset..].as_ptr(),
                        (p_page * PAGE_SIZE + p_off) as *mut u8,
                        prog.p_filesz,
                    )
                };
            }
        }

        Ok(Self {
            pid: scheduler.max_pid + 1,
            ppid: scheduler.tasks[scheduler.current_tsk_idx].pid,
            page: Box::new(page),
            nice: scheduler.tasks[scheduler.current_tsk_idx].nice,
            context: Context::default(),
        })
    }
    pub fn renice(&mut self, nice: isize) {
        if (NICE_MIN..=NICE_MAX).contains(&nice) {
            self.nice = nice;
        }
    }
}

pub unsafe fn task_init() {
    let mut kernel_page = unsafe { Box::new(PageManager::new()) };
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
        context: Context::default(),
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
            context: Context::default(),
        };
        scheduler.tasks.push(new_task);
        scheduler.max_pid += 1;
    }
}
