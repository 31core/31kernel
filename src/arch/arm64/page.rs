/*!
 * VMSAv8-64 paging implementaion.
 */

use crate::{
    address::{PhysPage, PhysicalPage, VirtPage},
    page::{
        PAGE_BITS, PageACL, PageAllocator, Paging,
        mapping::{Entry, Mapper, Table},
    },
};
use core::arch::asm;

const TYPE_VALID: u64 = 0b01;
const TYPE_BLOCK_ENTRY: u64 = 0b00;
const TYPE_TABLE_ENTRY: u64 = 0b10;
const TYPE_PAGE_ENTRY: u64 = 0b10;

const AP1: u64 = 0b1000000;
const AP2_RO: u64 = 0b10000000;
const AP2_RW: u64 = 0b00000000;

const AF: u64 = 1 << 10;

const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;

unsafe fn mmu_enable() {
    let mut sctlr: u64;
    unsafe {
        asm!("mrs {}, SCTLR_EL1", out(reg) sctlr);
        sctlr |= 1 << 0; // M=1: MMU enable
        sctlr |= 1 << 2; // C=1: D-cache enable
        sctlr |= 1 << 12; // I=1: I-cache enable
        asm!("msr SCTLR_EL1, {}", in(reg) sctlr);
    }
}

pub(super) unsafe fn set_ttbrx(tbbrx_el1: u64) {
    unsafe {
        asm!("msr TTBR0_EL1, {}", in(reg) tbbrx_el1);
        asm!("msr TTBR1_EL1, {}", in(reg) tbbrx_el1);
        asm!("dsb ish");
        asm!("isb");
    }
}

pub(super) unsafe fn refresh_tlb() {
    unsafe {
        asm!("tlbi vmalle1is");
        asm!("dsb ish");
        asm!("isb");
    }
}

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct TableDescriptor(u64);

impl Entry for TableDescriptor {
    fn empty() -> Self {
        Self(0)
    }
    fn is_valid(&self) -> bool {
        self.0 & TYPE_VALID > 0
    }
    fn new(page_number: usize, leaf: bool, mode: &[PageACL]) -> Self {
        let mut descriptor = (page_number as u64) << 12;

        if leaf {
            if mode.contains(&PageACL::User) {
                descriptor |= AP1;
            }
            if mode.contains(&PageACL::Write) {
                descriptor |= AP2_RW;
            } else {
                descriptor |= AP2_RO;
            }
            if !mode.contains(&PageACL::Execute) {
                descriptor |= UXN;
                descriptor |= PXN;
            } else if !mode.contains(&PageACL::User) {
                descriptor |= UXN;
            }
            descriptor |= AF | TYPE_VALID | TYPE_PAGE_ENTRY;
        } else {
            descriptor |= TYPE_VALID | TYPE_TABLE_ENTRY;
        }

        Self(descriptor)
    }
    fn page_number(&self) -> PhysPage {
        PhysicalPage(self.0 as usize >> PAGE_BITS)
    }
}

pub type PageMapper = Arm64Mapper<TableDescriptor>;

pub struct Arm64Mapper<E: Entry> {
    root: Table<E>,
}

unsafe impl Send for Arm64Mapper<TableDescriptor> {}

impl Mapper<TableDescriptor> for Arm64Mapper<TableDescriptor> {
    const LEVEL: usize = 4;
    const PTES_PER_DIR: usize = 512;
    const PTE_BITS: usize = 9;
    fn root_table(&self) -> Table<TableDescriptor> {
        self.root
    }

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
}

impl Paging<TableDescriptor> for Arm64Mapper<TableDescriptor> {
    unsafe fn switch_to(&self) {
        unsafe {
            set_ttbrx((self.root.page_number().0 as u64) << PAGE_BITS);
            mmu_enable();
        }
    }
    unsafe fn refresh(&self) {
        unsafe {
            refresh_tlb();
        }
    }
}
