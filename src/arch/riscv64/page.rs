/*!
 * SV39 paging implementaion.
 */

use crate::page::{PAGE_SIZE, PageACL, PageAllocator, Paging, ppn_to_vpn, vpn_to_ppn};
use alloc::vec::Vec;
use core::arch::asm;

pub const MODE_SV39: u64 = 8;

const PTES_PER_DIR: usize = 512;

const MIB: usize = 1024 * 1024;

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

impl PageTableEntry {
    fn mode(&self) -> Vec<PageACL> {
        let mut mode = Vec::new();

        if self.r() {
            mode.push(PageACL::Read);
        }
        if self.w() {
            mode.push(PageACL::Write);
        }
        if self.x() {
            mode.push(PageACL::Execute);
        }
        if self.u() {
            mode.push(PageACL::User);
        }
        mode
    }
    fn r(&self) -> bool {
        self.0 & PTE_R_FLAG != 0
    }
    fn w(&self) -> bool {
        self.0 & PTE_W_FLAG != 0
    }
    fn x(&self) -> bool {
        self.0 & PTE_X_FLAG != 0
    }
    fn v(&self) -> bool {
        self.0 & PTE_V_FLAG != 0
    }
    fn u(&self) -> bool {
        self.0 & PTE_U_FLAG != 0
    }
    fn ppn(&self) -> u64 {
        self.0 >> 10
    }
    fn is_leaf(&self) -> bool {
        self.r() || self.w() || self.x()
    }
}

pub struct PageTable {
    pub ptes: *mut PageTableEntry,
}

impl PageTable {
    fn from_pn(page_num: u64) -> Self {
        Self {
            ptes: (page_num << 12) as *mut PageTableEntry,
        }
    }
    pub fn set_pte(&self, index: usize, pte: PageTableEntry) {
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
    /** Get an entry and ensure the next level of table is not empty */
    unsafe fn get_not_empty<A>(&self, alloc: &mut A, index: usize) -> PageTableEntry
    where
        A: PageAllocator,
    {
        let pte = unsafe { self.ptes.add(index).read_volatile() };
        if pte.0 == 0 {
            /* PTE is empty */
            let ppn = vpn_to_ppn(alloc.alloc_pages(1)) as u64;
            let pte = PageTableEntry(ppn << 10 | PTE_V_FLAG);
            self.set_pte(index, pte);

            pte
        } else {
            pte
        }
    }
}

pub struct PageMapper {
    pub root: PageTable,
}

impl PageMapper {
    pub fn from_pn(root_pt: u64) -> Self {
        Self {
            root: PageTable::from_pn(root_pt),
        }
    }
    pub fn root_ppn(&self) -> u64 {
        vpn_to_ppn(self.root.ptes as usize >> 12) as u64
    }
    unsafe fn map_4k<A>(&mut self, alloc: &mut A, vpn: usize, ppn: usize, mode: u64)
    where
        A: PageAllocator,
    {
        let l2 = (vpn >> 18) & 0x1ff;
        let l1 = (vpn >> 9) & 0x1ff;
        let l0 = vpn & 0x1ff;

        let l2_pte = unsafe { self.root.get_not_empty(alloc, l2) };

        let l1_pt = PageTable::from_pn(ppn_to_vpn(l2_pte.ppn() as usize) as u64);
        let l1_pte = unsafe { l1_pt.get_not_empty(alloc, l1) };

        let l0_pt = PageTable::from_pn(ppn_to_vpn(l1_pte.ppn() as usize) as u64);
        let l0_pte = PageTableEntry((ppn as u64) << 10 | mode);
        l0_pt.set_pte(l0, l0_pte);
    }
    unsafe fn map_2m<A>(&mut self, alloc: &mut A, vpn: usize, ppn: usize, mode: u64)
    where
        A: PageAllocator,
    {
        let l2 = (vpn >> 18) & 0x1ff;
        let l1 = (vpn >> 9) & 0x1ff;

        let l2_pte = unsafe { self.root.get_not_empty(alloc, l2) };

        let l1_pt = PageTable::from_pn(ppn_to_vpn(l2_pte.ppn() as usize) as u64);
        /* release l0 page table if exists. */
        let l1_pte = unsafe { l1_pt.ptes.add(l1).read_volatile() };
        if !l1_pte.is_leaf() && l1_pte.ppn() > 0 {
            alloc.free_pages(l1_pte.ppn() as usize, 1);
        }
        let l1_pte = PageTableEntry((ppn as u64) << 10 | mode);
        l1_pt.set_pte(l1, l1_pte);
    }
    unsafe fn unmap_2m<A>(&mut self, alloc: &mut A, vpn: usize, pages: usize) -> usize
    where
        A: PageAllocator,
    {
        let l2 = vpn >> 18;
        let l1 = (vpn >> 9) & 0x1ff;
        let l0 = vpn & 0x1ff;

        let l2_pte: PageTableEntry = unsafe { self.root.ptes.add(l2).read_volatile() };
        let l1_pt = PageTable::from_pn(ppn_to_vpn(l2_pte.ppn() as usize) as u64);
        let l1_pte: PageTableEntry = unsafe { l1_pt.ptes.add(l1).read_volatile() };

        unsafe { l1_pt.ptes.add(l0).write_volatile(PageTableEntry(0)) };

        /*
         * l0 = vpn - 2mb page start
         */

        /* remap 4kb pages before `vpn` if vpn is not the start of the 2mb page */
        unsafe { self.map(vpn - l0, l1_pte.ppn() as usize, l0, &l1_pte.mode()) };

        if pages < PTES_PER_DIR - l0 {
            /* remap 4kb pages after `vpn` + `pages` */
            unsafe {
                self.map(
                    vpn + pages,
                    l1_pte.ppn() as usize + l0 + pages,
                    PTES_PER_DIR - l0 - pages,
                    &l1_pte.mode(),
                );
            }
            pages
        } else {
            if l1_pt.is_empty() {
                unsafe {
                    alloc.free_pages(ppn_to_vpn(l2_pte.ppn() as usize), 1);
                    self.root.ptes.add(l2).write_volatile(PageTableEntry(0));
                }
            }
            PTES_PER_DIR - l0
        }
    }
}

impl Paging for PageMapper {
    unsafe fn new_with_allocator<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator,
    {
        let root_pt = alloc.alloc_pages(1) as u64;
        Self {
            root: PageTable::from_pn(root_pt),
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
        /* convert ACLs list into riscv PTE mode field bits */
        let mut mode_u64 = 0;
        for m in mode {
            match m {
                PageACL::Read => mode_u64 |= PTE_R_FLAG,
                PageACL::Write => mode_u64 |= PTE_W_FLAG,
                PageACL::Execute => mode_u64 |= PTE_X_FLAG,
                PageACL::User => mode_u64 |= PTE_U_FLAG,
            }
        }
        mode_u64 |= PTE_V_FLAG;

        while pages > 0 {
            if vpn.is_multiple_of(2 * MIB / PAGE_SIZE)
                && ppn.is_multiple_of(2 * MIB / PAGE_SIZE)
                && pages >= 2 * MIB / PAGE_SIZE
            {
                unsafe { self.map_2m(alloc, vpn, ppn, mode_u64) };
                vpn += 2 * MIB / PAGE_SIZE;
                ppn += 2 * MIB / PAGE_SIZE;
                pages -= 2 * MIB / PAGE_SIZE;
            } else {
                unsafe { self.map_4k(alloc, vpn, ppn, mode_u64) };
                vpn += 1;
                ppn += 1;
                pages -= 1;
            }
        }
    }
    unsafe fn unmap_with_allocator<A>(&mut self, alloc: &mut A, mut vpn: usize, mut pages: usize)
    where
        A: PageAllocator,
    {
        while pages > 0 {
            let l2 = vpn >> 18;
            let l1 = (vpn >> 9) & 0x1ff;
            let l0 = vpn & 0x1ff;

            let l2_pte: PageTableEntry = unsafe { self.root.ptes.add(l2).read_volatile() };
            let l1_pt = PageTable::from_pn(ppn_to_vpn(l2_pte.ppn() as usize) as u64);
            let l1_pte: PageTableEntry = unsafe { l1_pt.ptes.add(l1).read_volatile() };
            /* 2MiB huge page */
            if !l1_pte.is_leaf() {
                let ummap_pages = unsafe { self.unmap_2m(alloc, vpn, pages) };
                vpn += ummap_pages;
                pages -= ummap_pages;
                continue;
            }

            let l0_pt = PageTable::from_pn(ppn_to_vpn(l1_pte.ppn() as usize) as u64);
            unsafe { l0_pt.ptes.add(l0).write_volatile(PageTableEntry(0)) };
            if l0_pt.is_empty() {
                unsafe {
                    alloc.free_pages(ppn_to_vpn(l1_pte.ppn() as usize), 1);
                    l1_pt.ptes.add(l1).write_volatile(PageTableEntry(0));
                }
            }

            if l1_pt.is_empty() {
                unsafe {
                    alloc.free_pages(ppn_to_vpn(l2_pte.ppn() as usize), 1);
                    self.root.ptes.add(l2).write_volatile(PageTableEntry(0));
                }
            }
            vpn += 1;
            pages -= 1;
        }
    }
    unsafe fn switch_to(&self) {
        unsafe { set_satp(self.root_ppn(), MODE_SV39) };
    }
    unsafe fn refresh(&self) {
        unsafe { asm!("sfence.vma") };
    }
    unsafe fn destroy_with_allocator<A>(&mut self, alloc: &mut A)
    where
        A: PageAllocator,
    {
        for l2 in 0..PTES_PER_DIR {
            let l2_pte: PageTableEntry = unsafe { self.root.ptes.add(l2).read_volatile() };
            if !l2_pte.v() {
                continue;
            }
            let l1_pt = PageTable::from_pn(ppn_to_vpn(l2_pte.ppn() as usize) as u64);

            for l1 in 0..PTES_PER_DIR {
                let l1_pte: PageTableEntry = unsafe { l1_pt.ptes.add(l1).read_volatile() };
                /* point to l0 table */
                if !l1_pte.is_leaf() && l1_pte.v() {
                    alloc.free_pages(ppn_to_vpn(l1_pte.ppn() as usize), 1);
                }
            }
            alloc.free_pages(ppn_to_vpn(l2_pte.ppn() as usize), 1);
        }
        alloc.free_pages(ppn_to_vpn(self.root_ppn() as usize), 1);
    }
}
