use super::cpu::Context;
use crate::{
    arch::riscv64::{page::MODE_SV39, *},
    page::KERNEL_PT,
    task::SCHEDULER,
};
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
pub unsafe extern "C" fn strap_handler(ctx: *mut Context) {
    unsafe { super::disable_interrupts() };
    let mut scause: u64;
    unsafe { asm!("csrr {}, scause", out(reg) scause) };

    if scause == SCAUSE_ECALL_U || scause == SCAUSE_ECALL_S {
        unsafe { (*ctx).epc += 4 }; // set return address
    } else if scause == SCAUSE_TIMER_S {
        crate::time::timer();
        super::set_timer(super::TIMER_INTERVAL);

        let kernel_ppn = unsafe { (*(&raw mut KERNEL_PT)).assume_init() as u64 };
        let satp = kernel_ppn | (MODE_SV39 << 60);
        unsafe {
            asm!("csrw satp, {}", in(reg) satp);
            asm!("sfence.vma");
        }

        let scheduler = unsafe { (*(&raw mut SCHEDULER)).assume_init_mut() };
        scheduler.current_task_mut().context = unsafe { ctx.read() };
        scheduler.schedule();
        let next_task = scheduler.current_task();
        let next_ctx = next_task.context.clone();
        unsafe { ctx.write(next_ctx) };

        unsafe {
            next_task.page.switch_to();
            asm!("sfence.vma");
        }
    }
    unsafe { super::enable_interrupts() };
}
