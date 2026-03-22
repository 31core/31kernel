use crate::{ELF_CLASS_32, ELF_CLASS_64, int};
use alloc::vec::Vec;

const FLAGS_X: usize = 0x01;
const FLAGS_W: usize = 0x02;
const FLAGS_R: usize = 0x04;

#[derive(Debug)]
pub enum Type {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
}

#[derive(Debug)]
pub enum Flags {
    Exec,
    Write,
    Read,
}

#[derive(Debug)]
pub struct Program {
    pub p_type: Type,
    pub p_flags: Vec<Flags>,
    pub p_offset: usize,
    pub v_addr: usize,
    pub p_addr: usize,
    pub p_filesz: usize,
    pub p_memsz: usize,
}

impl Program {
    pub fn parse(bytes: &[u8], class: u8, endian: u8) -> Result<Self, ()> {
        let p_type = match int!(u32, bytes, 0, endian) {
            0x00 => Type::Null,
            0x01 => Type::Load,
            0x02 => Type::Dynamic,
            0x03 => Type::Interp,
            0x04 => Type::Note,
            _ => Type::Null,
        };

        /* parse flags */
        let mut p_flags = Vec::new();
        let flags_bits = if class == ELF_CLASS_64 {
            int!(u64, bytes, 4, endian) as usize
        } else {
            int!(u64, bytes, 24, endian) as usize
        };
        if flags_bits & FLAGS_X > 0 {
            p_flags.push(Flags::Exec);
        }
        if flags_bits & FLAGS_W > 0 {
            p_flags.push(Flags::Write);
        }
        if flags_bits & FLAGS_R > 0 {
            p_flags.push(Flags::Read);
        }

        let p_offset = if class == ELF_CLASS_32 {
            int!(u32, bytes, 4, endian) as usize
        } else {
            int!(u64, bytes, 8, endian) as usize
        };

        let v_addr = if class == ELF_CLASS_32 {
            int!(u32, bytes, 8, endian) as usize
        } else {
            int!(u64, bytes, 16, endian) as usize
        };

        let p_addr = if class == ELF_CLASS_32 {
            int!(u32, bytes, 12, endian) as usize
        } else {
            int!(u64, bytes, 24, endian) as usize
        };

        let p_filesz = if class == ELF_CLASS_32 {
            int!(u32, bytes, 16, endian) as usize
        } else {
            int!(u64, bytes, 32, endian) as usize
        };

        let p_memsz = if class == ELF_CLASS_32 {
            int!(u32, bytes, 20, endian) as usize
        } else {
            int!(u64, bytes, 40, endian) as usize
        };

        Ok(Self {
            p_type,
            p_flags,
            p_offset,
            v_addr,
            p_addr,
            p_filesz,
            p_memsz,
        })
    }
}
