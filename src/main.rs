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
mod page;
mod rand;
mod syscall;
mod task;
mod vfs;

use core::arch::asm;

extern crate alloc;

/* segments */
unsafe extern "C" {
    pub fn kernel_start();
    pub fn kernel_end();
    pub fn rodata_start();
    pub fn rodata_end();
    pub fn data_start();
    pub fn data_end();
    pub fn bss_start();
    pub fn bss_end();
    pub fn heap_start();
}

const MEM_SIZE: usize = 128 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const PTR_BYTES: usize = size_of::<usize>();

/**
 * Initialize the CPU
*/
fn cpu_init() {
    unsafe {
        #[cfg(target_arch = "riscv64")]
        {
            arch::riscv64::enable_timer();
            arch::riscv64::cpu::switch_to_s_level();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() {
    clear_bss();
    cpu_init();

    unsafe {
        (*(&raw mut buddy_allocator::BUDDY_ALLOCATOR)).init(
            heap_start as *const usize as usize / PAGE_SIZE,
            MEM_SIZE / PAGE_SIZE,
        );

        task::task_init();
    }

    rand::rand_init();
    vfs::vfs_init();
    kmsg::kmsg_init();

    panic!();
}

fn clear_bss() {
    unsafe {
        core::ptr::write_bytes(
            bss_start as *mut u8,
            0,
            bss_end as *const usize as usize - bss_start as *const usize as usize,
        );
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
