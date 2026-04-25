//! Common code for page management

use core::{mem::MaybeUninit, ptr::addr_of};

pub static mut KERNEL_PT: MaybeUninit<usize> = MaybeUninit::uninit();

pub const PAGE_SIZE: usize = 4096;

#[derive(PartialEq)]
pub enum PageACL {
    Read,
    Write,
    Execute,
    User,
}

macro_rules! map_range {
    ($start:expr, $end:expr, $mgr:expr, $map_fn:ident) => {
        $mgr.$map_fn(
            (addr_of!($start) as usize >> 12),
            (addr_of!($start) as usize >> 12),
            (addr_of!($end) as usize >> 12) - (addr_of!($start) as usize >> 12),
        );
    };
}

#[macro_export]
macro_rules! alloc_pages {
    ($pages_count:expr) => {{
        use $crate::page::PageAllocator;
        let allocator = &mut (*(&raw mut $crate::buddy_allocator::BUDDY_ALLOCATOR));
        allocator.assume_init_mut().alloc_pages($pages_count)
    }};
}

#[macro_export]
macro_rules! free_pages {
    ($pages_start:expr, $pages_count:expr) => {{
        use $crate::page::PageAllocator;
        let allocator = &mut (*(&raw mut $crate::buddy_allocator::BUDDY_ALLOCATOR));
        allocator
            .assume_init_mut()
            .free_pages($pages_start, $pages_count)
    }};
}

pub trait PageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> usize;
    fn free_pages(&mut self, page_start: usize, pages_count: usize);
}

pub trait PageManagement {
    /**
     * Map virtual page into physical page.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `mode`: Page access mode.
     */
    unsafe fn map(&mut self, vpn: usize, ppn: usize, pages: usize, mode: &[PageACL]);
    /**
     * Map as read-only acl
     */
    unsafe fn map_rodata(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read]) };
    }
    /**
     * Map as read-write acl
     */
    unsafe fn map_data(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read, PageACL::Write]) };
    }
    /**
     * Map as read-execute acl
     */
    unsafe fn map_text(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read, PageACL::Execute]) };
    }
    /**
     * Map as read-execute acl, user accessible
     */
    unsafe fn map_text_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe {
            self.map(
                vpn,
                ppn,
                pages,
                &[PageACL::Read, PageACL::Execute, PageACL::User],
            )
        };
    }
    /**
     * Map as read-only acl, user accessible
     */
    unsafe fn map_rodata_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read, PageACL::User]) };
    }
    /**
     * Map as read-write acl, user accessible
     */
    unsafe fn map_data_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe {
            self.map(
                vpn,
                ppn,
                pages,
                &[PageACL::Read, PageACL::Write, PageACL::User],
            )
        };
    }
    /**
     * Unset the map.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     */
    unsafe fn unmap(&mut self, vpn: usize, pages: usize);
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    unsafe fn refresh(&self);
    unsafe fn destroy(&mut self);
    /** map kernel memory into vm */
    unsafe fn map_kernel_region(&mut self) {
        unsafe {
            /* map .rodata */
            map_range!(crate::RODATA_START, crate::RODATA_END, self, map_rodata);
            /* map .data */
            map_range!(crate::DATA_START, crate::DATA_END, self, map_data);
            /* map .bss */
            map_range!(crate::BSS_START, crate::BSS_END, self, map_data);
            /* set kernel code (.text) */
            map_range!(crate::KERNEL_START, crate::KERNEL_END, self, map_text);
        }
    }
}

pub fn kernel_pt_init() {
    #[cfg(target_arch = "aarch64")]
    use crate::arch::arm64::page::PageManager;
    #[cfg(target_arch = "riscv64")]
    use crate::arch::riscv64::page::PageManager;

    let mut kernel_page = unsafe { PageManager::new() };
    unsafe {
        kernel_page.map_kernel_region();
        kernel_page.switch_to();
        kernel_page.refresh();
    }

    #[cfg(target_arch = "riscv64")]
    unsafe {
        KERNEL_PT = MaybeUninit::new(kernel_page.root_ppn() as usize);
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        KERNEL_PT = MaybeUninit::new(kernel_page.ttbrx_el1() as usize);
    }
}
