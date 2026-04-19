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

use core::{arch::asm, ptr::addr_of};

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
        cpu::switch_to_s_level();
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

        task::task_init();
        trap::enable_interrupts();
    }

    rand::rand_init();
    vfs::vfs_init();
    kmsg::kmsg_init();

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
