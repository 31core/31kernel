/*!
 * Buddy allocator implementation for the kernel heap.
 */

pub const NODE_COMPATIBILITY: usize = 8196;
const BUDDY_ALLOC_MAX_POW: usize = 64;

pub static mut BUDDY_ALLOCATOR: BuddyAllocator = BuddyAllocator {
    start: 0,
    free: 0,
    pows: [None; BUDDY_ALLOC_MAX_POW],
    free_start: None,
    free_nodes: [FreeNode {
        page_number: 0,
        next: None,
    }; NODE_COMPATIBILITY],
};

#[derive(Clone, Copy, Debug, Default)]
/**
 * Free list node for the buddy allocator.
*/
pub struct FreeNode {
    pub page_number: usize,
    pub next: Option<usize>,
}

impl FreeNode {
    pub fn new(page_number: usize) -> Self {
        Self {
            page_number,
            ..Default::default()
        }
    }
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
    /** Start page number. */
    pub start: usize,
    /** Total free pages. */
    pub free: usize,
    pub pows: [Option<usize>; 64],
    /** Recording relative address to `BuddyAllocator.start`. */
    pub free_start: Option<usize>,
    pub free_nodes: [FreeNode; NODE_COMPATIBILITY],
}

impl BuddyAllocator {
    /** Initialize the allocator. */
    pub unsafe fn init(&mut self, page_start: usize, mut pages_count: usize) {
        self.free = pages_count;
        self.start = page_start;

        fn floor_to_power_2(mem_size: usize) -> usize {
            for pow in (0..BUDDY_ALLOC_MAX_POW).rev() {
                if mem_size >> pow == 1 {
                    return pow;
                }
            }
            0
        }

        /* initialize free node linked table */
        self.free_start = Some(0);
        for (i, node) in self
            .free_nodes
            .iter_mut()
            .take(NODE_COMPATIBILITY - 1)
            .enumerate()
        {
            node.next = Some(i + 1);
        }

        let mut current_ptr = 0;
        while pages_count > 0 {
            let pow = floor_to_power_2(pages_count);

            self.add_node(pow, FreeNode::new(current_ptr));

            let node_pages = 2_usize.pow(pow as u32);
            pages_count -= node_pages;
            current_ptr += node_pages;
        }
    }

    /** Allocate and insert a node */
    fn add_node(&mut self, pow: usize, mut node: FreeNode) {
        match self.free_start {
            Some(node_start) => {
                self.free_start = self.free_nodes[node_start].next;
                node.next = self.pows[pow];
                self.pows[pow] = Some(node_start);

                self.free_nodes[node_start] = node;
            }
            None => panic!(),
        }
    }
    /**
     * Add a node into the free nodes linked table (release a node)
     */
    fn recycle_node(&mut self, node: usize) {
        self.free_nodes[node].next = self.free_start;
        self.free_start = Some(node);
    }
    /**
     * Get a free node and remove it from the free list.
     */
    fn new_node(&mut self, pow: usize) -> FreeNode {
        let node = self.free_nodes[self.pows[pow].unwrap()];
        self.pows[pow] = node.next;
        node
    }
    /**
     * Allocate pages and returns the start page number, where `page_num` must be n power of 2.
     */
    pub fn alloc_pages(&mut self, pages_count: usize) -> usize {
        assert!(pages_count.is_power_of_two());

        self.free -= pages_count;

        for pow in 0..BUDDY_ALLOC_MAX_POW {
            let start = self.pows[pow];

            if start.is_some() && 2_usize.pow(pow as u32) == pages_count {
                return self.start + self.new_node(pow).page_number;
            } else if start.is_some() && 2_usize.pow(pow as u32) > pages_count {
                let mut found_pages = 2_usize.pow(pow as u32);
                let start_page = self.new_node(pow).page_number;
                let mut new_page = start_page + found_pages;

                for i in (0..pow).rev() {
                    found_pages /= 2;
                    new_page -= found_pages;
                    self.add_node(i, FreeNode::new(new_page));

                    if found_pages == pages_count {
                        return self.start + new_page - found_pages;
                    }
                }
            }
        }

        panic!("No enough memory to allocate");
    }
    pub fn free_pages(&mut self, page_start: usize, mut pages_count: usize) {
        assert!(pages_count.is_power_of_two());

        self.free += pages_count;

        let mut pow_final = 0;
        let mut page_relative = page_start - self.start;
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
            let node = &self.pows[pow];
            if let Some(mut current_idx) = *node {
                let found_pages = 2_usize.pow(pow as u32);

                let mut current = self.free_nodes[current_idx];

                /* left node */
                if page_relative.is_multiple_of(2_usize.pow((pow + 1) as u32)) {
                    /* current node is partner node */
                    if page_relative + found_pages == current.page_number {
                        self.pows[pow] = current.next;
                        self.recycle_node(current_idx);
                        pow_final += 1;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = current.next {
                        let next_node = self.free_nodes[next];

                        /* next node is partner node */
                        if page_relative + found_pages == next_node.page_number {
                            self.free_nodes[current_idx].next = next_node.next;
                            self.recycle_node(next);
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
                        self.recycle_node(current_idx);
                        pow_final += 1;
                        page_relative -= pages_count;
                        pages_count *= 2;

                        continue 'pow_loop;
                    }

                    while let Some(next) = current.next {
                        let next_node = self.free_nodes[next];

                        /* next node is is partner node */
                        if next_node.page_number + found_pages == page_relative {
                            self.free_nodes[current_idx].next = next_node.next;
                            self.recycle_node(next);
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

        self.add_node(pow_final, FreeNode::new(page_relative));
    }
}
