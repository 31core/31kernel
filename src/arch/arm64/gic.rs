pub const GICD_BASE: u32 = 0x08000000;
pub const GICD_CTLR: u32 = 0x00;
pub const GICD_ISENABLER: u32 = 0x100;

pub const GICC_BASE: u32 = 0x08010000;
pub const GICC_CTLR: u32 = 0x00;
pub const GICC_PMR: u32 = 0x04;
pub const GICC_IAR: u32 = 0x0c;
pub const GICC_EOIR: u32 = 0x10;

#[inline(always)]
pub unsafe fn gicd_mmio_read(reg: u32) -> u32 {
    unsafe { ((GICD_BASE + reg) as *mut u32).read_volatile() }
}

#[inline(always)]
pub unsafe fn gicd_mmio_write(reg: u32, value: u32) {
    unsafe { ((GICD_BASE + reg) as *mut u32).write_volatile(value) };
}

#[inline(always)]
pub unsafe fn gicc_mmio_read(reg: u32) -> u32 {
    unsafe { ((GICC_BASE + reg) as *mut u32).read_volatile() }
}

#[inline(always)]
pub unsafe fn gicc_mmio_write(reg: u32, value: u32) {
    unsafe { ((GICC_BASE + reg) as *mut u32).write_volatile(value) };
}

pub unsafe fn gic_enable_irq(irq: usize) {
    let reg = irq / 32;
    let bit = irq % 32;

    unsafe { gicd_mmio_write(GICD_ISENABLER + reg as u32 * 4, 1 << bit) };
}

pub const INTID_VTIMER: u32 = 27;
