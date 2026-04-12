use core::{arch::asm, ptr::addr_of};

const TCR_T0SZ: u64 = 16;
const TCR_TG0_4KB: u64 = 0 << 6;
const TCR_IRGN0: u64 = 0x01 << 8;
const TCR_ORGN0: u64 = 0x01 << 10;
const TCR_T1SZ: u64 = 16 << 16;
const TCR_TG1_4KB: u64 = 2 << 30;
const TCR_IRGN1: u64 = 0x01 << 24;
const TCR_ORGN1: u64 = 0x01 << 26;

const GICD_BASE: u32 = 0x08000000;
const GICC_BASE: u32 = 0x08010000;
const GICC_CTLR: u32 = 0x00;
const GICC_PMR: u32 = 0x04;

fn gic_enable_irq(irq: usize) {
    let reg = irq / 32;
    let bit = irq % 32;

    let isenabler = GICD_BASE + 0x100 + reg as u32 * 4;
    unsafe { (isenabler as *mut u32).write_volatile(1 << bit) };
}

unsafe extern "C" {
    #[link_name = "vector_table"]
    static VECTOR_TABLE: u8;
}

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
        asm!("msr TCR_EL1, {}", in(reg) tcr);

        asm!("msr VBAR_EL1, {}", "isb", in(reg) addr_of!(VECTOR_TABLE));

        /* enable timer */
        let freq: u64;
        asm!("mrs {}, CNTFRQ_EL0", out(reg) freq);
        asm!("msr CNTV_TVAL_EL0, {}", in(reg) freq / 1000); // 1ms clock
        asm!("msr CNTV_CTL_EL0, {}", in(reg) 1_u64);

        gic_enable_irq(27);
        (GICD_BASE as *mut u32).write_volatile(1);
        ((GICC_BASE + GICC_CTLR) as *mut u32).write_volatile(1);
        ((GICC_BASE + GICC_PMR) as *mut u32).write_volatile(0xff);
    }
}

#[derive(Default)]
#[repr(C)]
pub struct Context {
    pub x: [u64; 31],
    pub sp: u64,
    pub elr_el1: u64,
    pub psate: u64,
}
