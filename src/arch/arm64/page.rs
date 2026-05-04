/*!
 * VMSAv8-64 paging implementaion.
 */

use crate::page::{PageACL, PageAllocator, Paging, ppn_to_vpn, va_to_pa, vpn_to_ppn};
use core::arch::asm;

const PTES_PER_DIR: usize = 512;

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

impl TableDescriptor {
    fn ppn(&self) -> u64 {
        self.0 >> 12
    }
    fn is_leaf(&self) -> bool {
        self.0 & TYPE_TABLE_ENTRY == 0
    }
    fn is_valid(&self) -> bool {
        self.0 & TYPE_VALID > 0
    }
}

pub struct PageTable {
    pub ptes: *mut TableDescriptor,
}

impl PageTable {
    fn from_pn(page_num: u64) -> Self {
        Self {
            ptes: (page_num << 12) as *mut TableDescriptor,
        }
    }
    pub fn set_descriptor(&self, index: usize, pte: TableDescriptor) {
        unsafe { self.ptes.add(index).write_volatile(pte) };
    }
    /** check if a page directory contains any PTE */
    fn is_empty(&self) -> bool {
        for i in 0..PTES_PER_DIR {
            if unsafe { self.ptes.add(i).read_volatile().0 } != 0 {
                return false;
            }
        }
        true
    }
    /** Get a descriptor and ensure the next level of table is not empty */
    unsafe fn get_not_empty<A>(&self, alloc: &mut A, index: usize) -> TableDescriptor
    where
        A: PageAllocator,
    {
        let td = unsafe { self.ptes.add(index).read_volatile() };
        if td.0 == 0 {
            /* descriptor is empty */
            let ppn = vpn_to_ppn(alloc.alloc_pages(1)) as u64;
            let td = TableDescriptor(ppn << 12 | TYPE_VALID | TYPE_TABLE_ENTRY);
            self.set_descriptor(index, td);

            td
        } else {
            td
        }
    }
}

pub struct PageMapper {
    pub root: PageTable,
}

impl PageMapper {
    fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
    pub fn from_ttbrx_el1(ttbrx_el1: u64) -> Self {
        Self {
            root: PageTable {
                ptes: ttbrx_el1 as *mut TableDescriptor,
            },
        }
    }
    pub fn ttbrx_el1(&self) -> u64 {
        va_to_pa(self.root.ptes as usize) as u64
    }
    unsafe fn map_4k<A>(&mut self, alloc: &mut A, vpn: usize, ppn: usize, mode: u64)
    where
        A: PageAllocator,
    {
        let l0 = (vpn >> 27) & 0x1ff;
        let l1 = (vpn >> 18) & 0x1ff;
        let l2 = (vpn >> 9) & 0x1ff;
        let l3 = vpn & 0x1ff;

        let l0_td = unsafe { self.root.get_not_empty(alloc, l0) };

        let l1_pt = PageTable::from_pn(ppn_to_vpn(l0_td.ppn() as usize) as u64);
        let l1_td = unsafe { l1_pt.get_not_empty(alloc, l1) };

        let l2_pt = PageTable::from_pn(ppn_to_vpn(l1_td.ppn() as usize) as u64);
        let l2_td = unsafe { l2_pt.get_not_empty(alloc, l2) };

        let l3_pt = PageTable::from_pn(ppn_to_vpn(l2_td.ppn() as usize) as u64);
        l3_pt.set_descriptor(
            l3,
            TableDescriptor((ppn as u64) << 12 | TYPE_VALID | TYPE_PAGE_ENTRY | mode),
        );
    }
    unsafe fn unmap_4k(&mut self, vpn: usize) {
        let l0 = (vpn >> 27) & 0x1ff;
        let l1 = (vpn >> 18) & 0x1ff;
        let l2 = (vpn >> 9) & 0x1ff;
        let l3 = vpn & 0x1ff;

        let l0_td = unsafe { self.root.ptes.add(l0).read_volatile() };

        let l1_pt = PageTable::from_pn(ppn_to_vpn(l0_td.ppn() as usize) as u64);
        let l1_td = unsafe { l1_pt.ptes.add(l1).read_volatile() };

        let l2_pt = PageTable::from_pn(ppn_to_vpn(l1_td.ppn() as usize) as u64);
        let l2_td = unsafe { l2_pt.ptes.add(l2).read_volatile() };

        let l3_pt = PageTable::from_pn(ppn_to_vpn(l2_td.ppn() as usize) as u64);
        l3_pt.set_descriptor(l3, TableDescriptor(0));
    }
}

impl Paging for PageMapper {
    unsafe fn new_with_allocator<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator,
    {
        let root_pdir = alloc.alloc_pages(1) as u64;
        Self {
            root: PageTable::from_pn(root_pdir),
        }
    }
    unsafe fn map_with_allocator<A>(
        &mut self,
        alloc: &mut A,
        mut vpn: usize,
        mut ppn: usize,
        mut pages: usize,
        mode: &[PageACL],
    ) where
        A: PageAllocator,
    {
        let mut mode_u64 = 0;
        if mode.contains(&PageACL::User) {
            mode_u64 |= AP1;
        }
        if mode.contains(&PageACL::Write) {
            mode_u64 |= AP2_RW;
        } else {
            mode_u64 |= AP2_RO;
        }
        if !mode.contains(&PageACL::Execute) {
            mode_u64 |= UXN;
            mode_u64 |= PXN;
        } else if !mode.contains(&PageACL::User) {
            mode_u64 |= UXN;
        }
        mode_u64 |= AF;

        while pages > 0 {
            unsafe { self.map_4k(alloc, vpn, ppn, mode_u64) };
            vpn += 1;
            ppn += 1;
            pages -= 1;
        }
    }
    unsafe fn unmap_with_allocator<A>(&mut self, _alloc: &mut A, mut vpn: usize, mut pages: usize)
    where
        A: PageAllocator,
    {
        while pages > 0 {
            unsafe { self.unmap_4k(vpn) };
            vpn += 1;
            pages -= 1;
        }
    }
    unsafe fn switch_to(&self) {
        unsafe {
            set_ttbrx(self.ttbrx_el1());
            mmu_enable();
        }
    }
    unsafe fn refresh(&self) {
        unsafe {
            refresh_tlb();
        }
    }
    unsafe fn destroy_with_allocator<A>(&mut self, alloc: &mut A)
    where
        A: PageAllocator,
    {
        for l0 in 0..PTES_PER_DIR {
            let l0_td: TableDescriptor = unsafe { self.root.ptes.add(l0).read_volatile() };
            if !l0_td.is_valid() {
                continue;
            }
            let l1_pt = PageTable::from_pn(ppn_to_vpn(l0_td.ppn() as usize) as u64);

            for l1 in 0..PTES_PER_DIR {
                let l1_td: TableDescriptor = unsafe { l1_pt.ptes.add(l1).read_volatile() };
                if !l1_td.is_valid() {
                    continue;
                }
                let l2_pt = PageTable::from_pn(ppn_to_vpn(l1_td.ppn() as usize) as u64);

                for l2 in 0..PTES_PER_DIR {
                    let l2_td: TableDescriptor = unsafe { l2_pt.ptes.add(l2).read_volatile() };
                    /* point to l3 table */
                    if !l2_td.is_leaf() && l2_td.is_valid() {
                        alloc.free_pages(ppn_to_vpn(l2_td.ppn() as usize), 1);
                    }
                }
                alloc.free_pages(ppn_to_vpn(l1_td.ppn() as usize), 1);
            }
            alloc.free_pages(ppn_to_vpn(l0_td.ppn() as usize), 1);
        }
        alloc.free_pages(ppn_to_vpn(self.root_ppn() as usize), 1);
    }
}
