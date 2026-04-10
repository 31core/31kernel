use core::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    locked: AtomicBool,
    data: T,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data,
        }
    }
    pub fn lock(&mut self) -> &mut T {
        while self.locked.swap(true, Ordering::Acquire) {}
        &mut self.data
    }
    pub fn unlock(&mut self) {
        self.locked.store(false, Ordering::Release);
    }
}
