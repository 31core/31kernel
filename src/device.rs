/*!
 * Generic device drivers
 */

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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
