use super::{PAGE_BITS, PAGE_SIZE, PageACL, PageAllocator, buddy_allocator::BUDDY_ALLOCATOR};
use crate::address::{PhysAddr, PhysPage, VirtPage, VirtualAddress, VirtualPage};
use core::ptr::addr_of;

macro_rules! map_range_with_alloc {
    ($alloc:ident, $start:expr, $end:expr, $mgr:expr, $mode:expr) => {
        $mgr.map_with_allocator(
            $alloc,
            addr_of!($start) as usize >> PAGE_BITS,
            PhysAddr::from(VirtualAddress(addr_of!($start) as usize)).0 >> PAGE_BITS,
            (addr_of!($end) as usize >> PAGE_BITS) - (addr_of!($start) as usize >> PAGE_BITS),
            $mode,
        );
    };
}

const RO: &[PageACL] = &[PageACL::Read];
const RW: &[PageACL] = &[PageACL::Read, PageACL::Write];
const RX: &[PageACL] = &[PageACL::Read, PageACL::Execute];
const URO: &[PageACL] = &[PageACL::User, PageACL::Read];
const URW: &[PageACL] = &[PageACL::User, PageACL::Read, PageACL::Write];
const URX: &[PageACL] = &[PageACL::User, PageACL::Read, PageACL::Execute];

const MAX_SUPPORTED_LEVEL: usize = 5;

pub trait Entry: Copy {
    fn new(page_number: usize, leaf: bool, mode: &[PageACL]) -> Self;
    fn empty() -> Self;
    fn page_number(&self) -> PhysPage;
    fn is_valid(&self) -> bool;
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Table<E: Entry> {
    ptr: *mut E,
}

impl<E: Entry> Table<E> {
    fn empty() -> Self {
        Self {
            ptr: core::ptr::null_mut(),
        }
    }
    pub fn new<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator,
    {
        let page = alloc.alloc_pages(1).unwrap();
        let ptr = (page << PAGE_BITS) as *mut E;
        unsafe { core::ptr::write_bytes(ptr as *mut u8, 0, PAGE_SIZE) };
        Self { ptr }
    }
    pub fn from_page(page_number: VirtPage) -> Self {
        Self {
            ptr: (page_number.0 << PAGE_BITS) as *mut E,
        }
    }
    pub fn page_number(&self) -> PhysPage {
        PhysPage::from(VirtualPage((self.ptr as usize) >> PAGE_BITS))
    }
    fn set_entry(&self, index: usize, entry: E) {
        unsafe { self.ptr.add(index).write_volatile(entry) };
    }
    fn get_entry(&self, index: usize) -> E {
        unsafe { self.ptr.add(index).read_volatile() }
    }
    fn is_empty(&self, entry_count: usize) -> bool {
        for index in 0..entry_count {
            if self.get_entry(index).is_valid() {
                return false;
            }
        }
        true
    }
}

pub trait Mapper<E: Entry> {
    const LEVEL: usize;
    const PTES_PER_DIR: usize;
    const PTE_BITS: usize;
    fn root_table(&self) -> Table<E>;

    fn new_with_allocator<A>(alloc: &mut A) -> Self
    where
        A: PageAllocator;

    fn new() -> Self
    where
        Self: Sized,
    {
        let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
        Self::new_with_allocator(alloc_guard)
    }

    fn from_root(page_number: VirtPage) -> Self;

    fn map_4k<A>(&mut self, alloc: &mut A, vpn: usize, ppn: usize, mode: &[PageACL])
    where
        A: PageAllocator,
    {
        let mut current_table = self.root_table();

        for level in (1..Self::LEVEL).rev() {
            let index = (vpn >> (Self::PTE_BITS * level)) & (Self::PTES_PER_DIR - 1);
            let mut entry = current_table.get_entry(index);
            if !entry.is_valid() {
                let sub_table = Table::<E>::new(alloc);
                entry = E::new(sub_table.page_number().0, false, &[]);
                current_table.set_entry(index, entry);
            }
            current_table = Table::from_page(VirtPage::from(entry.page_number()));
        }

        let entry = E::new(ppn, true, mode);
        current_table.set_entry(vpn & (Self::PTES_PER_DIR - 1), entry);
    }

    /**
     * Map virtual page into physical page with a specified [PageAllocator].
     *
     * Args:
     * * `alloc`: Page allocator.
     * * `vpn`: Virtual Page Number.
     * * `ppn`: Pysical Page Number.
     * * `pages`: Pages count to map.
     * * `mode`: Page access mode.
     */
    fn map_with_allocator<A>(
        &mut self,
        alloc: &mut A,
        mut vpn: usize,
        mut ppn: usize,
        mut pages: usize,
        mode: &[PageACL],
    ) where
        A: PageAllocator,
    {
        while pages > 0 {
            self.map_4k(alloc, vpn, ppn, mode);
            vpn += 1;
            ppn += 1;
            pages -= 1;
        }
    }

    fn map(&mut self, vpn: usize, ppn: usize, pages: usize, mode: &[PageACL]) {
        let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
        self.map_with_allocator(alloc_guard, vpn, ppn, pages, mode);
    }

    /**
     * Map as read-only acl
     */
    fn map_rodata(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, RO);
    }
    /**
     * Map as read-write acl
     */
    fn map_data(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, RW);
    }
    /**
     * Map as read-execute acl
     */
    fn map_text(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, RX);
    }
    /**
     * Map as read-execute acl, user accessible
     */
    fn map_text_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, URX);
    }
    /**
     * Map as read-only acl, user accessible
     */
    fn map_rodata_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, URO);
    }
    /**
     * Map as read-write acl, user accessible
     */
    fn map_data_u(&mut self, vpn: usize, ppn: usize, pages: usize) {
        self.map(vpn, ppn, pages, URW);
    }

    fn unmap_4k<A>(&mut self, alloc: &mut A, vpn: usize)
    where
        A: PageAllocator,
    {
        debug_assert!(Self::LEVEL <= MAX_SUPPORTED_LEVEL);
        let mut page_tables = [Table::empty(); MAX_SUPPORTED_LEVEL];
        let mut indexes = [0; MAX_SUPPORTED_LEVEL];

        page_tables[Self::LEVEL - 1] = self.root_table();

        for level in (0..Self::LEVEL).rev() {
            let index = (vpn >> (Self::PTE_BITS * level)) & (Self::PTES_PER_DIR - 1);
            indexes[level] = index;

            if level == 0 {
                page_tables[level].set_entry(index, E::empty());
            } else {
                let entry = page_tables[level].get_entry(index);
                page_tables[level - 1] = Table::from_page(VirtPage::from(entry.page_number()));
            }
        }

        for table in page_tables.iter().take(Self::LEVEL - 1) {
            if table.is_empty(Self::PTES_PER_DIR) {
                let page_start = VirtPage::from(table.page_number()).0;
                alloc.free_pages(page_start, 1);
            } else {
                break;
            }
        }
    }

    fn unmap_with_allocator<A>(&mut self, alloc: &mut A, mut vpn: usize, mut pages: usize)
    where
        A: PageAllocator,
    {
        while pages > 0 {
            self.unmap_4k(alloc, vpn);
            vpn += 1;
            pages -= 1;
        }
    }

    fn destroy_with_allocator<A>(&mut self, alloc: &mut A)
    where
        A: PageAllocator,
    {
        debug_assert!(Self::LEVEL <= MAX_SUPPORTED_LEVEL);
        let mut page_tables = [Table::empty(); MAX_SUPPORTED_LEVEL];
        let mut indexes = [0; MAX_SUPPORTED_LEVEL];

        let mut current_level = Self::LEVEL - 1;
        page_tables[current_level] = self.root_table();

        loop {
            if current_level == 1 {
                for index in 0..Self::PTES_PER_DIR {
                    let entry = page_tables[current_level].get_entry(index);
                    if entry.is_valid() {
                        alloc.free_pages(VirtPage::from(entry.page_number()).0, 1);
                    }
                }
                current_level += 1;
                indexes[current_level] += 1;
            } else if indexes[current_level] == Self::PTES_PER_DIR {
                alloc.free_pages(
                    VirtPage::from(page_tables[current_level].page_number()).0,
                    1,
                );
                if current_level == Self::LEVEL - 1 {
                    break;
                }
                indexes[current_level] = 0;
                current_level += 1;
                indexes[current_level] += 1;
            } else if !page_tables[current_level]
                .get_entry(indexes[current_level])
                .is_valid()
            {
                indexes[current_level] += 1;
            } else {
                let entry = page_tables[current_level].get_entry(indexes[current_level]);
                current_level -= 1;
                page_tables[current_level] = Table::from_page(VirtPage::from(entry.page_number()));
            }
        }
    }

    fn destroy(&mut self) {
        let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
        self.destroy_with_allocator(alloc_guard);
    }

    /** map kernel memory into vm */
    fn map_kernel_region(&mut self) {
        let alloc_guard = &mut *BUDDY_ALLOCATOR.lock();
        self.map_kernel_region_with_allocator(alloc_guard);
    }
    /** map kernel memory into vm, using a static page allocator */
    fn map_kernel_region_with_allocator<A>(&mut self, alloc: &mut A)
    where
        A: PageAllocator,
    {
        /* map .rodata */
        map_range_with_alloc!(alloc, crate::RODATA_START, crate::RODATA_END, self, RO);
        /* map .data */
        map_range_with_alloc!(alloc, crate::DATA_START, crate::DATA_END, self, RW);
        /* map .bss */
        map_range_with_alloc!(alloc, crate::BSS_START, crate::BSS_END, self, RW);
        /* set kernel code (.text) */
        map_range_with_alloc!(alloc, crate::KERNEL_START, crate::KERNEL_END, self, RX);
    }
}
