#![no_std]

extern crate alloc;

use crate::program::Program;
use alloc::vec::Vec;

pub mod program;

pub use program::{Flags as PFlags, Type as PType};

const ELF_HEADER: &[u8] = &[0x7f, 0x45, 0x4c, 0x46];
const ELF_CLASS_32: u8 = 1;
const ELF_CLASS_64: u8 = 2;
const PH_SIZE_32: usize = 32;
const PH_SIZE_64: usize = 56;

#[macro_export]
macro_rules! int {
    ($type:ident, $bytes:ident, $offset:expr, $endian:expr) => {{
        let size = core::mem::size_of::<$type>();
        if $endian == 1 {
            $type::from_le_bytes($bytes[$offset..$offset + size].try_into().unwrap())
        } else {
            $type::from_be_bytes($bytes[$offset..$offset + size].try_into().unwrap())
        }
    }};
}

#[derive(Debug)]
pub enum ElfError {
    InvalidHeader,
    InvalidAbi(u8),
    InvalidType(u16),
    InvalidArchitecture(u16),
}

#[derive(Debug)]
pub enum ElfAbi {
    SystemV,
    NetBSD,
    Linux,
    Solaris,
    FreeBSD,
    OpenBSD,
}

#[derive(Debug)]
pub enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
}

#[derive(Debug)]
pub enum ElfMachine {
    NoSpec,
    X86,
    Arm,
    X86_64,
    Arm64,
    Riscv,
}

#[derive(Debug)]
pub struct Elf {
    pub e_abi: ElfAbi,
    pub e_machine: ElfMachine,
    pub e_type: ElfType,
    pub e_entry: usize,
    pub p_headers: Vec<Program>,
}

impl Elf {
    pub fn parse(bytes: &[u8]) -> Result<Self, ElfError> {
        if !bytes.starts_with(ELF_HEADER) {
            return Err(ElfError::InvalidHeader);
        }

        let class = bytes[4];
        let endian = bytes[5];

        let e_abi = match bytes[7] {
            0x00 => ElfAbi::SystemV,
            0x02 => ElfAbi::NetBSD,
            0x03 => ElfAbi::Linux,
            0x06 => ElfAbi::Solaris,
            0x09 => ElfAbi::FreeBSD,
            0x0c => ElfAbi::OpenBSD,
            abi => return Err(ElfError::InvalidAbi(abi)),
        };

        let e_type = match int!(u16, bytes, 16, endian) {
            0x00 => ElfType::None,
            0x01 => ElfType::Rel,
            0x02 => ElfType::Exec,
            0x03 => ElfType::Dyn,
            etype => return Err(ElfError::InvalidType(etype)),
        };

        let e_machine = match int!(u16, bytes, 18, endian) {
            0x00 => ElfMachine::NoSpec,
            0x03 => ElfMachine::X86,
            0x28 => ElfMachine::Arm,
            0x3e => ElfMachine::X86_64,
            0xb7 => ElfMachine::Arm64,
            0xf3 => ElfMachine::Riscv,
            arch => return Err(ElfError::InvalidArchitecture(arch)),
        };

        let e_entry = if class == ELF_CLASS_32 {
            int!(u32, bytes, 24, endian) as usize
        } else {
            int!(u64, bytes, 24, endian) as usize
        };

        let e_phoff = if class == ELF_CLASS_32 {
            int!(u32, bytes, 28, endian) as usize
        } else {
            int!(u64, bytes, 32, endian) as usize
        };

        let e_phnum = if class == ELF_CLASS_32 {
            int!(u16, bytes, 44, endian) as usize
        } else {
            int!(u16, bytes, 56, endian) as usize
        };

        /* read section headers */
        let mut sections = Vec::new();
        for sec in 0..e_phnum {
            let (sh_start, sh_end) = if class == ELF_CLASS_32 {
                (sec * PH_SIZE_32, (sec + 1) * PH_SIZE_32)
            } else {
                (sec * PH_SIZE_64, (sec + 1) * PH_SIZE_64)
            };
            let section =
                Program::parse(&bytes[e_phoff + sh_start..e_phoff + sh_end], class, endian);
            sections.push(section);
        }

        Ok(Elf {
            e_abi,
            e_type,
            e_machine,
            e_entry,
            p_headers: sections,
        })
    }
}
