use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::KMSG;

pub fn kmsg_init() {
    unsafe {
        KMSG = Some(KernelMessage::default());
    }
}

#[derive(Default)]
pub struct KernelMessage {
    pub msgs: Vec<String>,
    pub output_handler: Option<fn(&str)>,
}

impl KernelMessage {
    pub fn add_message(&mut self, msg: &str) {
        self.msgs.push(msg.to_owned());

        if let Some(output_fn) = self.output_handler {
            output_fn(msg);
        }
    }
}
