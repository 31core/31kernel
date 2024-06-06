use core::panic::PanicInfo;

use alloc::format;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::device::uart::ns16550::print_str(&format!("{}\n", info));
    loop {}
}
