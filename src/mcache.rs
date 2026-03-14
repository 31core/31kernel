/*!
 * An mcache allocator for small objects allocation.
 */

use crate::{
    PAGE_SIZE,
    buddy_allocator::{BUDDY_ALLOCATOR, ceil_to_power_2},
};
use alloc::boxed::Box;
use core::{
    alloc::{GlobalAlloc, Layout},
    mem::size_of,
    ptr::NonNull,
};

const CACHE_NUM: usize = 1024;
const CACHE_OBJ_COUNT: usize = 512;

/** Check if a size of object can be allocated with a cache and get the cache size if so. */
fn to_objsize(size: usize) -> Option<usize> {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;
    const CACHE_SIZE: [usize; 16] = [
        64,
        128,
        256,
        512,
        KB,
        2 * KB,
        4 * KB,
        16 * KB,
        32 * KB,
        64 * KB,
        128 * KB,
        256 * KB,
        512 * KB,
        MB,
        2 * MB,
        4 * MB,
    ];
    CACHE_SIZE.into_iter().find(|&objsize| objsize >= size)
}

fn padding_bytes(align: usize) -> usize {
    let ptr_size = size_of::<*mut CacheManager>();
    let mut n_align = align;
    loop {
        if n_align >= ptr_size {
            return n_align - ptr_size;
        }
        n_align += align;
    }
}

#[global_allocator]
static mut GLOBAL_ALLOCATOR: CacheManager = CacheManager {
    caches: [None; CACHE_NUM],
    next: None,
    is_cache_full: false,
    is_init: false,
};

/**
 * A cache object with the following structure.
 * Allocated object:
 * ```
 * +------------------------------------------+
 * | Cache manager pointer | Padding | Object |
 * +------------------------------------------+
 * ```
 *
 * Unallocated object:
 * ```
 * +-----------------------------+
 * | Next pointer | Unused space |
 * +-----------------------------+
 * ```
 */
struct CacheObject {
    ptr: *mut u8,
}

impl CacheObject {
    fn object_ptr(&self, padding: usize) -> *mut u8 {
        unsafe { self.ptr.add(padding + size_of::<*mut CacheManager>()) }
    }
    /** Write cache manager pointer. */
    unsafe fn write_head(&self, mgr: *const CacheManager) {
        unsafe { (self.ptr as *mut *const CacheManager).write(mgr) };
    }
    unsafe fn read_next(&self) -> *mut u8 {
        unsafe { (self.ptr as *const usize).read() as *mut u8 }
    }
    unsafe fn write_next(&self, next: *mut u8) {
        unsafe { (self.ptr as *mut *const u8).write(next) };
    }
}

/**
 * An allocator for certain fix-sized objects.
 */
struct CachePage {
    /** Address of the first page. */
    page_start: *mut u8,
    page_num: usize,
    /** Must be >= `usize`. */
    obj_size: usize,
    obj_free: usize,
    obj_alloc: usize,
    /** Address of the first object. */
    obj_start: *mut u8,
}

impl CachePage {
    /** Initialize cache on pages. */
    unsafe fn init(&mut self, offset: usize) {
        for i in 0..self.obj_free - 1 {
            unsafe {
                CacheObject {
                    ptr: self.page_start.add((offset + i) * self.obj_size),
                }
                .write_next(self.page_start.add((offset + i + 1) * self.obj_size));
            }
        }

        unsafe {
            CacheObject {
                ptr: (self
                    .page_start
                    .add(self.page_num * PAGE_SIZE - self.obj_size)),
            }
            .write_next(core::ptr::null_mut());
        }
    }
    unsafe fn alloc_obj(&mut self) -> Option<*mut u8> {
        if self.obj_free == 0 {
            None
        } else {
            self.obj_free -= 1;
            self.obj_alloc += 1;

            let next_ptr = unsafe {
                CacheObject {
                    ptr: self.obj_start,
                }
                .read_next()
            };
            let alloc_addr = self.obj_start;
            self.obj_start = next_ptr;
            Some(alloc_addr)
        }
    }
    unsafe fn free_obj(&mut self, ptr: *mut u8) {
        self.obj_alloc -= 1;
        self.obj_free += 1;

        unsafe { CacheObject { ptr }.write_next(self.obj_start) };
        self.obj_start = ptr;
    }
    /** Check if an object is allocated by this cache. */
    fn in_range(&self, ptr: *const u8) -> bool {
        (ptr as usize) >= self.page_start as usize
            && (ptr as usize) < unsafe { self.page_start.add(self.page_num * PAGE_SIZE) } as usize
    }
}

struct CacheManager {
    caches: [Option<NonNull<CachePage>>; CACHE_NUM],
    next: Option<Box<Self>>,
    is_cache_full: bool,
    is_init: bool,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self {
            caches: [None; CACHE_NUM],
            next: None,
            is_cache_full: false,
            is_init: false,
        }
    }
}

impl CacheManager {
    fn add_cache(&mut self, cache: *mut CachePage) {
        for (i, cache_iter) in self.caches.iter_mut().enumerate() {
            if cache_iter.is_none() {
                *cache_iter = NonNull::new(cache);
                if i == CACHE_NUM - 1 {
                    self.is_cache_full = true;
                }
                break;
            }
        }
    }
}

impl CacheManager {
    unsafe fn _alloc(&mut self, layout: Layout) -> *mut u8 {
        unsafe {
            if !self.is_init {
                self.is_init = true;
                self.next = Some(Box::default());
            }

            let padding = padding_bytes(layout.align());
            let alloc_size = layout.size() + padding + core::mem::size_of::<*mut CacheManager>();
            /* allocate with cache manager */
            if let Some(alloc_size) = to_objsize(alloc_size) {
                for mut cache in self.caches.into_iter().flatten() {
                    if cache.as_ref().obj_size == alloc_size
                        && let Some(ptr) = cache.as_mut().alloc_obj()
                    {
                        let obj = CacheObject { ptr };
                        obj.write_head(self as *const Self);
                        return obj.object_ptr(padding);
                    }
                }
                if !self.is_cache_full {
                    /* add a new cache */
                    let page_num = ceil_to_power_2(CACHE_OBJ_COUNT * alloc_size / PAGE_SIZE);
                    let offset = size_of::<CachePage>().div_ceil(alloc_size); // offest in n objects size
                    let cache_addr = (PAGE_SIZE
                        * (*(&raw mut BUDDY_ALLOCATOR)).alloc_pages(page_num))
                        as *mut CachePage;
                    let mut cache = CachePage {
                        page_start: cache_addr as *mut u8,
                        page_num,
                        obj_size: alloc_size,
                        obj_alloc: 0,
                        obj_free: page_num * PAGE_SIZE / alloc_size - offset,
                        obj_start: (cache_addr).byte_add(offset * alloc_size) as *mut u8,
                    };
                    cache.init(offset);
                    let ptr = cache.alloc_obj().unwrap();
                    cache_addr.write(cache);

                    let obj = CacheObject { ptr };
                    self.add_cache(cache_addr);
                    obj.write_head(self as *const Self);
                    obj.object_ptr(padding)
                } else {
                    self.next.as_mut().unwrap()._alloc(layout)
                }
            }
            /* use buddy allocator for large object */
            else {
                (PAGE_SIZE
                    * (*(&raw mut BUDDY_ALLOCATOR))
                        .alloc_pages(ceil_to_power_2(layout.size() / PAGE_SIZE)))
                    as *mut u8
            }
        }
    }
}

unsafe impl GlobalAlloc for CacheManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { (*(&raw mut GLOBAL_ALLOCATOR))._alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let padding = padding_bytes(layout.align());
        let alloc_size = layout.size() + padding + size_of::<*mut Self>();

        unsafe {
            if to_objsize(alloc_size).is_some() {
                let mgr = (ptr as *mut *mut Self)
                    .byte_sub(padding + size_of::<*mut Self>())
                    .read();

                for i in &mut (*mgr).caches {
                    if let Some(mut cache) = *i
                        && cache.as_ref().in_range(ptr)
                    {
                        cache.as_mut().free_obj(ptr);

                        /* free object cache */
                        if cache.as_ref().obj_alloc == 0 {
                            (*(&raw mut BUDDY_ALLOCATOR)).free_pages(
                                cache.as_ref().page_start as usize / PAGE_SIZE,
                                cache.as_ref().page_num,
                            );
                            *i = None;
                            (*mgr).is_cache_full = false;
                        }
                        return;
                    }
                }
            } else {
                (*(&raw mut BUDDY_ALLOCATOR)).free_pages(
                    ptr as usize / PAGE_SIZE,
                    ceil_to_power_2(layout.size() / PAGE_SIZE),
                );
            }
        }
    }
}
