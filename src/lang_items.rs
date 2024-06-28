use core::panic::PanicInfo;

use alloc::format;

use crate::{kernel_wait, KMSG};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        KMSG.as_mut().unwrap().add_message(&format!("{}\n", info));
    }
    loop {
        kernel_wait();
    }
}
