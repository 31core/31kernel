#![no_std]
#![no_main]

mod arch;
mod device;
mod lang_items;
mod malloc;
mod syscall;

use core::arch::global_asm;
use malloc::*;

extern crate alloc;

#[global_allocator]
static mut GLOBAL: Allocator = Allocator {
    start: 0,
    free: 0,
    pows: [None; 64],
    free_start: None,
    free_nodes: [FreeNode {
        addr: 0,
        next: None,
    }; NODE_SIZE],
};

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv64/entry.S"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/arm64/entry.S"));

#[no_mangle]
pub extern "C" fn kernel_main() {
    clear_bss();
    unsafe {
        GLOBAL.init(128 * 1024 * 1024);
    }

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
