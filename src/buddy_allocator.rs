/*!
 * Buddy allocator implementation for the kernel heap.
 */

use crate::{
    global::GlobalUninit,
    mutex::Mutex,
    page::{PAGE_BITS, PAGE_SIZE, PageAllocator},
};
use core::{mem::MaybeUninit, ptr::NonNull};

const NODE_COMPATIBILITY: usize = 512;
const EXT_NODE_COMPATIBILITY: usize = 8196;
const MIN_POOL_REMAIN: usize = BUDDY_ALLOC_MAX_POW;
const BUDDY_ALLOC_MAX_POW: usize = 48 - PAGE_BITS; // for 48-bit VA
const MEM_ZONES: usize = 16;

pub static BUDDY_ALLOCATOR: GlobalUninit<BuddyAllocator> = Mutex::new(MaybeUninit::uninit());

#[derive(Debug, Default)]
/**
 * Free list node for the buddy allocator.
*/
struct FreeNode {
    page_number: usize,
    next: Option<NonNull<FreeNode>>,
}

impl FreeNode {
    fn new(page_number: usize) -> Self {
        Self {
            page_number,
            ..Default::default()
        }
    }
}

/**
 * A `FreeNode` allocator, using a linked list to manage unallocated `FreeNode`.
 */
#[derive(Debug)]
struct FreeNodePool {
    free_nodes: [FreeNode; NODE_COMPATIBILITY],
    freenode_remain: usize,
    /** The first free node in the linked list */
    free_start: Option<NonNull<FreeNode>>,
}

impl FreeNodePool {
    fn init(&mut self) {
        self.freenode_remain = NODE_COMPATIBILITY;
        self.free_start = Some(NonNull::new(self.free_nodes.as_mut_ptr()).unwrap());
        /* initialize free node linked table */
        for i in 0..NODE_COMPATIBILITY - 1 {
            let ptr = NonNull::new(&mut self.free_nodes[i + 1]);
            self.free_nodes[i].next = ptr;
        }
        self.free_nodes[NODE_COMPATIBILITY - 1].next = None;
    }
    fn alloc_node(&mut self) -> NonNull<FreeNode> {
        let new_node = self.free_start.unwrap();
        self.freenode_remain -= 1;
        self.free_start = unsafe { new_node.as_ref().next };
        new_node
    }
    /**
     * Add a node into the free nodes linked table (release a node)
     */
    fn recycle_node(&mut self, mut node: NonNull<FreeNode>) {
        unsafe { node.as_mut().next = self.free_start };
        self.free_start = Some(node);
        self.freenode_remain += 1;
    }
}

/**
 * Extended free node pool, dynamically allocated when the free node pool is running out of free nodes.
 */
#[derive(Debug)]
struct ExtendedFreeNodePool {
    free_nodes: [FreeNode; EXT_NODE_COMPATIBILITY],
    next: Option<NonNull<ExtendedFreeNodePool>>,
}

impl ExtendedFreeNodePool {
    fn init(&mut self) {
        /* initialize free node linked table */
        for i in 0..EXT_NODE_COMPATIBILITY - 1 {
            let ptr = NonNull::new(&mut self.free_nodes[i + 1] as *mut FreeNode);
            self.free_nodes[i].next = ptr;
        }
        self.free_nodes[EXT_NODE_COMPATIBILITY - 1].next = None;
    }
}

#[derive(Debug)]
struct MemoryZone {
    base: usize,
    pages: usize,
    pows: [Option<NonNull<FreeNode>>; BUDDY_ALLOC_MAX_POW],
}

impl MemoryZone {
    fn new(node_pool: &mut FreeNodePool, base: usize, mut pages: usize) -> Self {
        let mut zone = Self {
            base,
            pages,
            pows: [None; BUDDY_ALLOC_MAX_POW],
        };
        let mut current_ptr = 0;
        while pages > 0 {
            let pow = floor_to_power_2(pages);

            zone.add_node(node_pool, pow, FreeNode::new(current_ptr));

            let node_pages = 2_usize.pow(pow as u32);
            pages -= node_pages;
            current_ptr += node_pages;
        }
        zone
    }
    /** Allocate and insert a node */
    fn add_node(&mut self, node_pool: &mut FreeNodePool, pow: usize, mut node: FreeNode) {
        let new_node = node_pool.alloc_node();
        node.next = self.pows[pow];
        unsafe { new_node.write(node) };
        self.pows[pow] = Some(new_node);
    }
    /**
     * Get a free node and remove it from the free list.
     */
    fn new_node(&mut self, pow: usize) -> NonNull<FreeNode> {
        let node = self.pows[pow].unwrap();
        self.pows[pow] = unsafe { node.as_ref().next };
        node
    }
    /**
     * Allocate pages and returns the start page number, where `page_num` must be n power of 2.
     */
    fn alloc_pages(&mut self, node_pool: &mut FreeNodePool, pages_count: usize) -> Option<usize> {
        for pow in 0..BUDDY_ALLOC_MAX_POW {
            let start = self.pows[pow];

            if start.is_some() && 2_usize.pow(pow as u32) == pages_count {
                let new_node = self.new_node(pow);
                node_pool.recycle_node(new_node);
                let page_number = unsafe { new_node.as_ref().page_number };
                return Some(self.base + page_number);
            } else if start.is_some() && 2_usize.pow(pow as u32) > pages_count {
                let new_node = self.new_node(pow);
                let left_start = unsafe { new_node.as_ref().page_number };
                node_pool.recycle_node(new_node);

                for i in (0..pow).rev() {
                    let right_start = left_start + 2_usize.pow(i as u32);
                    self.add_node(node_pool, i, FreeNode::new(right_start));

                    if pages_count == 2_usize.pow(i as u32) {
                        return Some(self.base + left_start);
                    }
                }
            }
        }
        None
    }
    fn free_pages(
        &mut self,
        node_pool: &mut FreeNodePool,
        page_start: usize,
        mut pages_count: usize,
    ) {
        let mut pow_final = 0;
        let mut page_relative = page_start - self.base;
        /* insert into free list */
        for pow in 0..self.pows.len() {
            if 1 << pow == pages_count {
                pow_final = pow;
                break;
            }
        }

        let pow_start = pow_final;
        /* merge free list nodes if possible */
        'pow_loop: for pow in pow_start..BUDDY_ALLOC_MAX_POW {
            if let Some(mut current) = self.pows[pow] {
                let found_pages = 2_usize.pow(pow as u32);

                /* left node */
                if page_relative.is_multiple_of(2_usize.pow((pow + 1) as u32)) {
                    /* current node is partner node */
                    if page_relative + found_pages == unsafe { current.as_ref().page_number } {
                        self.pows[pow] = unsafe { current.as_ref().next };
                        node_pool.recycle_node(current);
                        pow_final += 1;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = unsafe { current.as_ref().next } {
                        /* next node is partner node */
                        if page_relative + found_pages == unsafe { next.as_ref().page_number } {
                            unsafe { current.as_mut().next = next.as_ref().next };
                            node_pool.recycle_node(next);
                            pow_final += 1;
                            pages_count *= 2;

                            continue 'pow_loop;
                        }
                        current = next;
                    }
                }
                /* right node */
                else {
                    /* current node is partner node */
                    if unsafe { current.as_ref().page_number } + found_pages == page_relative {
                        self.pows[pow] = unsafe { current.as_ref().next };
                        node_pool.recycle_node(current);
                        pow_final += 1;
                        page_relative -= pages_count;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = unsafe { current.as_ref().next } {
                        /* next node is is partner node */
                        if unsafe { next.as_ref().page_number } + found_pages == page_relative {
                            unsafe { current.as_mut().next = next.as_ref().next };
                            node_pool.recycle_node(next);
                            pow_final += 1;
                            page_relative -= pages_count;
                            pages_count *= 2;

                            continue 'pow_loop;
                        }
                        current = next;
                    }
                }
            } else {
                break 'pow_loop;
            }
        }

        self.add_node(node_pool, pow_final, FreeNode::new(page_relative));
    }
}

fn floor_to_power_2(mem_size: usize) -> usize {
    for pow in (0..BUDDY_ALLOC_MAX_POW).rev() {
        if mem_size >> pow == 1 {
            return pow;
        }
    }
    0
}

pub fn ceil_to_power_2(mem_size: usize) -> usize {
    let mut ceil_size = 1;
    for _ in 0..BUDDY_ALLOC_MAX_POW {
        if ceil_size >= mem_size {
            break;
        }

        ceil_size <<= 1;
    }

    ceil_size
}

/**
 * The kernel buddy allocator.
*/
#[derive(Debug)]
pub struct BuddyAllocator {
    /** Total free pages. */
    pub free: usize,
    zones: [MemoryZone; MEM_ZONES],
    zone_num: usize,
    node_pool: FreeNodePool,
    extended_pools: Option<NonNull<ExtendedFreeNodePool>>,
}

unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /** Initialize the allocator. */
    pub unsafe fn init(&mut self) {
        self.node_pool.init();
    }
    /** Add a memory zone to the allocator. */
    pub fn add_zone(&mut self, base: usize, pages: usize) {
        self.zones[self.zone_num] = MemoryZone::new(&mut self.node_pool, base, pages);
        self.zone_num += 1;
        self.free += pages;
    }
    /** Create a extended pool of free nodes. */
    fn new_extended_pool(&mut self) {
        /* freenode_remain += EXT_NODE_COMPATIBILITY before allocation to avoid recursion */
        self.node_pool.freenode_remain += EXT_NODE_COMPATIBILITY;

        let pages =
            ceil_to_power_2(core::mem::size_of::<ExtendedFreeNodePool>().div_ceil(PAGE_SIZE));
        let page_num = self.alloc_pages(pages);
        let pool = (page_num * PAGE_SIZE) as *mut ExtendedFreeNodePool;
        self.extended_pools = NonNull::new(pool);

        unsafe {
            (*pool).init();
            (*pool).next = self.extended_pools;
            (*pool).free_nodes[EXT_NODE_COMPATIBILITY - 1].next = self.node_pool.free_start;
            self.node_pool.free_start = NonNull::new(&mut (*pool).free_nodes[0]);
        }
    }
}

impl PageAllocator for BuddyAllocator {
    /**
     * Allocate pages and returns the start page number, where `page_num` must be n power of 2.
     */
    fn alloc_pages(&mut self, pages_count: usize) -> usize {
        if self.node_pool.freenode_remain <= MIN_POOL_REMAIN {
            self.new_extended_pool();
        }

        let pages_count = ceil_to_power_2(pages_count);

        self.free -= pages_count;

        for zone in &mut self.zones[..self.zone_num] {
            if let Some(page_num) = zone.alloc_pages(&mut self.node_pool, pages_count) {
                return page_num;
            }
        }

        panic!("No enough memory to allocate");
    }
    fn free_pages(&mut self, page_start: usize, pages_count: usize) {
        let pages_count = ceil_to_power_2(pages_count);

        self.free += pages_count;

        for zone in &mut self.zones[..self.zone_num] {
            if (zone.base..zone.base + zone.pages).contains(&page_start) {
                zone.free_pages(&mut self.node_pool, page_start, pages_count);
                break;
            }
        }
    }
}
