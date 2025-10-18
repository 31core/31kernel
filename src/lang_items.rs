use alloc::format;
use core::panic::PanicInfo;

use crate::{
    kernel_wait,
    kmsg::{KMSG, KernelMessageLevel},
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        match info.location() {
            Some(location) => (*(&raw mut KMSG)).assume_init_mut().add_message(
                KernelMessageLevel::Fatal,
                &format!("{} at {}\n", info.message(), location),
            ),
            None => (*(&raw mut KMSG))
                .assume_init_mut()
                .add_message(KernelMessageLevel::Fatal, &format!("{}\n", info.message())),
        }
    }

    loop {
        kernel_wait();
    }
}
