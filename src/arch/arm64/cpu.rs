use core::arch::asm;

const TCR_T0SZ: u64 = 16;
const TCR_TG0_4KB: u64 = 0 << 6;
const TCR_IRGN0: u64 = 0x01 << 8;
const TCR_ORGN0: u64 = 0x01 << 10;
const TCR_T1SZ: u64 = 16 << 16;
const TCR_TG1_4KB: u64 = 2 << 30;
const TCR_IRGN1: u64 = 0x01 << 24;
const TCR_ORGN1: u64 = 0x01 << 26;

pub unsafe fn cpu_init() {
    unsafe {
        let tcr: u64 = TCR_T0SZ
            | TCR_TG0_4KB
            | TCR_IRGN0
            | TCR_ORGN0
            | TCR_T1SZ
            | TCR_TG1_4KB
            | TCR_IRGN1
            | TCR_ORGN1;
        asm!("msr tcr_el1, {}", in(reg) tcr);
    }
}

#[derive(Default)]
#[repr(C)]
pub struct Context {
    x: [u64; 31],
    sp: u64,
    pc: u64,
    psate: u64,
}
