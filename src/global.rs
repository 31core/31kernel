/*! Types for global static variables. */

use core::mem::MaybeUninit;
use spinlock::Spinlock;

#[macro_export]
macro_rules! lock {
    ($var:tt) => {
        (*$var.get()).lock()
    };
}

#[macro_export]
macro_rules! lock_uinit {
    ($var:expr) => {
        $var.lock().assume_init_mut()
    };
}

pub type Global<T> = Spinlock<T>;
pub type GlobalUninit<T> = Spinlock<MaybeUninit<T>>;
