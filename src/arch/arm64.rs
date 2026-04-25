pub mod cpu;
mod gic;
pub mod page;
mod syscall;
mod trap;

use crate::page::PageManagement;
use core::arch::asm;
use dtb::{DeviceTree, Node, utils::*};

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

fn map_gic_regs(node: &Node) {
    if let Some(compatible) = node.get_property("compatible")
        && check_compatible(compatible, "arm,cortex-a15-gic")
        && let Some(reg) = node.get_property("reg")
    {
        use crate::page::{KERNEL_PT, PAGE_SIZE};
        let mut kernel_pt =
            unsafe { page::PageManager::from_ttbrx_el1(KERNEL_PT.assume_init() as u64) };

        for (reg_addr, reg_size) in parse_reg(reg, node.address_cells, node.size_cells) {
            unsafe {
                kernel_pt.map_data(
                    reg_addr as usize / PAGE_SIZE,
                    reg_addr as usize / PAGE_SIZE,
                    (reg_size as usize).div_ceil(PAGE_SIZE),
                );
            }
        }
    }
}

pub fn soc_init(dtb: &DeviceTree) {
    /* map gic registers */
    'root: for node in &dtb.root.child_nodes {
        if node_name(&node.name) == "intc" || node_name(&node.name) == "interrupt-controller" {
            map_gic_regs(node);
            break;
        } else if node_name(&node.name) == "soc" {
            for soc_node in &node.child_nodes {
                if node_name(&soc_node.name) == "intc"
                    || node_name(&soc_node.name) == "interrupt-controller"
                {
                    map_gic_regs(soc_node);
                    break 'root;
                }
            }
        }
    }
}
