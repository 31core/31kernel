#![no_std]

use core::arch::asm;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

fn disable_interrupts() {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        asm!("msr DAIFSet, #2");

        #[cfg(target_arch = "riscv64")]
        asm!("csrc sstatus, 2"); // unset SIE flag
    }
}

fn irq_save() -> u64 {
    let irq: u64;
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("mrs {}, DAIF", out(reg) irq);
        irq & (1 << 7) // I bit
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        asm!("csrr {}, sstatus", out(reg) irq);
        irq & (1 << 1) // SIE bit
    }
}

fn irq_load(irq: u64) {
    #[cfg(target_arch = "aarch64")]
    if irq == 0 {
        unsafe { asm!("msr DAIFClr, #2") };
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        asm!("csrs sstatus, {}", in(reg) irq)
    };
}

pub struct Spinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    pub fn lock(&self) -> SpinGuard<'_, T> {
        let irq = irq_save();
        disable_interrupts();
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinGuard { lock: self, irq }
    }
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

unsafe impl<T: Send> Sync for Spinlock<T> {}

pub struct SpinGuard<'a, T> {
    lock: &'a Spinlock<T>,
    irq: u64,
}

impl<'a, T> Drop for SpinGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
        irq_load(self.irq);
    }
}

impl<'a, T> Deref for SpinGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
