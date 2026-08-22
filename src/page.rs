//! Common code for page management

pub mod buddy_allocator;
pub mod mapping;
pub mod static_allocator;

use crate::{
    address::{PhysAddr, PhysPage, VirtualAddress},
    page::mapping::Entry,
};
use core::{mem::MaybeUninit, ptr::addr_of};
use mapping::Mapper;

pub static mut KERNEL_PT: MaybeUninit<PhysPage> = MaybeUninit::uninit();

pub const PAGE_BITS: usize = 12;
pub const VA_BITS: usize = 48;
pub const PAGE_SIZE: usize = 1 << PAGE_BITS;
pub const VIRT_ADDR: usize = 0xffffffc040000000;
pub const PHY_ADDR: usize = 0x40000000;

#[derive(PartialEq)]
pub enum PageACL {
    Read,
    Write,
    Execute,
    User,
}

/** Allocate pages using the global buddy allocator */
pub fn alloc_pages(pages_count: usize) -> usize {
    use buddy_allocator::BUDDY_ALLOCATOR;
    let allocator_guard = &mut *BUDDY_ALLOCATOR.lock();
    allocator_guard.alloc_pages(pages_count).unwrap()
}

/** Free pages using the global buddy allocator */
pub fn free_pages(pages_start: usize, pages_count: usize) {
    use buddy_allocator::BUDDY_ALLOCATOR;
    let allocator_guard = &mut *BUDDY_ALLOCATOR.lock();
    allocator_guard.free_pages(pages_start, pages_count);
}

#[derive(Debug)]
pub enum AllocError {
    OutOfMemory,
}

pub trait PageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> Result<usize, AllocError>;
    fn free_pages(&mut self, page_start: usize, pages_count: usize);
}

pub trait Paging<E: Entry>: Sized + Mapper<E> {
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    unsafe fn refresh(&self);
}

pub fn kernel_pt_init() {
    use crate::arch::PageMapper;

    unsafe {
        let alloc = &mut *static_allocator::STATIC_ALLOCATOR.lock();
        let mut kernel_page = PageMapper::new_with_allocator(alloc);

        kernel_page.map_kernel_region_with_allocator(alloc);
        kernel_page.map_with_allocator(
            alloc,
            addr_of!(crate::HEAP_START) as usize >> PAGE_BITS,
            PhysAddr::from(VirtualAddress(addr_of!(crate::HEAP_START) as usize)).0 >> PAGE_BITS,
            crate::MEM_SIZE >> PAGE_BITS,
            &[PageACL::Read, PageACL::Write],
        );
        kernel_page.switch_to();
        kernel_page.refresh();

        KERNEL_PT = MaybeUninit::new(kernel_page.root_table().page_number());
    }
}
