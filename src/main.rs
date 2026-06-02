#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(clippy::deref_addrof)]

mod arch;
mod buddy_allocator;
mod devfs;
mod device;
mod global;
mod kmsg;
mod lang_items;
mod mcache;
mod mutex;
mod page;
mod path;
mod rand;
mod syscall;
mod task;
mod time;
mod trap;
mod vfs;

use core::{arch::asm, ptr::addr_of};
use dtb::{DeviceTree, Node, ParseError, utils::*};
use page::Paging;

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
        #[cfg(feature = "riscv_m_mode")]
        cpu::switch_to_s_level();
    }
}

/**
 * Initialize the SoC
*/
fn soc_init(dtb: &DeviceTree) {
    #[cfg(target_arch = "aarch64")]
    arch::arm64::soc_init(dtb);
    #[cfg(target_arch = "riscv64")]
    let _ = dtb;
}

/** Setup console (serial0) for kmsg output. */
fn setup_console(dtb: &DeviceTree) {
    fn setup_by_serial0_node(serial0_node: &Node) {
        use alloc::boxed::Box;
        use devfs::CHAR_DEV_MAJOR;
        use device::{
            DEVICE_MGR,
            uart::{ns16550::NS16550, pl011::PL011},
        };
        use path::Path;
        use vfs::{FileType, ROOT_VFS};

        let mut kmsg_guard = kmsg::KMSG.lock();
        let kmsg = unsafe { kmsg_guard.assume_init_mut() };

        let mut device_mgr_guard = DEVICE_MGR.lock();
        let device_mgr = unsafe { device_mgr_guard.assume_init_mut() };

        let mut vfs_guard = ROOT_VFS.lock();
        let vfs = unsafe { vfs_guard.assume_init_mut() };
        if let Some(compatible) = serial0_node.get_property("compatible")
            && check_compatible(compatible, "arm,pl011")
            && let Some(reg) = serial0_node.get_property("reg")
        {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);
            kmsg.output_handler = Some(Box::new(PL011(regs[0].0)));
            let id = device_mgr.register_char_dev(Box::new(PL011(regs[0].0)));

            vfs.get_fs_mut("/dev")
                .unwrap()
                .mknod(Path::new("tty0"), FileType::CharDev, (CHAR_DEV_MAJOR, id))
                .unwrap();
        }
        if let Some(compatible) = serial0_node.get_property("compatible")
            && check_compatible(compatible, "ns16550a")
            && let Some(reg) = serial0_node.get_property("reg")
        {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);
            kmsg.output_handler = Some(Box::new(NS16550(regs[0].0)));
            let id = device_mgr.register_char_dev(Box::new(NS16550(regs[0].0)));

            vfs.get_fs_mut("/dev")
                .unwrap()
                .mknod(Path::new("tty0"), FileType::CharDev, (CHAR_DEV_MAJOR, id))
                .unwrap();
        }
        /* map registers */
        if let Some(reg) = serial0_node.get_property("reg") {
            let regs = parse_reg(reg, serial0_node.address_cells, serial0_node.size_cells);

            unsafe {
                let mut scheduler_guard = task::SCHEDULER.lock();
                let scheduler = scheduler_guard.assume_init_mut();
                let kernel_pt = &mut scheduler.current_task_mut().page;
                for (reg_addr, reg_size) in regs {
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

fn load_dtb(dtb_addr: u64) -> Result<DeviceTree, ParseError> {
    use arch::PageMapper;
    use page::{KERNEL_PT, PAGE_SIZE};
    unsafe {
        #[cfg(target_arch = "riscv64")]
        let mut kernel_page = {
            use page::ppn_to_vpn;
            PageMapper::from_pn(ppn_to_vpn(KERNEL_PT.assume_init()) as u64)
        };
        #[cfg(target_arch = "aarch64")]
        let mut kernel_page = {
            use page::pa_to_va;
            PageMapper::from_ttbrx_el1(pa_to_va(KERNEL_PT.assume_init()) as u64)
        };

        let dtb_ptr = dtb_addr as *const u8;
        kernel_page.map_rodata(
            dtb_addr as usize / PAGE_SIZE,
            dtb_addr as usize / PAGE_SIZE,
            1,
        );
        kernel_page.refresh();
        let dtb_size = DeviceTree::detect_totalsize(dtb_ptr);
        kernel_page.map_rodata(
            dtb_addr as usize / PAGE_SIZE + 1,
            dtb_addr as usize / PAGE_SIZE + 1,
            dtb_size.div_ceil(PAGE_SIZE) - 1,
        );
        kernel_page.refresh();
        let dtb_bytes = core::slice::from_raw_parts(dtb_ptr, dtb_size);
        DeviceTree::parse(dtb_bytes)
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

    page::kernel_pt_init();
    let dtb = load_dtb(dtb_addr);

    if let Ok(dtb) = &dtb {
        soc_init(dtb);
    }

    kmsg::kmsg_init();
    task::task_init();
    rand::rand_init();
    vfs::vfs_init();
    device::device_init();
    if let Ok(dtb) = &dtb {
        setup_console(dtb);
    }

    unsafe { trap::enable_interrupts() };

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
