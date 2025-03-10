use crate::PAGE_SIZE;

macro_rules! power_2 {
    ($pow:expr) => {
        1 << $pow
    };
}

pub const NODE_COMPATIBILITY: usize = 8196;
const BUDDY_ALLOC_MAX_POW: usize = 64;

pub static mut BUDDY_ALLOCATOR: BuddyAllocator = BuddyAllocator {
    start: 0,
    free: 0,
    pows: [None; BUDDY_ALLOC_MAX_POW],
    free_start: None,
    free_nodes: [FreeNode {
        addr: 0,
        next: None,
    }; NODE_COMPATIBILITY],
};

#[derive(Clone, Copy, Debug, Default)]
/**
 * Free list node for the buddy allocator.
*/
pub struct FreeNode {
    pub addr: usize,
    pub next: Option<usize>,
}

impl FreeNode {
    pub fn new(addr: usize) -> Self {
        Self {
            addr,
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
    /** Start address. */
    pub start: usize,
    /** Total free pages. */
    pub free: usize,
    pub pows: [Option<usize>; 64],
    /** Recording relative address to `BuddyAllocator.start`. */
    pub free_start: Option<usize>,
    pub free_nodes: [FreeNode; NODE_COMPATIBILITY],
}

impl BuddyAllocator {
    /** Initialize Allocator. */
    pub unsafe fn init(&mut self, mem_start: usize, mut page_num: usize) {
        self.free = page_num;
        self.start = mem_start;

        fn floor_to_power_2(mem_size: usize) -> usize {
            for pow in (0..BUDDY_ALLOC_MAX_POW).rev() {
                if (mem_size >> pow) & 1 == 1 {
                    return pow;
                }
            }

            0
        }

        /* initialize free node linked table */
        self.free_start = Some(0);
        for (i, node) in self.free_nodes.iter_mut().enumerate() {
            if i < NODE_COMPATIBILITY - 1 {
                *node = FreeNode {
                    addr: 0,
                    next: Some(i + 1),
                };
            } else {
                *node = FreeNode {
                    addr: 0,
                    next: None,
                };
            }
        }

        let mut current_ptr = 0;
        loop {
            let pow = floor_to_power_2(page_num);

            self.add_node(pow, FreeNode::new(current_ptr));

            let node_size = power_2!(pow);
            page_num -= node_size;
            current_ptr += PAGE_SIZE * node_size;

            if page_num == 0 {
                break;
            }
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
    /** Add a node into the free nodes linked table */
    fn add_free_node(&mut self, node: usize) {
        self.free_nodes[node].next = self.free_start;
        self.free_start = Some(node);
    }
    /**
     * Get a free node and remove it from the free list.
     */
    fn pop_node(&mut self, pow: usize) -> FreeNode {
        let node = self.free_nodes[self.pows[pow].unwrap()];
        self.pows[pow] = node.next;
        node
    }
    /**
     * Allocate pages, where `page_num` must be n power of 2.
     */
    pub fn alloc_pages(&mut self, page_num: usize) -> *mut u8 {
        self.free -= page_num;

        for pow in 0..BUDDY_ALLOC_MAX_POW {
            let start = self.pows[pow];
            let found_page_num = power_2!(pow);
            if start.is_some() && found_page_num == page_num {
                return (self.start + self.pop_node(pow).addr) as *mut u8;
            } else if start.is_some() && found_page_num > page_num {
                let mut found_size = found_page_num;
                let start_addr = self.pop_node(pow).addr;
                let mut new_addr = start_addr + PAGE_SIZE * found_page_num;

                for i in 1..pow + 1 {
                    found_size /= 2;
                    new_addr -= PAGE_SIZE * found_size;
                    self.add_node(pow - i, FreeNode::new(new_addr));

                    if found_size == page_num {
                        return (self.start + new_addr - PAGE_SIZE * found_size) as *mut u8;
                    }
                }
            }
        }

        // NULL pointer
        core::ptr::null_mut()
    }
    pub fn free_pages(&mut self, ptr: *mut u8, mut page_num: usize) {
        let mut power_psh = 0;
        let mut ptr_psh = ptr as usize - self.start;
        /* insert into free list */
        for pow in 0..self.pows.len() {
            if 1 << pow == page_num {
                power_psh = pow;
            }
        }

        /* merge free list nodes if possible */
        for pow in 0..BUDDY_ALLOC_MAX_POW {
            let node = &self.pows[pow];
            if let Some(node_start) = *node {
                let found_mem_size = power_2!(pow);
                if found_mem_size == page_num {
                    let mut current = self.free_nodes[node_start];

                    /* left node */
                    if ptr_psh % 2 * found_mem_size == 0 {
                        /* found partner node */
                        if ptr as usize + found_mem_size == current.addr {
                            self.pows[pow] = current.next;
                            self.add_free_node(node_start);
                            power_psh = pow + 1;

                            page_num *= 2;
                            continue;
                        }

                        while let Some(next) = current.next {
                            let next_node = self.free_nodes[next];

                            /* found partner node */
                            if ptr as usize + found_mem_size == next_node.addr {
                                current.next = next_node.next;
                                self.add_free_node(next);
                                power_psh = pow + 1;

                                page_num *= 2;
                                break;
                            }
                            current = next_node;
                        }
                    }
                    /* right node */
                    else {
                        /* found partner node */
                        if current.addr + found_mem_size == ptr as usize {
                            self.pows[pow] = current.next;
                            self.add_free_node(node_start);
                            power_psh = pow + 1;
                            ptr_psh -= page_num;

                            page_num *= 2;
                            continue;
                        }

                        while let Some(next) = current.next {
                            let next_node = self.free_nodes[next];

                            /* found partner node */
                            if next_node.addr + found_mem_size == ptr as usize {
                                current.next = next_node.next;
                                self.add_free_node(next);
                                power_psh = pow + 1;
                                ptr_psh -= page_num;

                                page_num *= 2;
                                break;
                            }
                            current = next_node;
                        }
                    }
                }
            }
        }

        self.add_node(power_psh, FreeNode::new(ptr_psh));
    }
}
