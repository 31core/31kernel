use alloc::alloc::{alloc_zeroed, Layout};
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
    ppn |= mode << 59;
    asm!("csrw satp, {}", in(reg) ppn);
}

pub unsafe fn get_satp() -> u64 {
    let mut satp;
    asm!("csrr {}, satp", out(reg) satp);

    satp
}

const PTE_V_FLAG: u64 = 1;
const PTE_R_FLAG: u64 = 1 << 1;
const PTE_W_FLAG: u64 = 1 << 2;
const PTE_X_FLAG: u64 = 1 << 3;

#[derive(Default, Clone, Copy)]
pub struct PageTableEntry {
    pub r: bool,
    pub w: bool,
    pub x: bool,
    pub ppn: u64,
}

impl PageTableEntry {
    fn load(pte_u64: u64) -> Self {
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

        pte.ppn = pte_u64 >> 9;

        pte
    }
    fn dump(&self) -> u64 {
        let mut pte = 0;

        pte |= PTE_V_FLAG;

        if self.r {
            pte |= PTE_R_FLAG;
        }

        if self.w {
            pte |= PTE_W_FLAG;
        }

        if self.x {
            pte |= PTE_X_FLAG;
        }

        pte |= self.ppn << 9;

        pte
    }
}

pub struct PageDtrectory {
    pub ptes: *mut u64,
}

impl PageDtrectory {
    pub unsafe fn set_pte(&self, count: usize, pte: PageTableEntry) {
        self.ptes.add(count).write(pte.dump());
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
    /**
     * Set PTE address.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     */
    pub unsafe fn set_pte_addr(&self, vpn: u64, ppn: u64) {
        let v1 = vpn / (512 * 512);
        let v2 = vpn % (512 * 512) / 512;
        let v3 = vpn % 512;

        let v1_pte = *self.root.ptes.add(v1 as usize);
        let v1_pte = if v1_pte == 0 {
            /* v1 PTE is empty */
            let ppn = Self::alloc_page_dir();
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            self.root.set_pte(v1 as usize, pte);

            pte
        } else {
            PageTableEntry::load(v1_pte)
        };

        let v2_pdir = PageDtrectory {
            ptes: (v1_pte.ppn << 12) as *mut u64,
        };
        let v2_pte = *v2_pdir.ptes.add(v2 as usize);
        let v2_pte = if v2_pte == 0 {
            /* v2 PTE is empty */
            let ppn = Self::alloc_page_dir();
            let pte = PageTableEntry {
                ppn,
                ..Default::default()
            };
            v2_pdir.set_pte(v2 as usize, pte);

            pte
        } else {
            PageTableEntry::load(v2_pte)
        };

        let v3_pdir = PageDtrectory {
            ptes: (v2_pte.ppn << 12) as *mut u64,
        };
        let v3_pte = *v3_pdir.ptes.add(v3 as usize);
        let mut v3_pte = PageTableEntry::load(v3_pte);
        v3_pte.ppn = ppn;
        v3_pdir.set_pte(v3 as usize, v3_pte);
    }
    /** Allocate a page directory.
     *
     * Return: Pysical Page Number
     */
    pub unsafe fn alloc_page_dir() -> u64 {
        alloc_zeroed(Layout::new::<[u8; 4096]>()) as u64 >> 12
    }
    pub fn root_ppn(&self) -> u64 {
        self.root.ptes as u64 >> 12
    }
}
