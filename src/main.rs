#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(clippy::deref_addrof)]

/// Architecture related code
mod arch;
mod buddy_allocator;
mod devfs;
/// Generic device drivers
mod device;
mod kmsg;
mod lang_items;
mod mcache;
mod page;
/// Random generators
mod rand;
mod syscall;
mod task;
mod vfs;

use alloc::{boxed::Box, string::String};
use core::{
    arch::{asm, global_asm},
    mem::MaybeUninit,
};
use devfs::DevFS;
use kmsg::{KMSG, kmsg_init};
use vfs::VirtualFileSystem;

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

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv64/entry.S"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/arm64/entry.S"));

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() {
    clear_bss();
    unsafe {
        (*(&raw mut buddy_allocator::BUDDY_ALLOCATOR))
            .init(heap_start as usize, MEM_SIZE / PAGE_SIZE);
        rand::rand_init();
        vfs::ROOT_VFS = MaybeUninit::new(VirtualFileSystem::default());
        (*(&raw mut vfs::ROOT_VFS))
            .assume_init_mut()
            .mount(Box::<DevFS>::default(), &[String::from("dev")]);

        #[cfg(target_arch = "riscv64")]
        {
            arch::riscv64::enable_timer();
            arch::riscv64::cpu::switch_to_s_level();
        }

        task::task_init();
    }

    kmsg_init();

    panic!();
}

fn clear_bss() {
    unsafe {
        core::ptr::write_bytes(
            bss_start as *mut u8,
            0,
            bss_end as usize - bss_start as usize,
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
