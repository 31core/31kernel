use crate::*;

pub enum PageACL {
    Read,
    Write,
    Execute,
}

pub trait PageManagement {
    /**
     * Map virtual page into physical page.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `mode`: Page access mode.
     */
    unsafe fn map(&self, vpn: usize, ppn: usize, mode: &[PageACL]);
    /**
     * Unset the map.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     */
    unsafe fn unmap(&self, vpn: usize);
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    /** map kernel memory into vm */
    unsafe fn map_kernel_region(&self) {
        /* set kernel stack */
        for i in 0..STACK_SIZE / PAGE_SIZE {
            self.map(
                crate::heap_start as usize + i,
                crate::heap_start as usize + i,
                &[PageACL::Read, PageACL::Write],
            );
        }
        /* set kernel code */
        for i in 0..(kernel_end as usize - kernel_start as usize) / PAGE_SIZE {
            self.map(
                crate::kernel_start as usize + i,
                crate::kernel_start as usize + i,
                &[PageACL::Read, PageACL::Write, PageACL::Execute],
            );
        }
    }
}
