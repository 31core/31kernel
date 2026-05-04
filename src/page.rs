//! Common code for page management

use crate::buddy_allocator::BUDDY_ALLOCATOR;
use core::{mem::MaybeUninit, ptr::addr_of};

pub static mut KERNEL_PT: MaybeUninit<usize> = MaybeUninit::uninit();

pub const PAGE_SIZE: usize = 4096;
const VIRT_ADDR: usize = 0xffffffc040000000;
const PHY_ADDR: usize = 0x40000000;

const RO: &[PageACL] = &[PageACL::Read];
const RW: &[PageACL] = &[PageACL::Read, PageACL::Write];
const RX: &[PageACL] = &[PageACL::Read, PageACL::Execute];
const URO: &[PageACL] = &[PageACL::User, PageACL::Read];
const URW: &[PageACL] = &[PageACL::User, PageACL::Read, PageACL::Write];
const URX: &[PageACL] = &[PageACL::User, PageACL::Read, PageACL::Execute];

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
            (addr_of!($start) as usize / PAGE_SIZE),
            vpn_to_ppn(addr_of!($start) as usize / PAGE_SIZE),
            (addr_of!($end) as usize / PAGE_SIZE) - (addr_of!($start) as usize / PAGE_SIZE),
        );
    };
}

macro_rules! map_range_with_alloc {
    ($alloc:ident, $start:expr, $end:expr, $mgr:expr, $mode:expr) => {
        $mgr.map_with_allocator(
            $alloc,
            (addr_of!($start) as usize / PAGE_SIZE),
            vpn_to_ppn(addr_of!($start) as usize / PAGE_SIZE),
            (addr_of!($end) as usize / PAGE_SIZE) - (addr_of!($start) as usize / PAGE_SIZE),
            $mode,
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

/* Virtual page number to physical page number */
pub fn vpn_to_ppn(vpn: usize) -> usize {
    let delta = (VIRT_ADDR - PHY_ADDR) / PAGE_SIZE;
    vpn - delta
}

/* Physical page number to virtual page number */
pub fn ppn_to_vpn(ppn: usize) -> usize {
    let delta = (VIRT_ADDR - PHY_ADDR) / PAGE_SIZE;
    ppn + delta
}

/* Virtual address to physical address */
pub fn va_to_pa(va: usize) -> usize {
    let delta = VIRT_ADDR - PHY_ADDR;
    va - delta
}

/* Physical address to virtual address */
pub fn pa_to_va(pa: usize) -> usize {
    let delta = VIRT_ADDR - PHY_ADDR;
    pa + delta
}

pub trait PageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> usize;
    fn free_pages(&mut self, page_start: usize, pages_count: usize);
}

pub trait Paging: Sized {
    unsafe fn new_with_allocator<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator;
    /**
     * Map virtual page into physical page.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `pages`: Pages count to map.
     * * `mode`: Page access mode.
     */
    unsafe fn new() -> Self {
        unsafe {
            let alloc = (*(&raw mut BUDDY_ALLOCATOR)).assume_init_mut();
            Self::new_with_allocator(alloc)
        }
    }
    unsafe fn map(&mut self, vpn: usize, ppn: usize, pages: usize, mode: &[PageACL]) {
        unsafe {
            let alloc = (*(&raw mut BUDDY_ALLOCATOR)).assume_init_mut();
            self.map_with_allocator(alloc, vpn, ppn, pages, mode);
        }
    }
    /**
     * Map virtual page into physical page with a specified [PageAllocator].
     *
     * Args:
     * * `alloc`: Page allocator.
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `pages`: Pages count to map.
     * * `mode`: Page access mode.
     */
    unsafe fn map_with_allocator<A>(
        &mut self,
        alloc: &mut A,
        vpn: usize,
        ppn: usize,
        pages: usize,
        mode: &[PageACL],
    ) where
        A: PageAllocator;
    /**
     * Map as read-only acl
     */
    unsafe fn map_rodata(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, RO) };
    }
    /**
     * Map as read-write acl
     */
    unsafe fn map_data(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, RW) };
    }
    /**
     * Map as read-execute acl
     */
    unsafe fn map_text(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, RX) };
    }
    /**
     * Map as read-execute acl, user accessible
     */
    unsafe fn map_text_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, URX) };
    }
    /**
     * Map as read-only acl, user accessible
     */
    unsafe fn map_rodata_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, URO) };
    }
    /**
     * Map as read-write acl, user accessible
     */
    unsafe fn map_data_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, URW) };
    }
    /**
     * Unset the map.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     */
    unsafe fn unmap_with_allocator<A>(&mut self, alloc: &mut A, vpn: usize, pages: usize)
    where
        A: PageAllocator;
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    unsafe fn refresh(&self);
    unsafe fn destroy_with_allocator<A>(&mut self, alloc: &mut A)
    where
        A: PageAllocator;
    unsafe fn destroy(&mut self) {
        unsafe {
            let alloc = (*(&raw mut BUDDY_ALLOCATOR)).assume_init_mut();
            self.destroy_with_allocator(alloc);
        }
    }
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
    /** map kernel memory into vm, using a static page allocator */
    unsafe fn map_kernel_region_bootstrap(&mut self) {
        unsafe {
            let alloc = &mut (*(&raw mut STATIC_ALLOCATOR));
            /* map .rodata */
            map_range_with_alloc!(alloc, crate::RODATA_START, crate::RODATA_END, self, RO);
            /* map .data */
            map_range_with_alloc!(alloc, crate::DATA_START, crate::DATA_END, self, RW);
            /* map .bss */
            map_range_with_alloc!(alloc, crate::BSS_START, crate::BSS_END, self, RW);
            /* set kernel code (.text) */
            map_range_with_alloc!(alloc, crate::KERNEL_START, crate::KERNEL_END, self, RX);
        }
    }
}

pub fn kernel_pt_init() {
    use crate::arch::PageMapper;

    unsafe {
        let alloc = &mut (*(&raw mut STATIC_ALLOCATOR));
        let mut kernel_page = PageMapper::new_with_allocator(alloc);

        kernel_page.map_kernel_region_bootstrap();
        kernel_page.map_with_allocator(
            alloc,
            addr_of!(crate::HEAP_START) as usize / PAGE_SIZE,
            vpn_to_ppn(addr_of!(crate::HEAP_START) as usize / PAGE_SIZE),
            crate::MEM_SIZE / PAGE_SIZE,
            &[PageACL::Read, PageACL::Write],
        );
        kernel_page.switch_to();
        kernel_page.refresh();

        #[cfg(target_arch = "riscv64")]
        {
            KERNEL_PT = MaybeUninit::new(kernel_page.root_ppn() as usize);
        }

        #[cfg(target_arch = "aarch64")]
        {
            KERNEL_PT = MaybeUninit::new(kernel_page.ttbrx_el1() as usize);
        }
    }
}

static mut STATIC_ALLOCATOR: StaticPageAllocator = StaticPageAllocator {
    pages: [[0; PAGE_SIZE]; STATIC_PAGE_CAP],
    bitmap: [0; STATIC_PAGE_CAP / 64],
};

const STATIC_PAGE_CAP: usize = 256;
#[repr(C, align(4096))]
struct StaticPageAllocator {
    pages: [[u8; PAGE_SIZE]; STATIC_PAGE_CAP],
    bitmap: [u64; STATIC_PAGE_CAP / 64],
}

impl PageAllocator for StaticPageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> usize {
        assert_eq!(pages_count, 1);
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            if *byte != u64::MAX {
                for bit in 0..64 {
                    if *byte & (1 << (63 - bit)) == 0 {
                        *byte |= 1 << (63 - bit);
                        return self.pages[64 * byte_idx + bit].as_ptr() as usize / PAGE_SIZE;
                    }
                }
            }
        }
        panic!("No enough page to allocate");
    }
    fn free_pages(&mut self, page_start: usize, pages_count: usize) {
        assert_eq!(pages_count, 1);
        let byte = page_start / 64;
        let bit = page_start % 64;
        self.bitmap[byte] &= !(1 << (63 - bit));
    }
}
