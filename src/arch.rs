/*!
 *  Architecture related code
 */

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

use core::arch::global_asm;

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv64/entry.S"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/arm64/entry.S"));
