/*!
 * Kernel debug message.
 */

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{
    fmt::Result as FmtResult,
    fmt::{Display, Formatter},
    mem::MaybeUninit,
};

pub static mut KMSG: MaybeUninit<KernelMessage> = MaybeUninit::uninit();

#[macro_export]
macro_rules! printk_error {
    ($($arg:tt)*) => {
        {
            #[allow(unused_unsafe)]
            let kmsg = unsafe { (*(&raw mut $crate::kmsg::KMSG)).assume_init_mut() };
            kmsg.error(&alloc::format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! printk_warning {
    ($($arg:tt)*) => {
        {
            #[allow(unused_unsafe)]
            let kmsg = unsafe { (*(&raw mut $crate::kmsg::KMSG)).assume_init_mut() };
            kmsg.warning(&alloc::format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {
        {
            #[allow(unused_unsafe)]
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
    pub fn new<S>(time: u64, level: KernelMessageLevel, msg: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            level,
            time,
            message: msg.into(),
        }
    }
}

impl Display for KernelMessageEntry {
    /** Fromat a message into `[ttttt:tttttt] xxxxxx` */
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "[{:5}.{:06}] {}",
            self.time / 1_000_000_000,
            self.time % 1_000_000_000 / 1_000, // keep high 6 digits
            self.message
        )
    }
}

#[derive(Default)]
pub struct KernelMessage {
    pub msgs: Vec<KernelMessageEntry>,
    /** If `output_handler` is set, message will outputs when calling `add_message`. */
    pub output_handler: Option<fn(&str)>,
}

impl KernelMessage {
    pub fn fatal<S>(&mut self, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(KernelMessageLevel::Fatal, msg);
    }
    pub fn error<S>(&mut self, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(KernelMessageLevel::Error, msg);
    }
    pub fn warning<S>(&mut self, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(KernelMessageLevel::Warning, msg);
    }
    pub fn debug<S>(&mut self, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(KernelMessageLevel::Debug, msg);
    }
    pub fn add_message<S>(&mut self, level: KernelMessageLevel, msg: S)
    where
        S: Into<String>,
    {
        let time = crate::time::get_sys_time();
        self.msgs.push(KernelMessageEntry::new(time, level, msg));

        if let Some(output_fn) = self.output_handler {
            output_fn(&self.msgs.last().unwrap().to_string());
        }
    }
}
