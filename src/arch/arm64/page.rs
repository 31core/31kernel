/*!
 * VMSAv8-64 paging implementaion.
 */

use crate::{
    PAGE_SIZE,
    page::{PageACL, PageManagement},
};
use alloc::alloc::{Layout, alloc_zeroed, dealloc};
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
        asm!("mrs {}, sctlr_el1", out(reg) sctlr);
        sctlr |= 1 << 0; // M=1: MMU enable
        sctlr |= 1 << 2; // C=1: D-cache enable
        sctlr |= 1 << 12; // I=1: I-cache enable
        asm!("msr sctlr_el1, {}", in(reg) sctlr);
    }
}

/** Allocate a page directory.
 *
 * Return: Pysical Page Number
 */
unsafe fn alloc_page_dir() -> u64 {
    unsafe {
        alloc_zeroed(
            Layout::new::<[u8; PAGE_SIZE]>()
                .align_to(PAGE_SIZE)
                .unwrap(),
        ) as u64
            >> 12
    }
}
/** Release a page directory. */
unsafe fn release_page_dir(ppn: u64) {
    unsafe {
        dealloc(
            (ppn << 12) as *mut u8,
            Layout::new::<[u8; PAGE_SIZE]>()
                .align_to(PAGE_SIZE)
                .unwrap(),
        )
    };
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
    fn from_ppn(ppn: u64) -> Self {
        Self {
            ptes: (ppn << 12) as *mut TableDescriptor,
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
    unsafe fn get_not_empty(&self, index: usize) -> TableDescriptor {
        let td = unsafe { self.ptes.add(index).read_volatile() };
        if td.0 == 0 {
            /* descriptor is empty */
            let ppn = unsafe { alloc_page_dir() };
            let td = TableDescriptor(ppn << 12 | TYPE_VALID | TYPE_TABLE_ENTRY);
            self.set_descriptor(index, td);

            td
        } else {
            td
        }
    }
}

pub struct PageManager {
    pub root: PageTable,
}

impl PageManager {
    pub unsafe fn new() -> Self {
        let root_pdir = unsafe { alloc_page_dir() };
        Self {
            root: PageTable::from_ppn(root_pdir),
        }
    }
    fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
    unsafe fn map_4k(&mut self, vpn: usize, ppn: usize, mode: u64) {
        let l3 = vpn >> 27;
        let l2 = (vpn >> 18) & 0x1ff;
        let l1 = (vpn >> 9) & 0x1ff;
        let l0 = vpn & 0x1ff;

        let l3_td = unsafe { self.root.get_not_empty(l3) };

        let l2_pt = PageTable::from_ppn(l3_td.ppn());
        let l2_td = unsafe { l2_pt.get_not_empty(l2) };

        let l1_pt = PageTable::from_ppn(l2_td.ppn());
        let l1_td = unsafe { l1_pt.get_not_empty(l1) };

        let l0_pt = PageTable::from_ppn(l1_td.ppn());
        l0_pt.set_descriptor(
            l0,
            TableDescriptor((ppn as u64) << 12 | TYPE_VALID | TYPE_PAGE_ENTRY | mode),
        );
    }
    unsafe fn unmap_4k(&mut self, vpn: usize) {
        let l3 = vpn >> 27;
        let l2 = (vpn >> 18) & 0x1ff;
        let l1 = (vpn >> 9) & 0x1ff;
        let l0 = vpn & 0x1ff;

        let l3_td = unsafe { self.root.ptes.add(l3).read_volatile() };

        let l2_pt = PageTable::from_ppn(l3_td.ppn());
        let l2_td = unsafe { l2_pt.ptes.add(l2).read_volatile() };

        let l1_pt = PageTable::from_ppn(l2_td.ppn());
        let l1_td = unsafe { l1_pt.ptes.add(l1).read_volatile() };

        let l0_pt = PageTable::from_ppn(l1_td.ppn());
        l0_pt.set_descriptor(l0, TableDescriptor(0));
    }
}

impl PageManagement for PageManager {
    unsafe fn map(&mut self, mut vpn: usize, mut ppn: usize, mut pages: usize, mode: &[PageACL]) {
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
            if mode.contains(&PageACL::User) {
                mode_u64 |= UXN;
            }
            mode_u64 |= PXN;
        }
        mode_u64 |= AF;

        while pages > 0 {
            unsafe { self.map_4k(vpn, ppn, mode_u64) };
            vpn += 1;
            ppn += 1;
            pages -= 1;
        }
    }
    unsafe fn unmap(&mut self, mut vpn: usize, mut pages: usize) {
        while pages > 0 {
            unsafe { self.unmap_4k(vpn) };
            vpn += 1;
            pages -= 1;
        }
    }
    unsafe fn switch_to(&self) {
        unsafe {
            asm!("msr ttbr0_el1, {}", in(reg) self.root_ppn() << 12);
            asm!("msr ttbr1_el1, {}", in(reg) self.root_ppn() << 12);
            asm!("dsb ish");
            asm!("isb");
            mmu_enable();
        }
    }
    unsafe fn refresh(&self) {
        unsafe {
            asm!("tlbi vmalle1is");
            asm!("dsb ish");
            asm!("isb");
        }
    }
    unsafe fn destroy(&mut self) {
        for l3 in 0..PTES_PER_DIR {
            let l3_td: TableDescriptor = unsafe { self.root.ptes.add(l3).read_volatile() };
            if !l3_td.is_valid() {
                continue;
            }
            let l2_pt = PageTable::from_ppn(l3_td.ppn());

            for l2 in 0..PTES_PER_DIR {
                let l2_td: TableDescriptor = unsafe { l2_pt.ptes.add(l2).read_volatile() };
                if !l2_td.is_valid() {
                    continue;
                }
                let l1_pt = PageTable::from_ppn(l2_td.ppn());

                for l1 in 0..PTES_PER_DIR {
                    let l1_td: TableDescriptor = unsafe { l1_pt.ptes.add(l1).read_volatile() };
                    /* point to l0 table */
                    if !l1_td.is_leaf() && l1_td.is_valid() {
                        unsafe { release_page_dir(l1_td.ppn()) };
                    }
                }
                unsafe { release_page_dir(l2_td.ppn()) };
            }
            unsafe { release_page_dir(l3_td.ppn()) };
        }
        unsafe { release_page_dir(self.root_ppn()) };
    }
}
