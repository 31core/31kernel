//! Common code for page management

use crate::PAGE_SIZE;

pub enum PageACL {
    Read,
    Write,
    Execute,
}

macro_rules! map_range {
    ($start:expr, $end:expr, $mgr:expr, $map_fn:ident) => {
        for i in 0..($end as *const usize as usize - $start as *const usize as usize) / PAGE_SIZE {
            unsafe {
                $mgr.$map_fn(
                    ($start as *const usize as usize >> 12) + i,
                    ($start as *const usize as usize >> 12) + i,
                );
            }
        }
    };
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
    unsafe fn map(&mut self, vpn: usize, ppn: usize, mode: &[PageACL]);
    /**
     * Map as read-only acl
     */
    unsafe fn map_rodata(&mut self, vpn: usize, ppn: usize) {
        unsafe { self.map(vpn, ppn, &[PageACL::Read]) };
    }
    /**
     * Map as read-write acl
     */
    unsafe fn map_data(&mut self, vpn: usize, ppn: usize) {
        unsafe { self.map(vpn, ppn, &[PageACL::Read, PageACL::Write]) };
    }
    /**
     * Map as read-execute acl
     */
    unsafe fn map_text(&mut self, vpn: usize, ppn: usize) {
        unsafe { self.map(vpn, ppn, &[PageACL::Read, PageACL::Execute]) };
    }
    /**
     * Unset the map.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     */
    unsafe fn unmap(&mut self, vpn: usize);
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    /** map kernel memory into vm */
    unsafe fn map_kernel_region(&mut self) {
        /* map .rodata */
        map_range!(crate::rodata_start, crate::rodata_end, self, map_rodata);
        /* map .data */
        map_range!(crate::data_start, crate::data_end, self, map_data);
        /* map .bss */
        map_range!(crate::bss_start, crate::bss_end, self, map_data);
        /* set kernel code (.text) */
        map_range!(crate::kernel_start, crate::kernel_end, self, map_text);
    }
}
