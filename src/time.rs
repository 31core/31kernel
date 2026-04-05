/**
 * Get current timestamp in nano-second.
 */
pub fn get_sys_time() -> u64 {
    #[cfg(target_arch = "riscv64")]
    return crate::arch::riscv64::get_sys_time() * 100; // for qemu's 10Mhz clock
    #[cfg(target_arch = "aarch64")]
    return crate::arch::arm64::get_sys_time();
}

/**
 * Called by trap.
 */
pub fn timer() {}
