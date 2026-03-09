/*!
 * Generic device drivers
 */

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

pub mod uart;

pub trait CharDev {
    fn can_read() -> bool;
    fn can_write() -> bool;
    fn put_char(c: u8);
    fn get_char() -> u8;
    fn print_str(s: &str) {
        for b in s.as_bytes() {
            Self::put_char(*b);
        }
    }
    fn input_str() -> String {
        let mut input = Vec::new();
        loop {
            let c = Self::get_char();
            if c == b'\n' {
                return String::from_utf8_lossy(&input).to_string();
            }
            input.push(c);
        }
    }
}
