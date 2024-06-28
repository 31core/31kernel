use core::arch::asm;

#[no_mangle]
#[link_section = ".text.trap"]
#[cfg(target_arch = "riscv64")]
pub extern "C" fn trap() {
    unsafe {
        /* set return address */
        let mut mepc: u64;
        asm!("csrr {}, mepc", out(reg) mepc);
        mepc += 4;
        asm!("csrw mepc, {}", in(reg) mepc);
        asm!("mret");
    }
}
