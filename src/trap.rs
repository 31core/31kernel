#[inline(always)]
pub unsafe fn enable_interrupts() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        crate::arch::riscv64::enable_interrupts()
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        crate::arch::arm64::enable_interrupts()
    };
}

#[inline(always)]
pub unsafe fn disable_interrupts() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        crate::arch::riscv64::disable_interrupts()
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        crate::arch::arm64::disable_interrupts()
    };
}

pub unsafe fn trap_stack_init(trap_stack: usize) {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        use crate::PAGE_SIZE;
        use core::arch::asm;

        asm!("csrw sscratch, {}", in(reg) (trap_stack + 16) * PAGE_SIZE);
    }
}
