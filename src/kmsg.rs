use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

pub static mut KMSG: Option<KernelMessage> = None;

#[macro_export]
macro_rules! printk {
    ($($arg: tt)*) => {
        {
            use alloc::format;
            let kmsg = unsafe { crate::kmsg::KMSG.as_mut().unwrap() };
            kmsg.add_message(&format!($($arg)*));
        }
    };
}

pub fn kmsg_init() {
    unsafe {
        KMSG = Some(KernelMessage::default());
    }
}

#[derive(Default)]
pub struct KernelMessageEntry {
    pub time: u64,
    pub message: String,
}

impl KernelMessageEntry {
    pub fn new(time: u64, msg: &str) -> Self {
        Self {
            time,
            message: msg.to_owned(),
        }
    }
    /** Fromat a message into `[ttttt:tttttt] xxxxxx` */
    pub fn to_string(&self) -> String {
        format!(
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
    pub fn add_message(&mut self, msg: &str) {
        #[cfg(target_arch = "riscv64")]
        let time = crate::arch::riscv64::get_sys_time();
        #[cfg(target_arch = "aarch64")]
        let time = 0;
        self.msgs.push(KernelMessageEntry::new(time, msg));

        if let Some(output_fn) = self.output_handler {
            output_fn(&self.msgs.last().unwrap().to_string());
        }
    }
}
