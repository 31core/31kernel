use super::cpu::Context;
use core::arch::{asm, global_asm};

global_asm!(include_str!("trap.S"));

const MCAUSE_TIMER_S: u64 = 5;
const MCAUSE_TIMER_M: u64 = 7;
const MCAUSE_ECALL_U: u64 = 8;
const MCAUSE_ECALL_S: u64 = 9;
const MCAUSE_ECALL_M: u64 = 11;

#[no_mangle]
#[link_section = ".text.trap"]
#[cfg(target_arch = "riscv64")]
pub unsafe extern "C" fn trap_handler(ctx: &mut Context) -> &mut Context {
    let mut mcause: u64;
    asm!("csrr {}, mcause", out(reg) mcause);
    let ecode = mcause & !(1 << 63);

    if ecode == MCAUSE_ECALL_M || ecode == MCAUSE_ECALL_S || ecode == MCAUSE_ECALL_U {
        /* set return address */
        let mut mepc: u64;
        asm!("csrr {}, mepc", out(reg) mepc);
        mepc += 4;
        asm!("csrw mepc, {}", in(reg) mepc);
    } else if ecode == MCAUSE_TIMER_M || ecode == MCAUSE_TIMER_S {
        super::set_timer(super::TIMER_INTERVAL);
    }
    ctx
}
