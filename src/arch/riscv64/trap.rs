use super::cpu::Context;
use crate::arch::riscv64::*;
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));

const INTERRUPT_FLAG: u64 = 1 << 63;

const MCAUSE_ECALL_U: u64 = 8;
const MCAUSE_ECALL_S: u64 = 9;
const MCAUSE_ECALL_M: u64 = 11;

const SCAUSE_TIMER_S: u64 = 5 | INTERRUPT_FLAG;
const SCAUSE_ECALL_U: u64 = 8;
const SCAUSE_ECALL_S: u64 = 9;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mtrap_handler(ctx: &mut Context) -> &mut Context {
    let mut mcause: u64;
    unsafe { asm!("csrr {}, mcause", out(reg) mcause) };

    if mcause == MCAUSE_ECALL_M || mcause == MCAUSE_ECALL_S || mcause == MCAUSE_ECALL_U {
        unsafe { mepc_w(mepc_r() + 4) }; // set return address
    }
    ctx
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strap_handler(ctx: &mut Context) -> &mut Context {
    let mut scause: u64;
    unsafe { asm!("csrr {}, scause", out(reg) scause) };

    if scause == SCAUSE_ECALL_U || scause == SCAUSE_ECALL_S {
        unsafe { sepc_w(sepc_r() + 4) }; // set return address
    } else if scause == SCAUSE_TIMER_S {
        super::set_timer(super::TIMER_INTERVAL);
    }
    ctx
}
