#![no_std]
#![no_main]

mod arch;
mod devfs;
mod device;
mod kmsg;
mod lang_items;
mod malloc;
mod syscall;
mod vfs;

use core::arch::{asm, global_asm};

use alloc::{boxed::Box, string::String};
use devfs::DevFS;
use kmsg::{kmsg_init, KMSG};
use vfs::VirtualFileSystem;

extern crate alloc;

extern "C" {
    pub fn heap_start();
}

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv64/entry.S"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/arm64/entry.S"));

#[no_mangle]
pub extern "C" fn kernel_main() {
    clear_bss();
    unsafe {
        malloc::GLOBAL_ALLOCATOR.init(heap_start as usize, 128 * 1024 * 1024);
        vfs::ROOT_VFS = Some(VirtualFileSystem::default());
        vfs::ROOT_VFS
            .as_mut()
            .unwrap()
            .mount(Box::<DevFS>::default(), &[String::from("dev")]);

        #[cfg(target_arch = "riscv64")]
        {
            arch::riscv64::enable_timer();
            arch::riscv64::cpu::switch_to_s_level();
        }
    }
    kmsg_init();

    panic!();
}

fn clear_bss() {
    extern "C" {
        fn bss_start();
        fn bss_end();
    }

    unsafe {
        core::ptr::write_bytes(
            bss_start as *mut u8,
            0,
            bss_end as usize - bss_start as usize,
        );
    }
}

fn kernel_wait() {
    unsafe {
        #[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
        asm!("wfi");
        #[cfg(target_arch = "x86_64")]
        asm!("hlt");
    }
}
