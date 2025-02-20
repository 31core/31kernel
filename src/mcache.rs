use crate::{
    malloc::{ceil_to_power_2, BUDDY_ALLOCATOR},
    PAGE_SIZE,
};

use alloc::boxed::Box;
use core::{alloc::GlobalAlloc, alloc::Layout};

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

#[global_allocator]
static mut GLOBAL_ALLOCATOR: CacheManager = CacheManager {
    caches: [None; CACHE_NUM],
    next: None,
    is_cache_full: false,
    is_init: false,
};

/**
 * An object allocator.
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
    unsafe fn init(&mut self) {
        for i in 0..self.obj_free - 1 {
            (self.page_start.add(i * self.obj_size) as *mut usize)
                .write(self.page_start.add((i + 1) * self.obj_size) as usize);
        }

        (self
            .page_start
            .add(self.page_num * PAGE_SIZE - self.obj_size) as *mut usize)
            .write(0);
    }
    unsafe fn alloc_obj(&mut self) -> Option<*mut u8> {
        if self.obj_free == 0 {
            return None;
        }

        self.obj_free -= 1;
        self.obj_alloc += 1;
        let mut next_ptr = self.obj_start as *const usize;
        if *next_ptr == 0 {
            return Some(self.obj_start);
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
        self.obj_alloc -= 1;

        if self.obj_free == 0 {
            self.obj_free += 1;
            self.obj_start = ptr;
            (ptr as *mut usize).write(0);
            return;
        }

        self.obj_free += 1;

        if (ptr as usize) < self.obj_start as usize {
            (ptr as *mut usize).write(self.obj_start as usize);
            self.obj_start = ptr;
            return;
        }

        let mut next_ptr = self.obj_start as *const usize;
        loop {
            if (next_ptr as usize) < ptr as usize && *next_ptr > ptr as usize {
                (ptr as *mut usize).write(*next_ptr);
                (next_ptr as *mut usize).write(ptr as usize);
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

struct CacheManager {
    caches: [Option<*mut CachePage>; CACHE_NUM],
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
                *cache_iter = Some(cache);
                if i == CACHE_NUM - 1 {
                    self.is_cache_full = true;
                }
                break;
            }
        }
    }
}

unsafe impl GlobalAlloc for CacheManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !GLOBAL_ALLOCATOR.is_init {
            GLOBAL_ALLOCATOR.is_init = true;
            GLOBAL_ALLOCATOR.next = Some(Box::new(Self::default()));
        }

        /* allocate with cache manager */
        if let Some(obj_size) = to_objsize(layout.size()) {
            for cache in GLOBAL_ALLOCATOR.caches.into_iter().flatten() {
                if (*cache).obj_size == obj_size {
                    if let Some(addr) = (*cache).alloc_obj() {
                        return addr;
                    }
                }
            }
            if !GLOBAL_ALLOCATOR.is_cache_full {
                /* add a new cache */
                let page_count = ceil_to_power_2(CACHE_OBJ_COUNT * obj_size / PAGE_SIZE);
                let offset = core::mem::size_of::<CachePage>().div_ceil(obj_size);
                let cache_addr =
                    (*(&raw mut BUDDY_ALLOCATOR)).alloc_pages(page_count) as *mut CachePage;
                let mut cache = CachePage {
                    page_start: cache_addr as *mut u8,
                    page_num: page_count,
                    obj_size,
                    obj_alloc: 0,
                    obj_free: page_count * PAGE_SIZE / obj_size - offset,
                    obj_start: (cache_addr).byte_add(offset * obj_size) as *mut u8,
                };
                cache.init();
                let addr = cache.alloc_obj().unwrap();
                cache_addr.write(cache);

                (*(&raw mut GLOBAL_ALLOCATOR)).add_cache(cache_addr);
                addr
            } else {
                (*(&raw mut GLOBAL_ALLOCATOR))
                    .next
                    .as_mut()
                    .unwrap()
                    .alloc(layout)
            }
        }
        /* use buddy allocator for large object */
        else {
            (*(&raw mut BUDDY_ALLOCATOR)).alloc_pages(ceil_to_power_2(layout.size() / PAGE_SIZE))
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if to_objsize(layout.size()).is_some() {
            for i in &mut (*(&raw mut GLOBAL_ALLOCATOR)).caches {
                if let Some(cache) = *i {
                    if (ptr as usize) >= (*cache).page_start as usize
                        && (ptr as usize)
                            < (*cache).page_start.add((*cache).page_num * PAGE_SIZE) as usize
                    {
                        (*cache).free_obj(ptr);

                        /* free object cache */
                        if (*cache).obj_alloc == 0 {
                            (*(&raw mut BUDDY_ALLOCATOR))
                                .free_pages((*cache).page_start, (*cache).page_num);
                            *i = None;
                            GLOBAL_ALLOCATOR.is_cache_full = false;
                        }
                        return;
                    }
                }
            }
        } else {
            (*(&raw mut BUDDY_ALLOCATOR))
                .free_pages(ptr, ceil_to_power_2(layout.size() / PAGE_SIZE));
        }
    }
}
