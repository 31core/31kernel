use crate::Node;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

pub fn node_name(node: &str) -> &str {
    if let Some(idx) = node.find('@') {
        &node[..idx]
    } else {
        node
    }
}

pub fn unit_address(node: &str) -> Option<&str> {
    if let Some(idx) = node.find('@') {
        Some(&node[idx..])
    } else {
        None
    }
}

/** Get node by alias. */
pub fn node_by_alias<'a>(mut root: &'a Node, alias: &[u8]) -> Option<&'a Node> {
    let alias = String::from_utf8_lossy(&alias[..alias.len() - 1]).to_string();
    let mut path = alias.split('/').skip(1).collect::<Vec<&str>>();
    'main: while !path.is_empty() {
        for node in &root.child_nodes {
            if node.name == *path.first().unwrap() {
                if path.len() == 1 {
                    return Some(node);
                } else {
                    path.remove(0);
                    root = node;
                    continue 'main;
                }
            }
        }
    }
    None
}

/**
 * * `value`: value of the `compatible` key of a node.
 * * `compatible`: compatible to match.
*/
pub fn check_compatible(mut value: &[u8], compatible: &str) -> bool {
    while let Some(null_idx) = value.iter().position(|&x| x == b'\0') {
        if null_idx == compatible.len() && compatible.as_bytes() == &value[..null_idx] {
            return true;
        }
        value = &value[null_idx + 1..];
    }
    false
}

/**
 * Returns [Vec]<(`reg_address`, `reg_size`)>
 */
pub fn parse_reg(mut reg: &[u8], address_cells: usize, size_cells: usize) -> Vec<(u64, u64)> {
    let mut regs = Vec::new();
    for _ in 0..reg.len() / (4 * size_cells + 4 * address_cells) {
        let reg_addr = if address_cells == 2 {
            u64::from_be_bytes(reg[..8].try_into().unwrap())
        } else {
            u32::from_be_bytes(reg[..4].try_into().unwrap()) as u64
        };
        let reg_size = if size_cells == 2 {
            u64::from_be_bytes(reg[8..16].try_into().unwrap())
        } else {
            u32::from_be_bytes(reg[4..8].try_into().unwrap()) as u64
        };
        regs.push((reg_addr, reg_size));
        reg = &reg[4 * size_cells + 4 * address_cells..];
    }
    regs
}
