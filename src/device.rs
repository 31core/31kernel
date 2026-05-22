/*!
 * Generic device drivers
 */

use crate::{global::GlobalUninit, mutex::Mutex};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::mem::MaybeUninit;

pub mod uart;

pub trait CharDev {
    fn can_read(&self) -> bool;
    fn can_write(&self) -> bool;
    fn put_char(&self, c: u8);
    fn get_char(&self) -> u8;
    fn print_str(&self, s: &str) {
        for b in s.as_bytes() {
            self.put_char(*b);
        }
    }
    fn input_str(&self) -> String {
        let mut input = Vec::new();
        loop {
            let c = self.get_char();
            if c == b'\n' {
                return String::from_utf8_lossy(&input).to_string();
            }
            input.push(c);
        }
    }
}

pub static DEVICE_MGR: GlobalUninit<DeviceManager> = Mutex::new(MaybeUninit::uninit());

pub fn device_init() {
    let mut dev_mgr = DEVICE_MGR.lock();
    *dev_mgr = MaybeUninit::new(DeviceManager::default());
}

#[derive(Default)]
pub struct DeviceManager {
    pub char_devs: Vec<Box<dyn CharDev>>,
}

unsafe impl Send for DeviceManager {}

impl DeviceManager {
    pub fn register_char_dev(&mut self, dev: Box<dyn CharDev>) -> usize {
        let id = self.char_devs.len();
        self.char_devs.push(dev);
        id
    }
}
