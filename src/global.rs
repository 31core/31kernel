/*! Types for global static variables. */

use crate::mutex::Mutex;
use core::mem::MaybeUninit;

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

pub type Global<T> = Mutex<T>;
pub type GlobalUninit<T> = Mutex<MaybeUninit<T>>;
