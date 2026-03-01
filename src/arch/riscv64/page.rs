use alloc::alloc::{Layout, alloc_zeroed, dealloc};
use core::arch::asm;

use crate::{
    PAGE_SIZE,
    page::{PageACL, PageManagement},
};

pub const MODE_SV39: u64 = 8;

const PTES_PER_DIR: usize = 512;

/**
 * Set RV64 SATP register.
 *
 * Args:
 * * `ppn`: Pysical Page Number.
 * * `mode`: Mode from 60 to 63 bits.
 */
pub unsafe fn set_satp(mut ppn: u64, mode: u64) {
    ppn |= mode << 60;
    unsafe {
        asm!("csrw satp, {}", in(reg) ppn);
        asm!("sfence.vma");
    }
}

pub unsafe fn get_satp() -> u64 {
    let mut satp;
    unsafe { asm!("csrr {}, satp", out(reg) satp) };

    satp
}

pub const PTE_V_FLAG: u64 = 1;
pub const PTE_R_FLAG: u64 = 1 << 1;
pub const PTE_W_FLAG: u64 = 1 << 2;
pub const PTE_X_FLAG: u64 = 1 << 3;
pub const PTE_U_FLAG: u64 = 1 << 4;

#[derive(Default, Clone, Copy)]
pub struct PageTableEntry {
    pub r: bool,
    pub w: bool,
    pub x: bool,
    pub u: bool,
    pub ppn: u64,
}

impl From<u64> for PageTableEntry {
    fn from(pte_u64: u64) -> Self {
        let mut pte = Self::default();

        if pte_u64 & PTE_R_FLAG != 0 {
            pte.r = true;
        }

        if pte_u64 & PTE_W_FLAG != 0 {
            pte.w = true;
        }

        if pte_u64 & PTE_X_FLAG != 0 {
            pte.x = true;
        }

        if pte_u64 & PTE_U_FLAG != 0 {
            pte.u = true;
        }

        pte.ppn = pte_u64 >> 10;

        pte
    }
}

impl From<PageTableEntry> for u64 {
    fn from(pte: PageTableEntry) -> Self {
        let mut pte_u64 = 0;

        pte_u64 |= PTE_V_FLAG;

        if pte.r {
            pte_u64 |= PTE_R_FLAG;
        }

        if pte.w {
            pte_u64 |= PTE_W_FLAG;
        }

        if pte.x {
            pte_u64 |= PTE_X_FLAG;
        }

        if pte.u {
            pte_u64 |= PTE_U_FLAG;
        }

        pte_u64 |= pte.ppn << 10;

        pte_u64
    }
}

pub struct PageDirectory {
    pub ptes: *mut u64,
}

impl PageDirectory {
    fn from_ppn(ppn: u64) -> Self {
        Self {
            ptes: (ppn << 12) as *mut u64,
        }
    }
    pub fn set_pte(&mut self, count: usize, pte: PageTableEntry) {
        unsafe { self.ptes.add(count).write(pte.into()) };
    }
    /** check if a page directory contains any PTE */
    fn is_empty(&self) -> bool {
        for i in 0..PTES_PER_DIR {
            if unsafe { self.ptes.add(i).read_volatile() } != 0 {
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
            let root_pdir = Self::alloc_page_dir();
            Self {
                root: PageDirectory::from_ppn(root_pdir),
            }
        }
    }
    pub unsafe fn from_satp() -> Self {
        unsafe {
            let addr = get_satp() & 0xfffffffffff;
            Self {
                root: PageDirectory::from_ppn(addr),
            }
        }
    }
    /** Allocate a page directory.
     *
     * Return: Pysical Page Number
     */
    pub unsafe fn alloc_page_dir() -> u64 {
        unsafe { alloc_zeroed(Layout::new::<[u8; PAGE_SIZE]>()) as u64 >> 12 }
    }
    /** Release a page directory. */
    pub unsafe fn release_page_dir(ppn: u64) {
        unsafe { dealloc((ppn << 12) as *mut u8, Layout::new::<[u8; PAGE_SIZE]>()) };
    }
    pub fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
}

impl PageManagement for PageManager {
    unsafe fn map(&mut self, vpn: usize, ppn: usize, mode: &[PageACL]) {
        /* convert ACLs list into riscv PTE mode field bits */
        let mut mode_u64 = 0;
        for i in mode {
            match i {
                PageACL::Read => mode_u64 |= PTE_R_FLAG,
                PageACL::Write => mode_u64 |= PTE_W_FLAG,
                PageACL::Execute => mode_u64 |= PTE_X_FLAG,
            }
        }

        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;
        let v3 = vpn & 0x1ff;

        let v1_pte = unsafe { self.root.ptes.add(v1).read_volatile() };
        let v1_pte = if v1_pte == 0 {
            /* v1 PTE is empty */
            let ppn = unsafe { Self::alloc_page_dir() };
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            self.root.set_pte(v1, pte);

            pte
        } else {
            v1_pte.into()
        };

        let mut v2_pdir = PageDirectory::from_ppn(v1_pte.ppn);
        let v2_pte = unsafe { v2_pdir.ptes.add(v2).read_volatile() };
        let v2_pte = if v2_pte == 0 {
            /* v2 PTE is empty */
            let ppn = unsafe { Self::alloc_page_dir() };
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            v2_pdir.set_pte(v2, pte);

            pte
        } else {
            v2_pte.into()
        };

        let mut v3_pdir = PageDirectory::from_ppn(v2_pte.ppn);
        let v3_pte = (((ppn as u64) << 10) | mode_u64).into();
        v3_pdir.set_pte(v3, v3_pte);
    }
    unsafe fn unmap(&mut self, vpn: usize) {
        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;
        let v3 = vpn & 0x1ff;

        let v1_pte: PageTableEntry = unsafe { self.root.ptes.add(v1).read_volatile().into() };
        let v2_pdir = PageDirectory::from_ppn(v1_pte.ppn);
        let v2_pte: PageTableEntry = unsafe { v2_pdir.ptes.add(v2).read_volatile().into() };
        let v3_pdir = PageDirectory::from_ppn(v2_pte.ppn);
        unsafe { v3_pdir.ptes.add(v3).write_volatile(0) };
        if v3_pdir.is_empty() {
            unsafe {
                Self::release_page_dir(v2_pte.ppn);
            }
            unsafe { v2_pdir.ptes.add(v2).write_volatile(0) };
        }

        if v2_pdir.is_empty() {
            unsafe {
                Self::release_page_dir(v1_pte.ppn);
            }
            unsafe { self.root.ptes.add(v1).write_volatile(0) };
        }
    }
    unsafe fn switch_to(&self) {
        unsafe {
            set_satp(self.root_ppn(), MODE_SV39);
        }
    }
}
