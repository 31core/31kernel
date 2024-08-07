use crate::*;

pub enum PageACL {
    Read,
    Write,
    Execute,
}

pub trait PageManagement {
    /**
     * Set PTE address.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `mode`: Page access mode.
     */
    unsafe fn set_pte_addr(&self, vpn: u64, ppn: u64, mode: &[PageACL]);
    unsafe fn switch_to(&self);
    unsafe fn set_kernel_page(&self) {
        /* set kernel stack */
        for i in 0..STACK_SIZE / PAGE_SIZE {
            self.set_pte_addr(
                crate::heap_start as u64 + i as u64,
                crate::heap_start as u64 + i as u64,
                &[PageACL::Read, PageACL::Write],
            );
        }
        /* set kernel code */
        for i in 0..(kernel_end as usize - kernel_start as usize) / PAGE_SIZE {
            self.set_pte_addr(
                crate::kernel_start as u64 + i as u64,
                crate::kernel_start as u64 + i as u64,
                &[PageACL::Read, PageACL::Write],
            );
        }
    }
}
