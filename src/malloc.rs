use core::alloc::GlobalAlloc;
use core::alloc::Layout;

extern "C" {
    pub fn heap_start();
}

pub const NODE_COMPATIBILITY: usize = 8196;

#[global_allocator]
pub static mut GLOBAL_ALLOCATOR: Allocator = Allocator {
    start: 0,
    free: 0,
    pows: [None; 64],
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

fn ceil_to_power_2(mem_size: usize) -> usize {
    let mut ceil_size = 1;
    for _ in 0..64 {
        if ceil_size >= mem_size {
            break;
        }

        ceil_size <<= 1;
    }

    ceil_size
}

/**
 * The kernel buddy allocator.
 *
 * Fields:
 * * `start`: Start address.
 * * `free_lists`: Recording relative address to `Allocator.start`.
*/
#[derive(Debug)]
pub struct Allocator {
    pub start: usize,
    pub free: usize,
    pub pows: [Option<usize>; 64],
    pub free_start: Option<usize>,
    pub free_nodes: [FreeNode; NODE_COMPATIBILITY],
}

impl Allocator {
    /** Initialize Allocator. */
    pub unsafe fn init(&mut self, mut mem_size: usize) {
        self.free = mem_size;
        self.start = heap_start as usize;

        fn floor_to_power_2(mem_size: usize) -> usize {
            for pow in (0..64).rev() {
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
            let pow = floor_to_power_2(mem_size);

            self.add_node(pow, FreeNode::new(current_ptr));

            let node_size = 2_usize.pow(pow as u32);
            mem_size -= node_size;
            current_ptr += node_size;

            if mem_size == 0 {
                break;
            }
        }
    }

    /** Allocate and insert a node */
    fn add_node(&mut self, pow: usize, mut node: FreeNode) {
        match self.free_start {
            Some(node_start) => {
                node.next = self.pows[pow];
                let next = self.free_nodes[node_start].next;
                self.pows[pow] = Some(node_start);
                self.free_nodes[node_start] = node;
                self.free_start = next;
            }
            None => panic!(),
        }
    }
    /** Add a node into the free nodes linked table */
    fn add_free_node(&mut self, node: usize) {
        self.free_nodes[node].next = self.free_start;
        self.free_start = Some(node);
    }
    fn pop_node(&mut self, pow: usize) -> FreeNode {
        let node = self.free_nodes[self.pows[pow].unwrap()];
        self.pows[pow] = node.next;
        node
    }
}

unsafe impl Sync for Allocator {}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mem_size = ceil_to_power_2(layout.size());
        let allocator = (self as *const Self as *mut Self).as_mut().unwrap();
        allocator.free -= mem_size;

        for (pow, start) in self.pows.iter().enumerate() {
            let found_mem_size = 2_usize.pow(pow as u32);
            if start.is_some() && found_mem_size == mem_size {
                return (self.start + allocator.pop_node(pow).addr) as *mut u8;
            } else if start.is_some() && found_mem_size > mem_size {
                let mut found_size = found_mem_size;
                let start_addr = allocator.pop_node(pow).addr;
                let mut new_addr = start_addr + found_mem_size;

                for i in 1..pow + 1 {
                    found_size /= 2;
                    new_addr -= found_size;
                    allocator.add_node(pow - i, FreeNode::new(new_addr));

                    if found_size == mem_size {
                        return (self.start + new_addr - found_size) as *mut u8;
                    }
                }
            }
        }

        // NULL pointer
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut mem_size = ceil_to_power_2(layout.size());
        let allocator = (self as *const Self as *mut Self).as_mut().unwrap();
        allocator.free += mem_size;

        let mut power_psh = 0;
        let mut ptr_psh = ptr as usize - self.start;
        /* insert into free list */
        for pow in 0..self.pows.len() {
            if 2_usize.pow(pow as u32) == mem_size {
                power_psh = pow;
            }
        }

        /* merge free list nodes if possible */
        for (pow, node) in self.pows.iter().enumerate() {
            if let Some(node_start) = *node {
                let found_mem_size = 2_usize.pow(pow as u32);
                if found_mem_size == mem_size {
                    /* left node */
                    if ptr_psh % 2 * found_mem_size == 0 {
                        let mut current = allocator.free_nodes[node_start];

                        /* found partner node */
                        if ptr as usize + found_mem_size == current.addr {
                            allocator.pows[pow] = current.next;
                            allocator.add_free_node(node_start);
                            power_psh = pow + 1;

                            mem_size *= 2;
                            continue;
                        }

                        while let Some(next) = current.next {
                            let next_node = allocator.free_nodes[next];

                            /* found partner node */
                            if ptr as usize + found_mem_size == next_node.addr {
                                current.next = next_node.next;
                                allocator.add_free_node(current.next.unwrap());
                                power_psh = pow + 1;

                                mem_size *= 2;
                                break;
                            }
                            current = next_node;
                        }
                    }
                    /* right node */
                    else {
                        let mut current = allocator.free_nodes[node_start];

                        /* found partner node */
                        if current.addr + found_mem_size == ptr as usize {
                            allocator.pows[pow] = current.next;
                            allocator.add_free_node(node_start);
                            power_psh = pow + 1;
                            ptr_psh -= mem_size;

                            mem_size *= 2;
                            continue;
                        }

                        while let Some(next) = current.next {
                            let next_node = allocator.free_nodes[next];

                            /* found partner node */
                            if next_node.addr + found_mem_size == ptr as usize {
                                current.next = next_node.next;
                                allocator.add_free_node(current.next.unwrap());
                                power_psh = pow + 1;
                                ptr_psh -= mem_size;

                                mem_size *= 2;
                                break;
                            }
                            current = next_node;
                        }
                    }
                }
            }
        }

        allocator.add_node(power_psh, FreeNode::new(power_psh));
    }
}
