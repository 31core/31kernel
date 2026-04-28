pub mod cpu;
mod gic;
pub mod page;
mod syscall;
mod trap;

use core::arch::asm;
use dtb::{DeviceTree, utils::*};
use gic::*;

pub fn get_sys_time() -> u64 {
    let ticks: u64;
    unsafe { asm!("mrs {}, CNTVCT_EL0" , out(reg) ticks) };

    let freq: u64;
    unsafe { asm!("mrs {}, CNTFRQ_EL0" , out(reg) freq) };

    ticks * 1_000_000_000 / freq
}

#[inline(always)]
pub unsafe fn enable_interrupts() {
    unsafe { asm!("msr DAIFClr, #2") };
}

#[inline(always)]
pub unsafe fn disable_interrupts() {
    unsafe { asm!("msr DAIFSet, #2") };
}

pub fn soc_init(dtb: &DeviceTree) {
    /* map gic registers */
    'root: for node in &dtb.root.child_nodes {
        if node_name(&node.name) == "intc" || node_name(&node.name) == "interrupt-controller" {
            init_gic_regs(node);
            break;
        } else if node_name(&node.name) == "soc" {
            for soc_node in &node.child_nodes {
                if node_name(&soc_node.name) == "intc"
                    || node_name(&soc_node.name) == "interrupt-controller"
                {
                    init_gic_regs(soc_node);
                    break 'root;
                }
            }
        }
    }

    /* initalize gic */
    unsafe {
        gic_enable_irq(27);
        gicd_mmio_write(GICD_CTLR, 1);
        gicc_mmio_write(GICC_CTLR, 1);
        gicc_mmio_write(GICC_PMR, 0xff);
    }
}
