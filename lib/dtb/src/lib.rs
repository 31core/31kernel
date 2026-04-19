#![no_std]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::result::Result;

const MAGIC: [u8; 4] = [0xd0, 0x0d, 0xfe, 0xed];
const FDT_BEGIN_NODE: u32 = 0x01;
const FDT_END_NODE: u32 = 0x02;
const FDT_PROP: u32 = 0x03;
const FDT_NOP: u32 = 0x04;
const FDT_END: u32 = 0x09;

macro_rules! u32 {
    ($bytes:ident, $offset:tt) => {
        u32::from_be_bytes($bytes[$offset..$offset + 4].try_into().unwrap())
    };
}

fn parse_null_string(mut bytes: &[u8]) -> (&[u8], String) {
    let mut string = String::new();
    while bytes[0] != b'\0' {
        string.push(bytes[0] as char);
        bytes = &bytes[1..];
    }
    (bytes, string)
}

#[derive(Debug)]
pub enum ParseError {
    InvalidHeader([u8; 4]),
}

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub progs: Vec<Property>,
    pub child_nodes: Vec<Box<Node>>,
}

impl Node {
    pub fn parse<'a>(mut bytes: &'a [u8], strings_buf: &[u8]) -> (&'a [u8], Self) {
        bytes = &bytes[4..]; // skip FDT_BEGIN_NODE
        let (mut bytes, name) = parse_null_string(bytes);
        let padding = 4 - name.len() % 4;
        bytes = &bytes[padding..];

        let mut progs = Vec::new();
        let mut child_nodes = Vec::new();
        while u32!(bytes, 0) != FDT_END_NODE {
            if u32!(bytes, 0) == FDT_BEGIN_NODE {
                let child_node;
                (bytes, child_node) = Self::parse(bytes, strings_buf);
                debug_assert_eq!(u32!(bytes, 0), FDT_END_NODE);
                bytes = &bytes[4..]; // skip FDT_END_NODE
                child_nodes.push(Box::new(child_node));
                continue;
            } else if u32!(bytes, 0) == FDT_NOP {
                bytes = &bytes[4..]; // skip FDT_NOP
                continue;
            }

            debug_assert_eq!(u32!(bytes, 0), FDT_PROP);
            let len = u32!(bytes, 4) as usize;
            let nameoff = u32!(bytes, 8) as usize;
            bytes = &bytes[12..];

            let name;
            if nameoff > 0 {
                (_, name) = parse_null_string(&strings_buf[nameoff..]);
            } else {
                name = String::default();
            }
            let value = bytes[..len].to_vec();
            let padding = (4 - len % 4) % 4;
            progs.push(Property { name, value });
            bytes = &bytes[len + padding..];
        }

        (
            bytes,
            Self {
                name,
                progs,
                child_nodes,
            },
        )
    }
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub value: Vec<u8>,
}

#[derive(Debug)]
pub struct DeviceTree {
    pub version: u32,
    pub root: Node,
}

impl DeviceTree {
    /** Get total size of an in-memory dtb. */
    pub unsafe fn detect_totalsize(ptr: *const u8) -> usize {
        unsafe { u32::from_be((ptr as *const u32).add(1).read()) as usize }
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseError> {
        if !bytes.starts_with(&MAGIC) {
            return Err(ParseError::InvalidHeader(bytes[..4].try_into().unwrap()));
        }
        let struct_off = u32!(bytes, 8) as usize;
        let strings_off = u32!(bytes, 12) as usize;
        let version = u32!(bytes, 20);

        let strings_buf = &bytes[strings_off..];
        let struct_buf = &bytes[struct_off..];
        let (struct_buf, root) = Node::parse(struct_buf, strings_buf);
        debug_assert_eq!(u32!(struct_buf, 4), FDT_END);

        Ok(Self { version, root })
    }
}
