/*!
 * SV39 paging implementaion.
 */

use crate::{
    address::{PhysPage, PhysicalPage, VirtPage},
    page::{
        PageACL, PageAllocator, Paging,
        mapping::{Entry, Mapper, Table},
    },
};
use core::arch::asm;

pub const MODE_SV39: u64 = 8;

/**
 * Set RV64 SATP register.
 *
 * Args:
 * * `ppn`: Pysical Page Number.
 * * `mode`: Mode from 60 to 63 bits.
 */
pub unsafe fn set_satp(mut ppn: u64, mode: u64) {
    ppn |= mode << 60;
    unsafe { asm!("csrw satp, {}", in(reg) ppn) };
}

pub const PTE_V_FLAG: u64 = 1;
pub const PTE_R_FLAG: u64 = 1 << 1;
pub const PTE_W_FLAG: u64 = 1 << 2;
pub const PTE_X_FLAG: u64 = 1 << 3;
pub const PTE_U_FLAG: u64 = 1 << 4;

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl Entry for PageTableEntry {
    fn new(page_number: usize, _leaf: bool, mode: &[PageACL]) -> Self {
        let mut entry = (page_number as u64) << 10;
        if mode.contains(&PageACL::Read) {
            entry |= PTE_R_FLAG;
        }
        if mode.contains(&PageACL::Write) {
            entry |= PTE_W_FLAG;
        }
        if mode.contains(&PageACL::Execute) {
            entry |= PTE_X_FLAG;
        }
        if mode.contains(&PageACL::User) {
            entry |= PTE_U_FLAG;
        }
        entry |= PTE_V_FLAG;
        Self(entry)
    }
    fn empty() -> Self {
        Self(0)
    }
    fn is_valid(&self) -> bool {
        self.0 & PTE_V_FLAG > 0
    }
    fn page_number(&self) -> PhysPage {
        PhysicalPage(self.0 as usize >> 10)
    }
}

pub type PageMapper = Sv39Mapper<PageTableEntry>;

pub struct Sv39Mapper<E: Entry> {
    root: Table<E>,
}

impl Mapper<PageTableEntry> for Sv39Mapper<PageTableEntry> {
    const LEVEL: usize = 3;
    const PTES_PER_DIR: usize = 512;
    const PTE_BITS: usize = 9;
    fn new_with_allocator<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator,
    {
        Self {
            root: Table::new(alloc),
        }
    }
    fn from_root(page_number: VirtPage) -> Self {
        Self {
            root: Table::from_page(page_number),
        }
    }
    fn root_table(&self) -> Table<PageTableEntry> {
        self.root
    }
}

unsafe impl Send for Sv39Mapper<PageTableEntry> {}

impl Paging<PageTableEntry> for Sv39Mapper<PageTableEntry> {
    unsafe fn switch_to(&self) {
        unsafe { set_satp(self.root.page_number().0 as u64, MODE_SV39) };
    }
    unsafe fn refresh(&self) {
        unsafe { asm!("sfence.vma") };
    }
}
