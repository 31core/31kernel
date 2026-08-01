/*!
 * Kernel debug message.
 */

use crate::{device::CharDev, global::Global, mutex::Mutex};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    fmt::Result as FmtResult,
    fmt::{Display, Formatter},
};

pub static KMSG: Global<KernelMessage> = Mutex::new(KernelMessage::default());
const KMSG_MAX: usize = 1024;

#[macro_export]
macro_rules! printk_error {
    ($($arg:tt)*) => {
        $crate::kmsg::KMSG.lock().error(None, &alloc::format!($($arg)*));
    };
}

#[macro_export]
macro_rules! printk_warning {
    ($($arg:tt)*) => {
        $crate::kmsg::KMSG.lock().warning(None, &alloc::format!($($arg)*));
    };
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::kmsg::KMSG.lock().debug(None, &alloc::format!($($arg)*));
    };
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
    pub module: Option<&'static str>,
}

impl KernelMessageEntry {
    pub fn new<S>(
        module: Option<&'static str>,
        time: u64,
        level: KernelMessageLevel,
        msg: S,
    ) -> Self
    where
        S: Into<String>,
    {
        Self {
            level,
            time,
            message: msg.into(),
            module,
        }
    }
}

impl Display for KernelMessageEntry {
    /** Fromat a message into `[ttttt:tttttt] xxxxxx` */
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "[{:5}.{:06}] ",
            self.time / 1_000_000_000,
            self.time % 1_000_000_000 / 1_000, // keep high 6 digits
        )?;
        if let Some(module) = self.module {
            write!(f, "{}: ", module)?;
        }
        write!(f, "{}", self.message)
    }
}

#[derive(Default)]
pub struct KernelMessage {
    msgs: Vec<KernelMessageEntry>,
    /** Number of maximum log messages to keep. */
    max_log: usize,
    /** If `output_handler` is set, message will outputs when calling `add_message`. */
    pub output_handler: Option<Box<dyn CharDev>>,
}

unsafe impl Send for KernelMessage {}

impl KernelMessage {
    pub const fn default() -> Self {
        Self {
            msgs: Vec::new(),
            output_handler: None,
            max_log: KMSG_MAX,
        }
    }
    pub fn fatal<S>(&mut self, module: Option<&'static str>, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(module, KernelMessageLevel::Fatal, msg);
    }
    pub fn error<S>(&mut self, module: Option<&'static str>, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(module, KernelMessageLevel::Error, msg);
    }
    pub fn warning<S>(&mut self, module: Option<&'static str>, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(module, KernelMessageLevel::Warning, msg);
    }
    pub fn debug<S>(&mut self, module: Option<&'static str>, msg: S)
    where
        S: Into<String>,
    {
        self.add_message(module, KernelMessageLevel::Debug, msg);
    }
    pub fn add_message<S>(
        &mut self,
        module: Option<&'static str>,
        level: KernelMessageLevel,
        msg: S,
    ) where
        S: Into<String>,
    {
        let time = crate::time::get_sys_time();
        self.msgs
            .push(KernelMessageEntry::new(module, time, level, msg));
        if self.msgs.len() > self.max_log {
            self.msgs.remove(0);
        }

        if let Some(output_fn) = &self.output_handler {
            output_fn.print_str(&self.msgs.last().unwrap().to_string());
        }
    }
    pub fn get_messages(&self) -> &[KernelMessageEntry] {
        &self.msgs
    }
    pub fn set_max_log(&mut self, max: usize) {
        self.max_log = max;
        while self.msgs.len() > self.max_log {
            self.msgs.remove(0);
        }
    }
    pub fn get_max_log(&self) -> usize {
        self.max_log
    }
}
