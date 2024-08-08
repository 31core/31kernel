use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use core::arch::asm;

use crate::page::{PageACL, PageManagement};

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
    asm!("csrw satp, {}", in(reg) ppn);
    asm!("sfence.vma")
}

pub unsafe fn get_satp() -> u64 {
    let mut satp;
    asm!("csrr {}, satp", out(reg) satp);

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

pub struct PageDtrectory {
    pub ptes: *mut u64,
}

impl PageDtrectory {
    fn from_ppn(ppn: u64) -> Self {
        Self {
            ptes: (ppn << 12) as *mut u64,
        }
    }
    pub unsafe fn set_pte(&self, count: usize, pte: PageTableEntry) {
        self.ptes.add(count).write(pte.into());
    }
    /** check if a page directory contains any PTE */
    fn is_empty(&self) -> bool {
        for i in 0..512 {
            unsafe {
                if self.ptes.add(i).read() != 0 {
                    return false;
                }
            }
        }

        true
    }
}

pub struct PageManager {
    pub root: PageDtrectory,
}

impl PageManager {
    pub unsafe fn new() -> Self {
        let root_pdir = Self::alloc_page_dir();
        Self {
            root: PageDtrectory {
                ptes: (root_pdir << 12) as *mut u64,
            },
        }
    }
    pub unsafe fn from_satp() -> Self {
        let addr = get_satp() & 0xfffffffffff;
        Self {
            root: PageDtrectory {
                ptes: (addr << 12) as *mut u64,
            },
        }
    }
    /** Allocate a page directory.
     *
     * Return: Pysical Page Number
     */
    pub unsafe fn alloc_page_dir() -> u64 {
        alloc_zeroed(Layout::new::<[u8; 4096]>()) as u64 >> 12
    }
    /** Release a page directory. */
    pub unsafe fn release_page_dir(ppn: u64) {
        dealloc((ppn << 12) as *mut u8, Layout::new::<[u8; 4096]>());
    }
    pub fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
}

impl PageManagement for PageManager {
    unsafe fn map(&self, vpn: usize, ppn: usize, mode: &[PageACL]) {
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

        let v1_pte = *self.root.ptes.add(v1);
        let v1_pte = if v1_pte == 0 {
            /* v1 PTE is empty */
            let ppn = Self::alloc_page_dir();
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            self.root.set_pte(v1, pte);

            pte
        } else {
            v1_pte.into()
        };

        let v2_pdir = PageDtrectory::from_ppn(v1_pte.ppn);
        let v2_pte = *v2_pdir.ptes.add(v2);
        let v2_pte = if v2_pte == 0 {
            /* v2 PTE is empty */
            let ppn = Self::alloc_page_dir();
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            v2_pdir.set_pte(v2, pte);

            pte
        } else {
            v2_pte.into()
        };

        let v3_pdir = PageDtrectory::from_ppn(v2_pte.ppn);
        let v3_pte = *v3_pdir.ptes.add(v3);
        let mut v3_pte: PageTableEntry = v3_pte.into();
        v3_pte.ppn = ppn as u64;
        /* set mode */
        let v3_pte: u64 = v3_pte.into();
        let v3_pte = (v3_pte | mode_u64).into();
        v3_pdir.set_pte(v3, v3_pte);
    }
    unsafe fn unmap(&self, vpn: usize) {
        let v1 = vpn >> 18;
        let v2 = (vpn >> 9) & 0x1ff;
        let v3 = vpn & 0x1ff;

        let v1_pte: PageTableEntry = (*self.root.ptes.add(v1)).into();
        let v2_pdir = PageDtrectory::from_ppn(v1_pte.ppn);
        let v2_pte: PageTableEntry = (*v2_pdir.ptes.add(v2)).into();
        let v3_pdir = PageDtrectory::from_ppn(v2_pte.ppn);
        v3_pdir.ptes.add(v3).write(0);

        if v3_pdir.is_empty() {
            Self::release_page_dir(v2_pte.ppn);
            v2_pdir.ptes.add(v2).write(0);
        }

        if v2_pdir.is_empty() {
            Self::release_page_dir(v1_pte.ppn);
            self.root.ptes.add(v1).write(0);
        }
    }
    unsafe fn switch_to(&self) {
        set_satp(self.root_ppn(), MODE_SV39);
    }
}
