use core::arch::asm;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

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

pub struct Mutex<T> {
    locked: AtomicBool,
    data: SyncUnsafeCell<T>,
}

impl<T> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: SyncUnsafeCell::new(data),
        }
    }
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let irq = irq_save();
        unsafe { crate::trap::disable_interrupts() };
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        MutexGuard { mutex: self, irq }
    }
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    irq: u64,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
        irq_load(self.irq);
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

#[repr(transparent)]
pub struct SyncUnsafeCell<T> {
    inner: UnsafeCell<T>,
}

impl<T> SyncUnsafeCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }
    pub fn get(&self) -> *mut T {
        self.inner.get()
    }
}

unsafe impl<T: Send> Sync for SyncUnsafeCell<T> {}
