use super::{AllocError, PAGE_BITS, PAGE_SIZE, PageAllocator};
use spinlock::Spinlock;

pub static STATIC_ALLOCATOR: Spinlock<StaticPageAllocator> =
    Spinlock::new(StaticPageAllocator::new());

const STATIC_PAGE_CAP: usize = 256;
#[repr(C, align(4096))]
pub struct StaticPageAllocator {
    pages: [[u8; PAGE_SIZE]; STATIC_PAGE_CAP],
    bitmap: [u64; STATIC_PAGE_CAP / u64::BITS as usize],
}

impl StaticPageAllocator {
    pub const fn new() -> Self {
        Self {
            pages: [[0; PAGE_SIZE]; STATIC_PAGE_CAP],
            bitmap: [0; STATIC_PAGE_CAP / u64::BITS as usize],
        }
    }
}

impl PageAllocator for StaticPageAllocator {
    fn alloc_pages(&mut self, pages_count: usize) -> Result<usize, AllocError> {
        assert_eq!(pages_count, 1);
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            let bit = byte.leading_ones();
            if bit < u64::BITS {
                *byte |= 1 << (63 - bit);
                return Ok(self.pages[64 * byte_idx + bit as usize].as_ptr() as usize >> PAGE_BITS);
            }
        }
        Err(AllocError::OutOfMemory)
    }
    fn free_pages(&mut self, page_start: usize, pages_count: usize) {
        assert_eq!(pages_count, 1);
        let byte = page_start / 64;
        let bit = page_start % 64;
        self.bitmap[byte] &= !(1 << (63 - bit));
    }
}
