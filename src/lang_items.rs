use alloc::format;
use core::panic::PanicInfo;

use crate::kernel_wait;
use crate::kmsg::{KernelMessageLevel, KMSG};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        KMSG.as_mut()
            .unwrap()
            .add_message(KernelMessageLevel::Fatal, &format!("{}\n", info.message()));
    }

    loop {
        kernel_wait();
    }
}
