use crate::{
    kernel_wait,
    kmsg::{KMSG, KernelMessageLevel},
    lock_uinit,
};
use alloc::format;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        match info.location() {
            Some(location) => lock_uinit!(KMSG).add_message(
                KernelMessageLevel::Fatal,
                format!("{} at {}\n", info.message(), location),
            ),
            None => lock_uinit!(KMSG)
                .add_message(KernelMessageLevel::Fatal, format!("{}\n", info.message())),
        }
    }

    loop {
        kernel_wait();
    }
}
