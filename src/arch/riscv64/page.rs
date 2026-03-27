use crate::{
    PAGE_SIZE,
    page::{PageACL, PageManagement},
};
use alloc::{
    alloc::{Layout, alloc_zeroed, dealloc},
    vec::Vec,
};
use core::arch::asm;

pub const MODE_SV39: u64 = 8;

const PTES_PER_DIR: usize = 512;

const MIB: usize = 1024 * 1024;

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

pub struct PageDirectory {
    pub ptes: *mut PageTableEntry,
}

impl PageDirectory {
    fn from_ppn(ppn: u64) -> Self {
        Self {
            ptes: (ppn << 12) as *mut PageTableEntry,
        }
    }
    pub fn set_pte(&mut self, index: usize, pte: PageTableEntry) {
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
}

pub struct PageManager {
    pub root: PageDirectory,
}

impl PageManager {
    pub unsafe fn new() -> Self {
        unsafe {
            let root_pdir = alloc_page_dir();
            Self {
                root: PageDirectory::from_ppn(root_pdir),
            }
        }
    }
    pub fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
    unsafe fn map_4k(&mut self, vpn: usize, ppn: usize, mode: u64) {
        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;
        let v3 = vpn & 0x1ff;

        let v1_pte = unsafe { self.root.ptes.add(v1).read_volatile() };
        let v1_pte = if v1_pte.0 == 0 {
            /* v1 PTE is empty */
            let ppn = unsafe { alloc_page_dir() };
            let pte = PageTableEntry(ppn << 10 | PTE_V_FLAG);
            self.root.set_pte(v1, pte);

            pte
        } else {
            v1_pte
        };

        let mut v2_pdir = PageDirectory::from_ppn(v1_pte.ppn());
        let v2_pte = unsafe { v2_pdir.ptes.add(v2).read_volatile() };
        let v2_pte = if v2_pte.0 == 0 {
            /* v2 PTE is empty */
            let ppn = unsafe { alloc_page_dir() };
            let pte = PageTableEntry(ppn << 10 | PTE_V_FLAG);
            v2_pdir.set_pte(v2, pte);

            pte
        } else {
            v2_pte
        };

        let mut v3_pdir = PageDirectory::from_ppn(v2_pte.ppn());
        let v3_pte = PageTableEntry((ppn as u64) << 10 | mode);
        v3_pdir.set_pte(v3, v3_pte);
    }
    unsafe fn map_2m(&mut self, vpn: usize, ppn: usize, mode: u64) {
        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;

        let v1_pte = unsafe { self.root.ptes.add(v1).read_volatile() };
        let v1_pte = if v1_pte.0 == 0 {
            /* v1 PTE is empty */
            let ppn = unsafe { alloc_page_dir() };
            let pte = PageTableEntry(ppn << 10 | PTE_V_FLAG);
            self.root.set_pte(v1, pte);

            pte
        } else {
            v1_pte
        };

        let mut v2_pdir = PageDirectory::from_ppn(v1_pte.ppn());
        let v2_pte = PageTableEntry((ppn as u64 >> 9) << 10 | mode);
        v2_pdir.set_pte(v2, v2_pte);
    }
    unsafe fn unmap_2m(&mut self, vpn: usize, pages: usize) -> usize {
        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;
        let v3 = vpn & 0x1ff;

        let v1_pte: PageTableEntry = unsafe { self.root.ptes.add(v1).read_volatile() };
        let v2_pdir = PageDirectory::from_ppn(v1_pte.ppn());
        let v2_pte: PageTableEntry = unsafe { v2_pdir.ptes.add(v2).read_volatile() };

        unsafe { v2_pdir.ptes.add(v3).write_volatile(PageTableEntry(0)) };

        /*
         * v3 = vpn - 2mb page start
         */

        /* remap 4kb pages before `vpn` if vpn is not the start of the 2mb page */
        unsafe { self.map(vpn - v3, v2_pte.ppn() as usize, v3, &v2_pte.mode()) };

        if pages < PTES_PER_DIR - v3 {
            /* remap 4kb pages after `vpn` + `pages` */
            unsafe {
                self.map(
                    vpn + pages,
                    v2_pte.ppn() as usize + v3 + pages,
                    PTES_PER_DIR - v3 - pages,
                    &v2_pte.mode(),
                );
            }
            pages
        } else {
            if v2_pdir.is_empty() {
                unsafe {
                    release_page_dir(v1_pte.ppn());
                    self.root.ptes.add(v1).write_volatile(PageTableEntry(0));
                }
            }
            PTES_PER_DIR - v3
        }
    }
}

impl PageManagement for PageManager {
    unsafe fn map(&mut self, mut vpn: usize, mut ppn: usize, mut pages: usize, mode: &[PageACL]) {
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
                unsafe { self.map_2m(vpn, ppn, mode_u64) };
                vpn += 2 * MIB / PAGE_SIZE;
                ppn += 2 * MIB / PAGE_SIZE;
                pages -= 2 * MIB / PAGE_SIZE;
            } else {
                unsafe { self.map_4k(vpn, ppn, mode_u64) };
                vpn += 1;
                ppn += 1;
                pages -= 1;
            }
        }
    }
    unsafe fn unmap(&mut self, mut vpn: usize, mut pages: usize) {
        while pages > 0 {
            let v1 = vpn >> 18;
            let v2 = (vpn >> 9) & 0x1ff;
            let v3 = vpn & 0x1ff;

            let v1_pte: PageTableEntry = unsafe { self.root.ptes.add(v1).read_volatile() };
            let v2_pdir = PageDirectory::from_ppn(v1_pte.ppn());
            let v2_pte: PageTableEntry = unsafe { v2_pdir.ptes.add(v2).read_volatile() };
            /* 2MiB huge page */
            if !v2_pte.is_leaf() {
                let ummap_pages = unsafe { self.unmap_2m(vpn, pages) };
                vpn += ummap_pages;
                pages -= ummap_pages;
                continue;
            }

            let v3_pdir = PageDirectory::from_ppn(v2_pte.ppn());
            unsafe { v3_pdir.ptes.add(v3).write_volatile(PageTableEntry(0)) };
            if v3_pdir.is_empty() {
                unsafe {
                    release_page_dir(v2_pte.ppn());
                    v2_pdir.ptes.add(v2).write_volatile(PageTableEntry(0));
                }
            }

            if v2_pdir.is_empty() {
                unsafe {
                    release_page_dir(v1_pte.ppn());
                    self.root.ptes.add(v1).write_volatile(PageTableEntry(0));
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
    unsafe fn destroy(&mut self) {
        for v1 in 0..PTES_PER_DIR {
            let v1_pte: PageTableEntry = unsafe { self.root.ptes.add(v1).read_volatile() };
            let v2_pdir = PageDirectory::from_ppn(v1_pte.ppn());

            for v2 in 0..PTES_PER_DIR {
                let v2_pte: PageTableEntry = unsafe { v2_pdir.ptes.add(v2).read_volatile() };
                /* point to l2 table */
                if !v2_pte.is_leaf() {
                    unsafe { release_page_dir(v2_pte.ppn()) };
                }
            }
            unsafe { release_page_dir(v1_pte.ppn()) };
        }
        unsafe { release_page_dir(self.root_ppn()) };
    }
}
