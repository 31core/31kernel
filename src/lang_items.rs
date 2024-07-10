use core::panic::PanicInfo;

use alloc::format;

use crate::{kernel_wait, printk};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    printk!("{}\n", info);
    loop {
        kernel_wait();
    }
}
