#![no_std]

use core::{alloc::Layout, ptr::NonNull};

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
#[derive(Debug, Copy, Clone)]
struct Region {
    start: usize,
    end: usize,  
    next: usize,  // 当前分配位置
}
impl Region {
    const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            next: 0,
        }
    }
}

pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    bitmap: u64,  // 标记哪些区域有效
    regions: [Region; 64], 
    current_region: usize,  
    total_size: usize,
    used_size: usize,

}
impl <const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            bitmap: 0,
            regions: [Region::empty(); 64],
            current_region: 0,
            total_size: 0,
            used_size: 0,
        }
    }
}

impl <const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.regions[0] = Region {
            start,
            end: start + size,
            next: start, 
        };
        self.bitmap = 1; 
        self.current_region = 0;
        self.total_size = size;
        self.used_size = 0;
    }
    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        let mut idx = 0;
        while idx < 64 {
            if (self.bitmap & (1 << idx)) == 0 {
                // 找到空闲位了
                self.regions[idx] = Region {
                    start,
                    end: start + size,
                    next: start,
                };
                self.bitmap |= 1 << idx;
                self.total_size += size;
                return Ok(());
            }
            idx += 1;
        }
        Err(AllocError::NoMemory)
    }
}

impl  <const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let size = layout.size();
        let align = layout.align();
        
        let mut region_idx = self.current_region;
        let mut tried_regions = 0u64; 
        while tried_regions != self.bitmap {
            if (self.bitmap & (1 << region_idx)) != 0 {
                let region = &mut self.regions[region_idx];
            
                let aligned_next = (region.next + align - 1) & !(align - 1);
                if aligned_next + size <= region.end {
                    let ptr = aligned_next as *mut u8;
                    region.next = aligned_next + size;
                    self.used_size += size;
                    return Ok(NonNull::new(ptr).unwrap());
                }
            }
            tried_regions |= 1 << region_idx;
            region_idx = (region_idx + 1) % 64;
        }
        
        Err(AllocError::NoMemory)
    }
    
    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: Layout) {
    }
    
    fn total_bytes(&self) -> usize {
        self.total_size
    }

    fn used_bytes(&self) -> usize {
        self.used_size
    }

    fn available_bytes(&self) -> usize {
        self.total_size - self.used_size
    }
}

impl <const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        let layout = Layout::from_size_align(
            num_pages * Self::PAGE_SIZE,
            1 << align_pow2
        ).unwrap();

        self.alloc(layout)
            .map(|ptr| ptr.as_ptr() as usize)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // Bump分配器不支持单独释放
    }

    fn total_pages(&self) -> usize {
        self.total_bytes() / Self::PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        self.used_bytes() / Self::PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        self.available_bytes() / Self::PAGE_SIZE
    }
}
