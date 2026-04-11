use crate::{PAGE_SIZE, alloc_pages, page::PageManagement};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
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
    pub tasks: BTreeMap<usize, Task>,
    /** (vruntime, pid) */
    pub vruntime: BTreeSet<(usize, usize)>,
    current_pid: usize,
    max_pid: usize,
    trap_stack: usize,
}

impl Scheduler {
    pub fn create_from_elf(&mut self, elf_bytes: &[u8]) -> Result<usize, ElfError> {
        let elf = Elf::parse(elf_bytes)?;

        let mut page = unsafe { Box::new(PageManager::new()) };
        let stack_start = unsafe { alloc_pages!(16) };
        unsafe {
            page.map_kernel_region();
            page.map_data(self.trap_stack, self.trap_stack, 16);
            page.map_data_u(stack_start, stack_start, 16);
        }

        for prog in &elf.p_headers {
            if let PType::Load = prog.p_type {
                let v_page = prog.v_addr / PAGE_SIZE;
                let v_pages = prog.p_memsz.div_ceil(PAGE_SIZE);

                let p_page = unsafe { alloc_pages!(v_pages) };
                if prog.p_flags.contains(&PFlags::Exec) {
                    unsafe { page.map_text_u(v_page, p_page, v_pages) };
                } else if prog.p_flags.contains(&PFlags::Write) {
                    unsafe { page.map_data_u(v_page, p_page, v_pages) };
                } else {
                    unsafe { page.map_rodata_u(v_page, p_page, v_pages) };
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

        /* initialize context for each architecture */
        let mut context = Context::default();
        #[cfg(target_arch = "riscv64")]
        {
            context.epc = elf.e_entry as u64;
            context.x[2] = ((stack_start + 16) * PAGE_SIZE) as u64; // sp
        }

        self.max_pid += 1;
        let pid = self.max_pid;
        let task = Task {
            uid: self.current_task().uid,
            pid,
            ppid: self.current_task().pid,
            page,
            nice: self.current_task().nice,
            context,
        };
        self.tasks.insert(pid, task);
        self.vruntime.insert((0, pid));

        Ok(pid)
    }
    pub fn current_task(&self) -> &Task {
        self.tasks.get(&self.current_pid).unwrap()
    }
    pub fn current_task_mut(&mut self) -> &mut Task {
        self.tasks.get_mut(&self.current_pid).unwrap()
    }
    pub fn schedule(&mut self) {
        let (mut vruntime, pid) = self.vruntime.pop_first().unwrap();
        let task = self.tasks.get(&pid).unwrap();
        vruntime += (task.nice + NICE_MAX) as usize; // higher nice -> larger vruntime
        self.vruntime.insert((vruntime, pid));
        self.current_pid = pid;
    }
}

pub const KERNEL_PID: usize = 0;
const NICE_DEFAULT: isize = 0;
const NICE_MAX: isize = 19;
const NICE_MIN: isize = -20;

pub struct Task {
    pub uid: u16,
    pub pid: usize,
    pub ppid: usize,
    pub page: Box<dyn PageManagement>,
    pub nice: isize,
    pub context: Context,
}

unsafe impl Sync for Task {}

impl Task {
    pub fn renice(&mut self, nice: isize) {
        if (NICE_MIN..=NICE_MAX).contains(&nice) {
            self.nice = nice;
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        unsafe { self.page.destroy() };
    }
}

pub unsafe fn task_init() {
    let trap_stack = unsafe { alloc_pages!(16) };
    unsafe {
        crate::trap::trap_stack_init(trap_stack);
    }
    let mut kernel_page = unsafe { Box::new(PageManager::new()) };
    unsafe {
        kernel_page.map_kernel_region();
        kernel_page.switch_to();
        kernel_page.refresh();
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        crate::page::KERNEL_PT = MaybeUninit::new(kernel_page.root_ppn() as usize);
    }

    let kernel_task = Task {
        page: kernel_page,
        uid: 0,
        pid: KERNEL_PID,
        ppid: 0,
        nice: NICE_DEFAULT,
        context: Context::default(),
    };

    let mut tasks = BTreeMap::new();
    tasks.insert(kernel_task.pid, kernel_task);
    let mut vruntime = BTreeSet::new();
    vruntime.insert((0, KERNEL_PID));
    unsafe {
        SCHEDULER = MaybeUninit::new(Scheduler {
            tasks,
            vruntime,
            current_pid: KERNEL_PID,
            trap_stack,
            ..Default::default()
        })
    };
}

pub unsafe fn kernel_fork() {
    unsafe {
        let scheduler = (*(&raw mut SCHEDULER)).assume_init_mut();
        let current_task = scheduler.current_task();
        let new_task = Task {
            uid: current_task.uid,
            pid: scheduler.max_pid + 1,
            page: Box::new(PageManager::new()),
            ppid: current_task.pid,
            nice: current_task.nice,
            context: Context::default(),
        };
        scheduler.tasks.insert(new_task.pid, new_task);
        scheduler.max_pid += 1;
    }
}
