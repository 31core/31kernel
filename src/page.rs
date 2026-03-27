//! Common code for page management

use core::ptr::addr_of;

pub enum PageACL {
    Read,
    Write,
    Execute,
    User,
}

macro_rules! map_range {
    ($start:expr, $end:expr, $mgr:expr, $map_fn:ident) => {
        $mgr.$map_fn(
            (addr_of!($start) as usize >> 12),
            (addr_of!($start) as usize >> 12),
            (addr_of!($end) as usize >> 12) - (addr_of!($start) as usize >> 12),
        );
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
    unsafe fn map(&mut self, vpn: usize, ppn: usize, pages: usize, mode: &[PageACL]);
    /**
     * Map as read-only acl
     */
    unsafe fn map_rodata(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read]) };
    }
    /**
     * Map as read-write acl
     */
    unsafe fn map_data(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read, PageACL::Write]) };
    }
    /**
     * Map as read-execute acl
     */
    unsafe fn map_text(&mut self, vpn: usize, ppn: usize, pages: usize) {
        unsafe { self.map(vpn, ppn, pages, &[PageACL::Read, PageACL::Execute]) };
    }
    /**
     * Unset the map.
     *
     * Args:
     * * `vpn`: Virtual Page Number.
     */
    unsafe fn unmap(&mut self, vpn: usize, pages: usize);
    /**
     * Switch to the page directory.
     */
    unsafe fn switch_to(&self);
    unsafe fn refresh(&self);
    unsafe fn destroy(&mut self);
    /** map kernel memory into vm */
    unsafe fn map_kernel_region(&mut self) {
        unsafe {
            /* map .rodata */
            map_range!(crate::RODATA_START, crate::RODATA_END, self, map_rodata);
            /* map .data */
            map_range!(crate::DATA_START, crate::DATA_END, self, map_data);
            /* map .bss */
            map_range!(crate::BSS_START, crate::BSS_END, self, map_data);
            /* set kernel code (.text) */
            map_range!(crate::KERNEL_START, crate::KERNEL_END, self, map_text);
        }
    }
}
