//! Kernel debug message

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};
use core::{fmt::Display, mem::MaybeUninit};

pub static mut KMSG: MaybeUninit<KernelMessage> = MaybeUninit::uninit();

#[macro_export]
macro_rules! printk_error {
    ($($arg: tt)*) => {
        {
            let kmsg = unsafe { (*(&raw mut $crate::kmsg::KMSG)).assume_init_mut() };
            kmsg.error(&alloc::format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! printk_warning {
    ($($arg: tt)*) => {
        {
            let kmsg = unsafe { (*(&raw mut $crate::kmsg::KMSG)).assume_init_mut() };
            kmsg.warning(&alloc::format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! printk {
    ($($arg: tt)*) => {
        {
            let kmsg = unsafe { (*(&raw mut $crate::kmsg::KMSG)).assume_init_mut() };
            kmsg.debug(&alloc::format!($($arg)*));
        }
    };
}

pub fn kmsg_init() {
    unsafe {
        KMSG = MaybeUninit::new(KernelMessage::default());
    }
}

#[derive(Default)]
pub enum KernelMessageLevel {
    /** The kernel has met critical error, usually on kernel panic. */
    Fatal,
    /** Error but not critical. */
    Error,
    /** Warning message but does no effect on running. */
    Warning,
    #[default]
    /** Regular debug message or kernel log. */
    Debug,
}

#[derive(Default)]
pub struct KernelMessageEntry {
    pub level: KernelMessageLevel,
    pub time: u64,
    pub message: String,
}

impl KernelMessageEntry {
    pub fn new(time: u64, level: KernelMessageLevel, msg: &str) -> Self {
        Self {
            level,
            time,
            message: msg.to_owned(),
        }
    }
}

impl Display for KernelMessageEntry {
    /** Fromat a message into `[ttttt:tttttt] xxxxxx` */
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[{:5}.{:06}] {}",
            self.time / 1000000,
            self.time % 1000000,
            self.message
        )
    }
}

#[derive(Default)]
pub struct KernelMessage {
    pub msgs: Vec<KernelMessageEntry>,
    pub output_handler: Option<fn(&str)>,
}

impl KernelMessage {
    pub fn fatal(&mut self, msg: &str) {
        self.add_message(KernelMessageLevel::Fatal, msg);
    }
    pub fn error(&mut self, msg: &str) {
        self.add_message(KernelMessageLevel::Error, msg);
    }
    pub fn warning(&mut self, msg: &str) {
        self.add_message(KernelMessageLevel::Warning, msg);
    }
    pub fn debug(&mut self, msg: &str) {
        self.add_message(KernelMessageLevel::Debug, msg);
    }
    pub fn add_message(&mut self, level: KernelMessageLevel, msg: &str) {
        #[cfg(target_arch = "riscv64")]
        let time = crate::arch::riscv64::get_sys_time();
        #[cfg(target_arch = "aarch64")]
        let time = 0;
        self.msgs.push(KernelMessageEntry::new(time, level, msg));

        if let Some(output_fn) = self.output_handler {
            output_fn(&self.msgs.last().unwrap().to_string());
        }
    }
}
