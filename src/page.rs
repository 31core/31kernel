//! Common code for page management

use crate::{
    address::{PhysAddr, VirtualAddress},
    buddy_allocator::BUDDY_ALLOCATOR,
};
use core::{mem::MaybeUninit, ptr::addr_of};

pub static mut KERNEL_PT: MaybeUninit<PhysAddr> = MaybeUninit::uninit();

pub const PAGE_BITS: usize = 12;
pub const VA_BITS: usize = 48;
pub const PAGE_SIZE: usize = 1 << PAGE_BITS;
pub const VIRT_ADDR: usize = 0xffffffc040000000;
pub const PHY_ADDR: usize = 0x40000000;

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
            addr_of!($start) as usize >> PAGE_BITS,
            PhysAddr::from(VirtualAddress(addr_of!($start) as usize)).0 >> PAGE_BITS,
            (addr_of!($end) as usize >> PAGE_BITS) - (addr_of!($start) as usize >> PAGE_BITS),
        );
    };
}

macro_rules! map_range_with_alloc {
    ($alloc:ident, $start:expr, $end:expr, $mgr:expr, $mode:expr) => {
        $mgr.map_with_allocator(
            $alloc,
            addr_of!($start) as usize >> PAGE_BITS,
            PhysAddr::from(VirtualAddress(addr_of!($start) as usize)).0 >> PAGE_BITS,
            (addr_of!($end) as usize >> PAGE_BITS) - (addr_of!($start) as usize >> PAGE_BITS),
            $mode,
        );
    };
}

#[macro_export]
macro_rules! alloc_pages {
    ($pages_count:expr) => {{
        use $crate::{buddy_allocator::BUDDY_ALLOCATOR, page::PageAllocator};
        let allocator_guard = &mut *BUDDY_ALLOCATOR.lock();
        allocator_guard.alloc_pages($pages_count).unwrap()
    }};
}

#[macro_export]
macro_rules! free_pages {
    ($pages_start:expr, $pages_count:expr) => {{
        use $crate::{buddy_allocator::BUDDY_ALLOCATOR, page::PageAllocator};
        let allocator_guard = &mut *BUDDY_ALLOCATOR.lock();
        allocator_guard.free_pages($pages_start, $pages_count)
    }};
}

pub trait PageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> Result<usize, AllocError>;
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
            let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
            Self::new_with_allocator(alloc_guard)
        }
    }
    unsafe fn map(&mut self, vpn: usize, ppn: usize, pages: usize, mode: &[PageACL]) {
        unsafe {
            let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
            self.map_with_allocator(alloc_guard, vpn, ppn, pages, mode);
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
            let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
            self.destroy_with_allocator(alloc_guard);
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
            let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
            /* map .rodata */
            map_range_with_alloc!(
                alloc_guard,
                crate::RODATA_START,
                crate::RODATA_END,
                self,
                RO
            );
            /* map .data */
            map_range_with_alloc!(alloc_guard, crate::DATA_START, crate::DATA_END, self, RW);
            /* map .bss */
            map_range_with_alloc!(alloc_guard, crate::BSS_START, crate::BSS_END, self, RW);
            /* set kernel code (.text) */
            map_range_with_alloc!(
                alloc_guard,
                crate::KERNEL_START,
                crate::KERNEL_END,
                self,
                RX
            );
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
            addr_of!(crate::HEAP_START) as usize >> PAGE_BITS,
            PhysAddr::from(VirtualAddress(addr_of!(crate::HEAP_START) as usize)).0 >> PAGE_BITS,
            crate::MEM_SIZE >> PAGE_BITS,
            &[PageACL::Read, PageACL::Write],
        );
        kernel_page.switch_to();
        kernel_page.refresh();

        #[cfg(target_arch = "riscv64")]
        {
            use crate::address::PhysicalAddress;

            KERNEL_PT = MaybeUninit::new(PhysicalAddress(kernel_page.root_ppn().0 << PAGE_BITS));
        }

        #[cfg(target_arch = "aarch64")]
        {
            KERNEL_PT = MaybeUninit::new(kernel_page.ttbrx_el1());
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

#[derive(Debug)]
pub enum AllocError {
    OutOfMemory,
}

impl PageAllocator for StaticPageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> Result<usize, AllocError> {
        assert_eq!(pages_count, 1);
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            let bit = byte.leading_ones();
            if bit < u64::BITS {
                *byte |= 1 << (63 - bit);
                return Ok(self.pages[64 * byte_idx + bit as usize].as_ptr() as usize / PAGE_SIZE);
            }
        }
        Err(AllocError::OutOfMemory)
    }
    fn free_pages(&mut self, page_start: usize, pages_count: usize) {
        assert_eq!(pages_count, 1);
        let byte = page_start / 64;
        let bit = page_start % 64;
        self.bitmap[byte] &= !(1 << (63 - bit));
    }
}
