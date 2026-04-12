use super::cpu::Context;
use core::arch::global_asm;

global_asm!(include_str!("trap.S"));

#[unsafe(no_mangle)]
pub unsafe extern "C" fn el1_irq_trap_handler(_ctx: *mut Context) {}
