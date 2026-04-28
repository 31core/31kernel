#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(clippy::deref_addrof)]

mod arch;
mod buddy_allocator;
mod devfs;
mod device;
mod kmsg;
mod lang_items;
mod mcache;
mod mutex;
mod page;
mod rand;
mod syscall;
mod task;
mod time;
mod trap;
mod vfs;

use alloc::boxed::Box;
use core::{arch::asm, ptr::addr_of};
use dtb::{DeviceTree, Node, utils::*};

extern crate alloc;

/* segments from linker script */
unsafe extern "C" {
    #[link_name = "rodata_start"]
    static RODATA_START: u8;
    #[link_name = "rodata_end"]
    static RODATA_END: u8;
    #[link_name = "data_start"]
    static DATA_START: u8;
    #[link_name = "data_end"]
    static DATA_END: u8;
    #[link_name = "bss_start"]
    static BSS_START: u8;
    #[link_name = "bss_end"]
    static BSS_END: u8;
    #[link_name = "kernel_start"]
    static KERNEL_START: u8;
    #[link_name = "kernel_end"]
    static KERNEL_END: u8;
    #[link_name = "heap_start"]
    static HEAP_START: u8;
}

macro_rules! static_mut {
    ($var:expr) => {
        #[allow(unused_unsafe)]
        unsafe {
            (*(&raw mut $var)).assume_init_mut()
        }
    };
}

const MEM_SIZE: usize = 128 * 1024 * 1024;
const STACK_SIZE: usize = 64 * 4096;
const PTR_BYTES: usize = size_of::<usize>();

/**
 * Initialize the CPU
*/
fn cpu_init() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        arch::arm64::cpu::cpu_init();
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        use arch::riscv64::*;

        cpu::cpu_init();
        enable_timer();
        cpu::switch_to_s_level();
    }
}

/**
 * Initialize the SoC
*/
fn soc_init(dtb: &DeviceTree) {
    #[cfg(target_arch = "aarch64")]
    arch::arm64::soc_init(dtb);
}

/** Setup console (serial0) for kmsg output. */
fn setup_console(dtb: &DeviceTree) {
    fn setup_by_serial0_node(serial0_node: &Node) {
        use device::uart::{ns16550::NS16550, pl011::PL011};
        let kmsg = unsafe { (*(&raw mut kmsg::KMSG)).assume_init_mut() };
        if let Some(compatible) = serial0_node.get_property("compatible")
            && check_compatible(compatible, "arm,pl011")
            && let Some(reg) = serial0_node.get_property("reg")
        {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);
            kmsg.output_handler = Some(Box::new(PL011(regs[0].0)));
        }
        if let Some(compatible) = serial0_node.get_property("compatible")
            && check_compatible(compatible, "ns16550a")
            && let Some(reg) = serial0_node.get_property("reg")
        {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);
            kmsg.output_handler = Some(Box::new(NS16550(regs[0].0)));
        }
        /* map registers */
        if let Some(reg) = serial0_node.get_property("reg") {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);

            let kernel_pt = &mut static_mut!(task::SCHEDULER).current_task_mut().page;
            for (reg_addr, reg_size) in regs {
                unsafe {
                    kernel_pt.map_data(
                        reg_addr as usize / page::PAGE_SIZE,
                        reg_addr as usize / page::PAGE_SIZE,
                        (reg_size as usize).div_ceil(page::PAGE_SIZE),
                    );
                }
            }
        }
    }
    for node in &dtb.root.child_nodes {
        if node_name(&node.name) == "serial0" {
            setup_by_serial0_node(node);
            return;
        } else if node.name == "aliases"
            && let Some(serial0) = node.get_property("serial0")
        {
            let node = node_by_alias(&dtb.root, serial0).unwrap();
            setup_by_serial0_node(node);
            return;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(dtb_addr: u64) -> ! {
    clear_bss();
    cpu_init();
    unsafe {
        use page::PAGE_SIZE;
        buddy_allocator::init(
            addr_of!(HEAP_START) as usize / PAGE_SIZE,
            MEM_SIZE / PAGE_SIZE,
        );
    }

    let dtb = unsafe {
        let dtb_ptr = dtb_addr as *const u8;
        let dtb_size = DeviceTree::detect_totalsize(dtb_ptr);
        let dtb_bytes = core::slice::from_raw_parts(dtb_ptr, dtb_size);
        DeviceTree::parse(dtb_bytes)
    };

    page::kernel_pt_init();
    if let Ok(dtb) = &dtb {
        soc_init(&dtb);
    }
    unsafe {
        task::task_init();
        trap::enable_interrupts();
    }

    rand::rand_init();
    vfs::vfs_init();
    kmsg::kmsg_init();
    if let Ok(dtb) = &dtb {
        setup_console(&dtb);
    }

    panic!();
}

fn clear_bss() {
    let bss_start = unsafe { addr_of!(BSS_START).add(STACK_SIZE) } as usize;
    let bss_end = addr_of!(BSS_END) as usize;
    unsafe {
        core::ptr::write_bytes(bss_start as *mut u8, 0, bss_end - bss_start);
    }
}

/**
 * Do cpu idle
*/
fn kernel_wait() {
    unsafe {
        #[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
        asm!("wfi");
        #[cfg(target_arch = "x86_64")]
        asm!("hlt");
    }
}
