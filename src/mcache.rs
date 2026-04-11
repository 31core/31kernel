/*!
 * An mcache allocator for small objects allocation.
 */

use crate::{PAGE_SIZE, alloc_pages, buddy_allocator::ceil_to_power_2, free_pages};
use alloc::alloc::{alloc, dealloc};
use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    ptr::NonNull,
};

const CACHE_NUM: usize = 1024;
const SIZE_CLASS_COUNT: usize = 14;

/** Alias to `GLOBAL_ALLOCATOR`, when used as the first allocator. */
#[doc(hidden)]
macro_rules! first_mgr {
    () => {
        (*(&raw mut GLOBAL_ALLOCATOR))
    };
}

#[doc(hidden)]
macro_rules! ref_to_ptr {
    ($ref:ident, $type:ident) => {
        $ref as *const $type as *mut $type
    };
}

/**
 * Check if a size of object can be allocated with a cache and get the cache size if so.
 *
 * Returns `None` if the size is too large for cache allocation,
 * and otherwise `Some(((cell_size, cell_count), idx))`,
 * where `cell_size` is the size of each cell in the cache,
 * `cell_count` is the number of cells in each cache,
 * and `idx` is the index of the cache class.
 */
fn size_to_class(size: usize) -> Option<((usize, usize), usize)> {
    const KB: usize = 1024;
    const CACHE_SIZE: [(usize, usize); SIZE_CLASS_COUNT] = [
        (64, 1024),
        (128, 512),
        (256, 256),
        (512, 128),
        (KB, 64),
        (2 * KB, 32),
        (4 * KB, 16),
        (8 * KB, 8),
        (16 * KB, 8),
        (32 * KB, 8),
        (64 * KB, 8),
        (128 * KB, 8),
        (256 * KB, 4),
        (512 * KB, 2),
    ];
    let idx = CACHE_SIZE
        .into_iter()
        .position(|(objsize, _)| objsize >= size);
    idx.map(|idx| (CACHE_SIZE[idx], idx))
}

fn calc_header_padding(align: usize) -> usize {
    let ptr_size = size_of::<*mut CacheManager>();
    let offset = ptr_size;
    (align - (offset % align)) % align
}

#[global_allocator]
static mut GLOBAL_ALLOCATOR: CacheManager = CacheManager {
    caches: [None; CACHE_NUM],
    next: None,
    prev: None,
    next_partial_free: [None; SIZE_CLASS_COUNT],
    partial_counts: [0; SIZE_CLASS_COUNT],
    next_free: None,
    allocated_caches: 0,
    is_init: false,
};

/**
 * A cache cell with the following structure.
 * Allocated cell:
 * ```
 * +------------------------------------------+
 * | Cache manager pointer | Padding | Object |
 * +------------------------------------------+
 * ```
 *
 * Unallocated cell:
 * ```
 * +-----------------------------+
 * | Next pointer | Unused space |
 * +-----------------------------+
 * ```
 */
struct CacheCell {
    ptr: *mut u8,
}

impl CacheCell {
    fn object_ptr(&self, padding: usize) -> *mut u8 {
        unsafe { self.ptr.add(padding + size_of::<*mut CacheManager>()) }
    }
    /** Write cache manager pointer. */
    unsafe fn write_head(&self, mgr: *const CacheManager) {
        unsafe { (self.ptr as *mut *const CacheManager).write(mgr) };
    }
    unsafe fn read_next(&self) -> *mut u8 {
        unsafe { (self.ptr as *const *mut u8).read() }
    }
    unsafe fn write_next(&self, next: *mut u8) {
        unsafe { (self.ptr as *mut *mut u8).write(next) };
    }
}

/**
 * An allocator for certain fix-sized cells.
 */
struct CachePage {
    /** Address of the first page. */
    page_start: *mut u8,
    page_count: usize,
    /** Must be >= `usize`. */
    cell_size: usize,
    free_count: usize,
    used_count: usize,
    /** Address of the first cell. */
    free_list_head: *mut u8,
}

impl CachePage {
    /** Initialize cache on pages. */
    unsafe fn init(&mut self, offset: usize) {
        for i in 0..self.free_count - 1 {
            unsafe {
                let ptr = self.page_start.add((offset + i) * self.cell_size);
                let ptr_next = self.page_start.add((offset + i + 1) * self.cell_size);
                CacheCell { ptr }.write_next(ptr_next);
            }
        }

        unsafe {
            let ptr = self
                .page_start
                .add(self.page_count * PAGE_SIZE - self.cell_size);
            CacheCell { ptr }.write_next(core::ptr::null_mut());
        }
    }
    unsafe fn alloc_obj(&mut self) -> Option<*mut u8> {
        if self.free_count == 0 {
            None
        } else {
            self.free_count -= 1;
            self.used_count += 1;

            let next_ptr = unsafe {
                CacheCell {
                    ptr: self.free_list_head,
                }
                .read_next()
            };
            let alloc_addr = self.free_list_head;
            self.free_list_head = next_ptr;
            Some(alloc_addr)
        }
    }
    unsafe fn free_obj(&mut self, ptr: *mut u8) {
        self.used_count -= 1;
        self.free_count += 1;

        unsafe { CacheCell { ptr }.write_next(self.free_list_head) };
        self.free_list_head = ptr;
    }
    /** Check if an cell is allocated by this cache. */
    fn in_range(&self, ptr: *const u8) -> bool {
        (ptr as usize) >= self.page_start as usize
            && (ptr as usize) < unsafe { self.page_start.add(self.page_count * PAGE_SIZE) } as usize
    }
}

pub struct CacheManager {
    /** Cache slots. */
    caches: [Option<NonNull<CachePage>>; CACHE_NUM],
    next: Option<NonNull<Self>>,
    prev: Option<NonNull<Self>>,
    /**
     * Next pointers of linked tables maintaining lists of `CacheManager`s with at least one
     * allocated but not full caches for `idx` cell size.
     */
    next_partial_free: [Option<NonNull<Self>>; SIZE_CLASS_COUNT],
    partial_counts: [usize; SIZE_CLASS_COUNT],
    /** Next pointer of a linked table maintaining a list of `CacheManager`s with at least one free cache slots. */
    next_free: Option<NonNull<Self>>,
    /** Number of allocated cache slots. */
    allocated_caches: usize,
    is_init: bool,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self {
            caches: [None; CACHE_NUM],
            next: None,
            prev: None,
            next_partial_free: [None; SIZE_CLASS_COUNT],
            partial_counts: [0; SIZE_CLASS_COUNT],
            allocated_caches: 0,
            next_free: None,
            is_init: false,
        }
    }
}

impl CacheManager {
    fn add_cache(&mut self, cache_ptr: *mut CachePage) {
        for cache in &mut self.caches {
            if cache.is_none() {
                *cache = NonNull::new(cache_ptr);
                self.allocated_caches += 1;
                break;
            }
        }
    }
}

impl CacheManager {
    fn is_cache_full(&self) -> bool {
        self.allocated_caches == CACHE_NUM
    }
    fn is_global_allocator(&self) -> bool {
        self.prev.is_none() // only the first allocator has null previous pointer
    }
    /** Allocate an object using a free cache. */
    unsafe fn cache_alloc(&mut self, cell_size: usize, idx: usize, padding: usize) -> *mut u8 {
        unsafe {
            for mut cache in self.caches.into_iter().flatten() {
                if cache.as_ref().cell_size == cell_size
                    && let Some(ptr) = cache.as_mut().alloc_obj()
                {
                    if cache.as_ref().free_count == 0 {
                        self.partial_counts[idx] -= 1;
                        /* delete current node from partial free list */
                        if !self.is_global_allocator() && self.partial_counts[idx] == 0 {
                            first_mgr!().next_partial_free[idx] = self.next_partial_free[idx];
                        }
                    }
                    let obj = CacheCell { ptr };
                    obj.write_head(ref_to_ptr!(self, Self));
                    return obj.object_ptr(padding);
                }
            }
            unreachable!("No free cache found.");
        }
    }
    /** Add a new cache and allocate an object. */
    unsafe fn new_cache_alloc(
        &mut self,
        cell_size: usize,
        cell_count: usize,
        idx: usize,
        padding: usize,
    ) -> *mut u8 {
        unsafe {
            let page_count = ceil_to_power_2(cell_count * cell_size / PAGE_SIZE);
            let offset = size_of::<CachePage>().div_ceil(cell_size); // offest in n cells size
            let cache_addr = (PAGE_SIZE * alloc_pages!(page_count)) as *mut CachePage;
            let mut cache = CachePage {
                page_start: cache_addr as *mut u8,
                page_count,
                cell_size,
                used_count: 0,
                free_count: page_count * PAGE_SIZE / cell_size - offset,
                free_list_head: (cache_addr).byte_add(offset * cell_size) as *mut u8,
            };
            cache.init(offset);
            let ptr = cache.alloc_obj().unwrap();
            cache_addr.write(cache);

            /* insert current manager to partial free list */
            if !self.is_global_allocator() && self.partial_counts[idx] == 0 {
                self.next_partial_free[idx] = first_mgr!().next_partial_free[idx];
                first_mgr!().next_partial_free[idx] = NonNull::new(ref_to_ptr!(self, Self));
            }

            self.add_cache(cache_addr);
            self.partial_counts[idx] += 1;

            /* delete current manager from free list */
            if !self.is_global_allocator() && self.is_cache_full() {
                first_mgr!().next_free = self.next_free;
            }

            let cell = CacheCell { ptr };
            cell.write_head(ref_to_ptr!(self, Self));
            cell.object_ptr(padding)
        }
    }
    unsafe fn allocate_inner(&mut self, layout: Layout) -> *mut u8 {
        unsafe {
            if !self.is_init {
                self.is_init = true;
                let next = alloc(Layout::new::<Self>()) as *mut Self;
                self.next = NonNull::new(next);
                next.write(Self {
                    prev: NonNull::new(ref_to_ptr!(self, Self)),
                    next_free: first_mgr!().next_free,
                    ..Default::default()
                });
                first_mgr!().next_free = NonNull::new(next);
            }

            let padding = calc_header_padding(layout.align());
            let alloc_size = layout.size() + padding + size_of::<*mut Self>();
            /* allocate with cache manager */
            if let Some(((cell_size, cell_count), idx)) = size_to_class(alloc_size) {
                if self.partial_counts[idx] > 0 {
                    self.cache_alloc(cell_size, idx, padding)
                } else if !self.is_cache_full() {
                    self.new_cache_alloc(cell_size, cell_count, idx, padding)
                } else {
                    if let Some(mut next) = self.next_partial_free[idx] {
                        next.as_mut().cache_alloc(cell_size, idx, padding)
                    } else {
                        self.next_free
                            .unwrap()
                            .as_mut()
                            .new_cache_alloc(cell_size, cell_count, idx, padding)
                    }
                }
            }
            /* use buddy allocator for large object */
            else {
                (PAGE_SIZE * alloc_pages!(layout.size().div_ceil(PAGE_SIZE))) as *mut u8
            }
        }
    }
}

unsafe impl GlobalAlloc for CacheManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { (*(&raw mut GLOBAL_ALLOCATOR)).allocate_inner(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let padding = calc_header_padding(layout.align());
        let alloc_size = layout.size() + padding + size_of::<*mut Self>();

        unsafe {
            if let Some((_, idx)) = size_to_class(alloc_size) {
                let mgr = (ptr as *mut *mut Self)
                    .byte_sub(padding + size_of::<*mut Self>())
                    .read();

                for i in &mut (*mgr).caches {
                    if let Some(mut cache) = *i
                        && cache.as_ref().in_range(ptr)
                    {
                        /* object cache is full */
                        if cache.as_ref().free_count == 0 {
                            if !(*mgr).is_global_allocator() && (*mgr).partial_counts[idx] == 0 {
                                (*mgr).next_partial_free[idx] = first_mgr!().next_partial_free[idx];
                                first_mgr!().next_partial_free[idx] = NonNull::new(mgr);
                            }
                            (*mgr).partial_counts[idx] += 1;
                        }
                        let cell_ptr = ptr.sub(padding + size_of::<*mut Self>());
                        cache.as_mut().free_obj(cell_ptr);

                        /* object cache is empty */
                        if cache.as_ref().used_count == 0 {
                            /* free object cache */
                            free_pages!(
                                cache.as_ref().page_start as usize / PAGE_SIZE,
                                cache.as_ref().page_count
                            );
                            *i = None;

                            /* insert the manager to free list */
                            if !(*mgr).is_global_allocator() && (*mgr).is_cache_full() {
                                (*mgr).next_free = first_mgr!().next_free;
                                first_mgr!().next_free = NonNull::new(mgr);
                            }
                            (*mgr).allocated_caches -= 1;
                            (*mgr).partial_counts[idx] -= 1;

                            /* free the manager */
                            if !(*mgr).is_global_allocator()
                                && (*mgr).allocated_caches == 0
                                && let Some(mut prev) = (*mgr).prev
                                && /* do not dealloc the last allocator*/ (*mgr).next.is_some()
                            {
                                prev.as_mut().next = (*mgr).next;

                                /* delete the manager from free list. */
                                let mut prev_free = first_mgr!().next_free.unwrap();
                                while let Some(next) = prev_free.as_ref().next_free
                                    && next.as_ptr() != mgr
                                {
                                    prev_free = next;
                                }
                                prev_free.as_mut().next_free = (*mgr).next_free;

                                dealloc(mgr as *mut u8, Layout::new::<Self>());
                            }
                        }

                        return;
                    }
                }
            } else {
                free_pages!(ptr as usize / PAGE_SIZE, layout.size().div_ceil(PAGE_SIZE));
            }
        }
    }
}
