use crate::{
    kernel_wait,
    kmsg::{KMSG, KernelMessageLevel},
};
use alloc::format;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    match info.location() {
        Some(location) => KMSG.lock().add_message(
            None,
            KernelMessageLevel::Fatal,
            format!("{} at {}\n", info.message(), location),
        ),
        None => KMSG.lock().add_message(
            None,
            KernelMessageLevel::Fatal,
            format!("{}\n", info.message()),
        ),
    }

    loop {
        kernel_wait();
    }
}
