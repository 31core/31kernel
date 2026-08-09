use crate::page::{PAGE_BITS, PHY_ADDR, VIRT_ADDR};

#[repr(transparent)]
#[derive(Default, Clone, Copy)]
pub struct VirtualAddress<const OFFSET: usize>(pub usize);

impl<const OFFSET: usize> From<PhysicalAddress<OFFSET>> for VirtualAddress<OFFSET> {
    fn from(pa: PhysicalAddress<OFFSET>) -> Self {
        VirtualAddress(pa.0 + OFFSET)
    }
}

#[repr(transparent)]
#[derive(Default, Clone, Copy)]
pub struct PhysicalAddress<const OFFSET: usize>(pub usize);

impl<const OFFSET: usize> From<VirtualAddress<OFFSET>> for PhysicalAddress<OFFSET> {
    fn from(va: VirtualAddress<OFFSET>) -> Self {
        PhysicalAddress(va.0 - OFFSET)
    }
}

pub type VirtAddr = VirtualAddress<{ VIRT_ADDR - PHY_ADDR }>;
pub type PhysAddr = PhysicalAddress<{ VIRT_ADDR - PHY_ADDR }>;

#[repr(transparent)]
#[derive(Default, Clone, Copy)]
pub struct VirtualPage<const OFFSET: usize>(pub usize);

impl<const OFFSET: usize> From<PhysicalPage<OFFSET>> for VirtualPage<OFFSET> {
    fn from(pa: PhysicalPage<OFFSET>) -> Self {
        VirtualPage(pa.0 + OFFSET)
    }
}

#[repr(transparent)]
#[derive(Default, Clone, Copy)]
pub struct PhysicalPage<const OFFSET: usize>(pub usize);

impl<const OFFSET: usize> From<VirtualPage<OFFSET>> for PhysicalPage<OFFSET> {
    fn from(va: VirtualPage<OFFSET>) -> Self {
        PhysicalPage(va.0 - OFFSET)
    }
}

pub type VirtPage = VirtualPage<{ (VIRT_ADDR - PHY_ADDR) >> PAGE_BITS }>;
pub type PhysPage = PhysicalPage<{ (VIRT_ADDR - PHY_ADDR) >> PAGE_BITS }>;
