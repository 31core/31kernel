use core::mem::MaybeUninit;

pub static mut GICD_BASE: MaybeUninit<u32> = MaybeUninit::uninit();
pub const GICD_CTLR: u32 = 0x00;
pub const GICD_ISENABLER: u32 = 0x100;

pub static mut GICC_BASE: MaybeUninit<u32> = MaybeUninit::uninit();
pub const GICC_CTLR: u32 = 0x00;
pub const GICC_PMR: u32 = 0x04;
pub const GICC_IAR: u32 = 0x0c;
pub const GICC_EOIR: u32 = 0x10;

#[inline(always)]
pub unsafe fn gicd_mmio_read(reg: u32) -> u32 {
    unsafe { ((GICD_BASE.assume_init() + reg) as *mut u32).read_volatile() }
}

#[inline(always)]
pub unsafe fn gicd_mmio_write(reg: u32, value: u32) {
    unsafe { ((GICD_BASE.assume_init() + reg) as *mut u32).write_volatile(value) };
}

#[inline(always)]
pub unsafe fn gicc_mmio_read(reg: u32) -> u32 {
    unsafe { ((GICC_BASE.assume_init() + reg) as *mut u32).read_volatile() }
}

#[inline(always)]
pub unsafe fn gicc_mmio_write(reg: u32, value: u32) {
    unsafe { ((GICC_BASE.assume_init() + reg) as *mut u32).write_volatile(value) };
}

pub unsafe fn gic_enable_irq(irq: usize) {
    let reg = irq / 32;
    let bit = irq % 32;

    unsafe { gicd_mmio_write(GICD_ISENABLER + reg as u32 * 4, 1 << bit) };
}

pub const INTID_VTIMER: u32 = 27;

use dtb::{Node, utils::*};

/**
 * Initialize GIC registers from a `interrupt-controller` node.
 */
pub fn init_gic_regs(node: &Node) -> Result<(), &str> {
    if let Some(compatible) = node.get_property("compatible")
        && (check_compatible(compatible, "arm,cortex-a15-gic")
            || check_compatible(compatible, "arm,gic-400"))
        && let Some(reg) = node.get_property("reg")
    {
        use crate::page::{KERNEL_PT, PAGE_SIZE, PageManagement};
        let mut kernel_pt =
            unsafe { super::page::PageManager::from_ttbrx_el1(KERNEL_PT.assume_init() as u64) };

        let regs = parse_reg(reg, node.address_cells, node.size_cells);

        unsafe {
            GICD_BASE = MaybeUninit::new(regs[0].0 as u32);
            GICC_BASE = MaybeUninit::new(regs[1].0 as u32);
        }

        /* map registers */
        for (reg_addr, reg_size) in regs {
            unsafe {
                kernel_pt.map_data(
                    reg_addr as usize / PAGE_SIZE,
                    reg_addr as usize / PAGE_SIZE,
                    (reg_size as usize).div_ceil(PAGE_SIZE),
                );
            }
        }
        Ok(())
    } else {
        Err("No compatible GIC node found")
    }
}
