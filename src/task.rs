/*!
 * Multi-tasking structures, including task scheduler, task structure.
 */

use crate::{alloc_pages, free_pages};
use crate::{
    arch::{Context, PageMapper},
    buddy_allocator::ceil_to_power_2,
    global::GlobalUninit,
    mutex::Mutex,
    page::{KERNEL_PT, PAGE_SIZE, Paging, ppn_to_vpn, vpn_to_ppn},
    vfs::VfsFile,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::mem::MaybeUninit;
use elf::{Elf, ElfError, PFlags, PType};

pub static SCHEDULER: GlobalUninit<Scheduler<PageMapper>> = Mutex::new(MaybeUninit::uninit());

const USER_STACK_PAGES: usize = 16;

#[derive(Default)]
pub struct Scheduler<P>
where
    P: Paging + Send,
{
    pub tasks: BTreeMap<usize, Task<P>>,
    /** (vruntime, pid) */
    pub vruntime: BTreeSet<(usize, usize)>,
    current_pid: usize,
    max_pid: usize,
    trap_stack: usize,
}

impl<P> Scheduler<P>
where
    P: Paging + Send,
{
    pub fn create_from_elf(&mut self, elf_bytes: &[u8]) -> Result<usize, ElfError> {
        let elf = Elf::parse(elf_bytes)?;

        let mut page = unsafe { P::new() };
        let stack = unsafe { alloc_pages!(USER_STACK_PAGES) };
        unsafe {
            page.map_kernel_region();
            page.map_data(self.trap_stack, vpn_to_ppn(self.trap_stack), 16);
            page.map_data_u(stack, vpn_to_ppn(stack), USER_STACK_PAGES);
        }
        let mut page_allocs = Vec::new();
        page_allocs.push(Arc::new((
            stack,
            vpn_to_ppn(stack),
            USER_STACK_PAGES,
            alloc::vec![PFlags::Read, PFlags::Write],
        )));

        for prog in &elf.p_headers {
            if let PType::Load = prog.p_type {
                let v_page = prog.v_addr / PAGE_SIZE;
                let v_pages = prog.p_memsz.div_ceil(PAGE_SIZE);

                let p_page = unsafe { vpn_to_ppn(alloc_pages!(ceil_to_power_2(v_pages))) };
                page_allocs.push(Arc::new((v_page, p_page, v_pages, prog.p_flags.to_vec())));
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
                        (ppn_to_vpn(p_page) * PAGE_SIZE + p_off) as *mut u8,
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
            context.x[2] = ((stack + USER_STACK_PAGES) * PAGE_SIZE) as u64; // sp
        }
        #[cfg(target_arch = "aarch64")]
        {
            context.elr_el1 = elf.e_entry as u64;
            context.sp = ((stack + USER_STACK_PAGES) * PAGE_SIZE) as u64;
        }
        #[cfg(target_arch = "x86_64")]
        {
            context.rsp = ((stack + USER_STACK_PAGES) * PAGE_SIZE) as u64;
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
            page_allocs,
            next_schedule: None,
            fds: FdTable::default(),
        };
        self.tasks.insert(pid, task);
        let (min_vruntime, _) = self.vruntime.first().unwrap();
        self.vruntime.insert((*min_vruntime, pid));

        Ok(pid)
    }
    pub fn current_task(&self) -> &Task<P> {
        self.tasks.get(&self.current_pid).unwrap()
    }
    pub fn current_task_mut(&mut self) -> &mut Task<P> {
        self.tasks.get_mut(&self.current_pid).unwrap()
    }
    /**
     * Do task schedule, and return the next task.
     */
    pub fn schedule(&mut self) -> &Task<P> {
        loop {
            let (mut vruntime, pid) = self.vruntime.pop_first().unwrap();
            let task = self.tasks.get(&pid).unwrap();
            vruntime += (task.nice + NICE_MAX) as usize; // higher nice -> larger vruntime
            self.vruntime.insert((vruntime, pid));
            self.current_pid = pid;

            match self.current_task().next_schedule {
                Some(next_schedule) => {
                    if next_schedule <= crate::time::get_sys_time() {
                        self.current_task_mut().next_schedule = None;
                        break;
                    }
                }
                None => break,
            }
        }

        self.current_task()
    }
    pub fn kill(&mut self, pid: usize) {
        self.tasks.remove(&pid);
        self.vruntime.retain(|(_, this_pid)| *this_pid != pid);
        self.schedule();
    }
    /**
     * Schedule, store context of current task, and set the context for the next task,
     * and return the next task.
     */
    pub fn switch_task(&mut self, ctx: *mut Context) -> &Task<P> {
        self.current_task_mut().context = unsafe { ctx.read() };
        let next_task = self.schedule();
        let next_ctx = next_task.context.clone();
        unsafe { ctx.write(next_ctx) };

        next_task
    }
    /** Fork current task. */
    pub fn fork(&mut self) -> usize {
        self.max_pid += 1;
        let pid = self.max_pid;

        let mut page = unsafe { P::new() };
        unsafe {
            page.map_kernel_region();
            page.map_data(self.trap_stack, vpn_to_ppn(self.trap_stack), 16);
        }
        let mut page_allocs = Vec::new();

        for alloc in &self.current_task().page_allocs {
            let (v_page, p_page, v_pages, flags) = alloc.as_ref();

            if flags.contains(&PFlags::Exec) {
                unsafe { page.map_text_u(*v_page, *p_page, *v_pages) };
                page_allocs.push(Arc::clone(alloc));
            } else if flags.contains(&PFlags::Write) {
                let p_page = unsafe { vpn_to_ppn(alloc_pages!(ceil_to_power_2(*v_pages))) };
                page_allocs.push(Arc::new((*v_page, p_page, *v_pages, flags.clone())));
                unsafe {
                    page.map_data_u(*v_page, p_page, *v_pages);
                    core::ptr::copy(
                        (*v_page * PAGE_SIZE) as *const u8,
                        (ppn_to_vpn(p_page) * PAGE_SIZE) as *mut u8,
                        *v_pages * PAGE_SIZE,
                    );
                }
            } else {
                unsafe { page.map_rodata_u(*v_page, *p_page, *v_pages) };
                page_allocs.push(Arc::clone(alloc));
            }
        }
        let child = Task {
            uid: self.current_task().uid,
            pid,
            ppid: self.current_task().pid,
            page,
            nice: self.current_task().nice,
            context: self.current_task().context.clone(),
            page_allocs,
            next_schedule: None,
            fds: FdTable::default(),
        };
        self.tasks.insert(pid, child);
        let (min_vruntime, _) = self.vruntime.first().unwrap();
        self.vruntime.insert((*min_vruntime, pid));

        pid
    }
}

#[derive(Default)]
pub struct FdTable {
    max_fd: usize,
    fds: BTreeMap<usize, VfsFile>,
}

impl FdTable {
    pub fn add(&mut self, file: VfsFile) -> usize {
        let fd = self.max_fd;
        self.fds.insert(fd, file);
        self.max_fd += 1;
        fd
    }
    pub fn get(&self, fd: usize) -> Option<&VfsFile> {
        self.fds.get(&fd)
    }
    pub fn get_mut(&mut self, fd: usize) -> Option<&mut VfsFile> {
        self.fds.get_mut(&fd)
    }
    pub fn remove(&mut self, fd: usize) {
        self.fds.remove(&fd);
    }
}

pub const KERNEL_PID: usize = 0;
const NICE_DEFAULT: isize = 0;
const NICE_MAX: isize = 19;
const NICE_MIN: isize = -20;

type PageAllocInfo = Arc<(usize, usize, usize, Vec<PFlags>)>; // (v_page, p_page, v_pages, flags)

pub struct Task<P>
where
    P: Paging + Send,
{
    pub uid: usize,
    pub pid: usize,
    pub ppid: usize,
    pub page: P,
    pub nice: isize,
    pub context: Context,
    /** Track pages allocations */
    page_allocs: Vec<PageAllocInfo>,
    /** Minimum timestamp for next schedule, set by `sleep` syscall */
    pub next_schedule: Option<u64>,
    pub fds: FdTable,
}

unsafe impl<P> Sync for Task<P> where P: Paging + Send {}

impl<P> Task<P>
where
    P: Paging + Send,
{
    pub fn renice(&mut self, nice: isize) {
        if (NICE_MIN..=NICE_MAX).contains(&nice) {
            self.nice = nice;
        }
    }
    pub fn is_kernel(&self) -> bool {
        self.pid == KERNEL_PID
    }
    /**
     * Returns the length of copied bytes.
     */
    pub fn copy_from_user(&self, user_addr: usize, mut kernel_buf: &mut [u8]) -> usize {
        let buf_size = kernel_buf.len();
        'main: while !kernel_buf.is_empty() {
            for alloc in &self.page_allocs {
                let (vpage, p_page, page_count, _flags) = alloc.as_ref();
                if user_addr >= vpage * PAGE_SIZE && user_addr < (vpage + page_count) * PAGE_SIZE {
                    let mut offset = user_addr - vpage * PAGE_SIZE;
                    while !kernel_buf.is_empty() && offset < page_count * PAGE_SIZE {
                        kernel_buf[0] = unsafe {
                            ((ppn_to_vpn(*p_page) * PAGE_SIZE + offset) as *const u8).read()
                        };
                        kernel_buf = &mut kernel_buf[1..];
                        offset += 1;
                    }
                    if kernel_buf.is_empty() {
                        break 'main;
                    } else {
                        continue 'main;
                    }
                }
            }
            break; // kernel_buf is not full but cannot dump from user space anymore
        }
        buf_size - kernel_buf.len()
    }
    /**
     * Returns the length of copied bytes.
     */
    pub fn copy_to_user(&self, user_addr: usize, mut kernel_buf: &[u8]) -> usize {
        let buf_size = kernel_buf.len();
        'main: while !kernel_buf.is_empty() {
            for alloc in &self.page_allocs {
                let (vpage, p_page, page_count, _flags) = alloc.as_ref();
                if user_addr >= vpage * PAGE_SIZE && user_addr < (vpage + page_count) * PAGE_SIZE {
                    let mut offset = user_addr - vpage * PAGE_SIZE;
                    while !kernel_buf.is_empty() && offset < page_count * PAGE_SIZE {
                        unsafe {
                            ((ppn_to_vpn(*p_page) * PAGE_SIZE + offset) as *mut u8)
                                .write(kernel_buf[0]);
                        };
                        kernel_buf = &kernel_buf[1..];
                        offset += 1;
                    }
                    if kernel_buf.is_empty() {
                        break 'main;
                    } else {
                        continue 'main;
                    }
                }
            }
            break; // kernel_buf is not full but cannot dump from user space anymore
        }
        buf_size - kernel_buf.len()
    }
    pub fn copy_user_string(&self, user_addr: usize) -> String {
        const BUF_SIZE: usize = 16;
        let mut string_vec = Vec::new();
        'main: loop {
            let mut buf = [0; BUF_SIZE];
            let len = self.copy_from_user(user_addr, &mut buf);
            for byte in &buf[..len] {
                if *byte == 0 {
                    break 'main;
                }
                string_vec.push(*byte);
            }
            if len < BUF_SIZE {
                break;
            }
        }
        String::from_utf8_lossy(&string_vec).to_string()
    }
}

impl<P> Drop for Task<P>
where
    P: Paging + Send,
{
    fn drop(&mut self) {
        unsafe { self.page.destroy() };
        for alloc in &mut self.page_allocs {
            let (_vpage, p_page, page_count, _flags) = alloc.as_ref();
            if Arc::strong_count(alloc) == 1 {
                unsafe { free_pages!(ppn_to_vpn(*p_page), ceil_to_power_2(*page_count)) };
            }
        }
    }
}

pub fn task_init() {
    let trap_stack = unsafe { alloc_pages!(16) };
    unsafe {
        crate::trap::trap_stack_init(trap_stack);
    }

    #[cfg(target_arch = "riscv64")]
    let kernel_page = unsafe {
        use crate::page::ppn_to_vpn;
        PageMapper::from_pn(ppn_to_vpn(KERNEL_PT.assume_init()) as u64)
    };
    #[cfg(target_arch = "aarch64")]
    let kernel_page = unsafe {
        use crate::page::pa_to_va;
        PageMapper::from_ttbrx_el1(pa_to_va(KERNEL_PT.assume_init()) as u64)
    };
    let kernel_task = Task {
        page: kernel_page,
        uid: 0,
        pid: KERNEL_PID,
        ppid: 0,
        nice: NICE_DEFAULT,
        context: Context::default(),
        page_allocs: Vec::default(),
        next_schedule: None,
        fds: FdTable::default(),
    };

    let mut tasks = BTreeMap::new();
    tasks.insert(kernel_task.pid, kernel_task);
    let mut vruntime = BTreeSet::new();
    vruntime.insert((0, KERNEL_PID));

    *SCHEDULER.lock() = MaybeUninit::new(Scheduler {
        tasks,
        vruntime,
        current_pid: KERNEL_PID,
        trap_stack,
        max_pid: 0,
    });
}

pub unsafe fn kernel_fork() {
    unsafe {
        let mut scheduler_guard = SCHEDULER.lock();
        let scheduler = scheduler_guard.assume_init_mut();
        let current_task = scheduler.current_task();
        let new_task = Task {
            uid: current_task.uid,
            pid: scheduler.max_pid + 1,
            page: PageMapper::new(),
            ppid: current_task.pid,
            nice: current_task.nice,
            context: Context::default(),
            page_allocs: Vec::default(),
            next_schedule: None,
            fds: FdTable::default(),
        };
        scheduler.tasks.insert(new_task.pid, new_task);
        scheduler.max_pid += 1;
    }
}
