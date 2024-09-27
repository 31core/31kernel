use crate::malloc::{ceil_to_power_2, BUDDY_ALLOCATOR};
use crate::*;
use core::alloc::GlobalAlloc;
use core::alloc::Layout;

const CACHE_NUM: usize = 1024;
const CACHE_OBJ_COUNT: usize = 512;

/** Check if a size of object can be allocated with a cache and get the cache size if so. */
fn to_objsize(size: usize) -> Option<usize> {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;
    const CACHE_SIZE: [usize; 12] = [
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

#[global_allocator]
pub static mut GLOBAL_ALLOCATOR: CacheManager = CacheManager {
    caches: [CachePage {
        objsize: 0,
        objcount: 0,
        page_num: 0,
        page_start: core::ptr::null_mut(),
        objstart: core::ptr::null_mut(),
        is_init: false,
    }; CACHE_NUM],
};

/**
 * An object allocator.
*/
#[derive(Clone, Copy)]
struct CachePage {
    /** Address of the first page. */
    page_start: *mut u8,
    page_num: usize,
    /** Must be >= `usize`. */
    objsize: usize,
    objcount: usize,
    /** Address of the first object. */
    objstart: *mut u8,
    is_init: bool,
}

impl CachePage {
    unsafe fn new(page_start: *mut u8, objsize: usize, page_num: usize) -> Self {
        Self {
            page_start,
            page_num,
            objsize,
            objcount: page_num * PAGE_SIZE / objsize,
            objstart: page_start,
            is_init: false,
        }
    }
    /** Initialize cache on pahes. */
    unsafe fn init(&mut self) {
        self.is_init = true;

        for i in 0..self.objcount - 1 {
            (self.page_start.add(i * self.objsize) as *mut usize)
                .write(self.page_start.add((i + 1) * self.objsize) as usize);
        }

        (self.page_start.add(self.page_num * PAGE_SIZE - PTR_BYTES) as *mut usize).write(0);
    }
    unsafe fn alloc_obj(&mut self) -> Option<*mut u8> {
        if !self.is_init {
            self.init();
        }
        if self.objcount == 0 {
            return None;
        }

        self.objcount -= 1;
        let mut next_ptr = self.objstart as *const usize;
        if *next_ptr == 0 {
            return Some(self.objstart);
        }

        let mut prev_ptr;
        loop {
            prev_ptr = next_ptr;
            next_ptr = *next_ptr as *const usize;

            if *next_ptr == 0 {
                (prev_ptr as *mut usize).write(0);
                return Some(next_ptr as *mut u8);
            }
        }
    }
    unsafe fn free_obj(&mut self, ptr: *mut u8) {
        if self.objcount == 0 {
            self.objcount += 1;
            self.objstart = ptr;
            (ptr as *mut usize).write(0);
            return;
        }

        self.objcount += 1;

        if (ptr as usize) < self.objstart as usize {
            self.objstart = ptr;
            (ptr as *mut usize).write(self.objstart as usize);
            return;
        }

        let mut next_ptr = self.objstart as *const usize;
        loop {
            if (next_ptr as usize) < ptr as usize && *next_ptr > ptr as usize {
                (next_ptr as *mut usize).write(ptr as usize);
                (ptr as *mut usize).write(*next_ptr);
                return;
            }
            if *next_ptr == 0 {
                (next_ptr as *mut usize).write(ptr as usize);
                (ptr as *mut usize).write(0);
                return;
            }

            next_ptr = *next_ptr as *const usize;
        }
    }
}

pub struct CacheManager {
    caches: [CachePage; CACHE_NUM],
}

impl CacheManager {
    fn add_cache(&mut self, cache: CachePage) {
        for i in &mut self.caches {
            if i.page_start.is_null() {
                *i = cache;
                break;
            }
        }
    }
}

unsafe impl GlobalAlloc for CacheManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let allocator = (self as *const Self as *mut Self).as_mut().unwrap();

        /* allocate with cache manager */
        if let Some(obj_size) = to_objsize(layout.size()) {
            for i in &mut allocator.caches {
                if i.objsize == obj_size {
                    if let Some(addr) = i.alloc_obj() {
                        return addr;
                    }
                }
            }
            let page_count = CACHE_OBJ_COUNT * obj_size / PAGE_SIZE;
            /* add a new cache */
            let mut cache = CachePage::new(
                BUDDY_ALLOCATOR.alloc_pages(page_count),
                obj_size,
                page_count,
            );
            let addr = cache.alloc_obj().unwrap();
            allocator.add_cache(cache);
            addr
        }
        /* use buddy allocator for large object */
        else {
            BUDDY_ALLOCATOR.alloc_pages(ceil_to_power_2(layout.size() / PAGE_SIZE))
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let allocator = (self as *const Self as *mut Self).as_mut().unwrap();

        if to_objsize(layout.size()).is_some() {
            for i in &mut allocator.caches {
                if (ptr as usize) >= i.page_start as usize
                    && (ptr as usize) < i.page_start.add(i.page_num * PAGE_SIZE) as usize
                {
                    i.free_obj(ptr);
                    return;
                }
            }
        } else {
            BUDDY_ALLOCATOR.free_pages(ptr, ceil_to_power_2(layout.size() / PAGE_SIZE));
        }
    }
}
