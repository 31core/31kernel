/*!
 * Buddy allocator implementation for the kernel heap.
 */

use crate::{global::GlobalUninit, mutex::Mutex, page::PageAllocator};
use core::mem::MaybeUninit;

const NODE_COMPATIBILITY: usize = 8196;
const BUDDY_ALLOC_MAX_POW: usize = 36; // for 48-bit VA
const MEM_ZONES: usize = 16;

pub static BUDDY_ALLOCATOR: GlobalUninit<BuddyAllocator> = Mutex::new(MaybeUninit::uninit());

#[derive(Debug, Default)]
/**
 * Free list node for the buddy allocator.
*/
struct FreeNode {
    page_number: usize,
    next: Option<usize>,
}

impl FreeNode {
    fn new(page_number: usize) -> Self {
        Self {
            page_number,
            ..Default::default()
        }
    }
}

#[derive(Debug)]
struct FreeNodePool {
    free_nodes: [FreeNode; NODE_COMPATIBILITY],
    /** The first free node in the linked list */
    free_start: Option<usize>,
}

impl FreeNodePool {
    fn init(&mut self) {
        self.free_start = Some(0);
        /* initialize free node linked table */
        for (i, node) in self
            .free_nodes
            .iter_mut()
            .take(NODE_COMPATIBILITY - 1)
            .enumerate()
        {
            node.next = Some(i + 1);
        }
    }
    /**
     * Add a node into the free nodes linked table (release a node)
     */
    fn recycle_node(&mut self, node: usize) {
        self.free_nodes[node].next = self.free_start;
        self.free_start = Some(node);
    }
}

#[derive(Debug)]
struct MomoryZone {
    base: usize,
    pages: usize,
    pows: [Option<usize>; BUDDY_ALLOC_MAX_POW],
}

impl MomoryZone {
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
        match node_pool.free_start {
            Some(node_start) => {
                node_pool.free_start = node_pool.free_nodes[node_start].next;
                node.next = self.pows[pow];
                self.pows[pow] = Some(node_start);

                node_pool.free_nodes[node_start] = node;
            }
            None => panic!(),
        }
    }

    /**
     * Get a free node and remove it from the free list.
     */
    fn new_node<'a>(&mut self, node_pool: &'a mut FreeNodePool, pow: usize) -> &'a FreeNode {
        let node = &node_pool.free_nodes[self.pows[pow].unwrap()];
        self.pows[pow] = node.next;
        node
    }
    /**
     * Allocate pages and returns the start page number, where `page_num` must be n power of 2.
     */
    fn alloc_pages(&mut self, node_pool: &mut FreeNodePool, pages_count: usize) -> Option<usize> {
        for pow in 0..BUDDY_ALLOC_MAX_POW {
            let start = self.pows[pow];

            if start.is_some() && 2_usize.pow(pow as u32) == pages_count {
                return Some(self.base + self.new_node(node_pool, pow).page_number);
            } else if start.is_some() && 2_usize.pow(pow as u32) > pages_count {
                let left_start = self.new_node(node_pool, pow).page_number;

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
            if let Some(mut current_idx) = self.pows[pow] {
                let found_pages = 2_usize.pow(pow as u32);

                let mut current = &node_pool.free_nodes[current_idx];

                /* left node */
                if page_relative.is_multiple_of(2_usize.pow((pow + 1) as u32)) {
                    /* current node is partner node */
                    if page_relative + found_pages == current.page_number {
                        self.pows[pow] = current.next;
                        node_pool.recycle_node(current_idx);
                        pow_final += 1;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = current.next {
                        let next_node = &node_pool.free_nodes[next];

                        /* next node is partner node */
                        if page_relative + found_pages == next_node.page_number {
                            node_pool.free_nodes[current_idx].next = next_node.next;
                            node_pool.recycle_node(next);
                            pow_final += 1;
                            pages_count *= 2;

                            continue 'pow_loop;
                        }
                        current = next_node;
                        current_idx = next;
                    }
                }
                /* right node */
                else {
                    /* current node is partner node */
                    if current.page_number + found_pages == page_relative {
                        self.pows[pow] = current.next;
                        node_pool.recycle_node(current_idx);
                        pow_final += 1;
                        page_relative -= pages_count;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = current.next {
                        let next_node = &node_pool.free_nodes[next];

                        /* next node is is partner node */
                        if next_node.page_number + found_pages == page_relative {
                            node_pool.free_nodes[current_idx].next = next_node.next;
                            node_pool.recycle_node(next);
                            pow_final += 1;
                            page_relative -= pages_count;
                            pages_count *= 2;

                            continue 'pow_loop;
                        }
                        current = next_node;
                        current_idx = next;
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
    zones: [MomoryZone; MEM_ZONES],
    zone_num: usize,
    node_pool: FreeNodePool,
}

impl BuddyAllocator {
    /** Initialize the allocator. */
    pub unsafe fn init(&mut self) {
        self.node_pool.init();
    }
    pub fn add_zone(&mut self, base: usize, pages: usize) {
        self.zones[self.zone_num] = MomoryZone::new(&mut self.node_pool, base, pages);
        self.zone_num += 1;
        self.free += pages;
    }
}

impl PageAllocator for BuddyAllocator {
    /**
     * Allocate pages and returns the start page number, where `page_num` must be n power of 2.
     */
    fn alloc_pages(&mut self, pages_count: usize) -> usize {
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
