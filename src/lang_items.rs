use alloc::format;
use core::panic::PanicInfo;

use crate::{
    kernel_wait,
    kmsg::{KMSG, KernelMessageLevel},
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        (*(&raw mut KMSG))
            .assume_init_mut()
            .add_message(KernelMessageLevel::Fatal, &format!("{}\n", info.message()));
    }

    loop {
        kernel_wait();
    }
}
