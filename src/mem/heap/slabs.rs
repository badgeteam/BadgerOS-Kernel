use core::{alloc::Layout, ptr::NonNull};

use alloc::{
    alloc::{AllocError, Allocator},
    boxed::Box,
    vec::Vec,
};

use crate::{
    config::PAGE_SIZE,
    impl_has_list_node,
    kcore::sync::mutex::Mutex,
    util::list::{BoxInvasiveList, InvasiveListNode},
};

use super::hhdm::HhdmAlloc;

pub const SMALLEST: usize = 8;
pub const SIZE_MUL: usize = 2;
pub const SIZES: usize = 9;
pub const LARGEST: usize = SMALLEST * SIZE_MUL.pow(SIZES as u32 - 1);
pub const BUDDY_ORDER: u8 = 2;
pub const BLOCK_SIZE: usize = (PAGE_SIZE as usize) << BUDDY_ORDER;

#[repr(transparent)]
struct SlabLink {
    next: *mut SlabLink,
}

#[repr(C)]
struct SlabBlockHeader {
    node: InvasiveListNode,
    occupancy: usize,
    capacity: usize,
    list_head: *mut SlabLink,
}

#[repr(C)]
pub struct SlabBlock {
    header: SlabBlockHeader,
    data: [u8; BLOCK_SIZE - size_of::<SlabBlockHeader>()],
}
impl_has_list_node!(SlabBlock, header.node);

impl SlabBlock {
    fn new(slab_size: usize) -> Result<Box<Self, HhdmAlloc>, AllocError> {
        unsafe {
            // Must be done this way or Rust will allocate it on the stack!
            let mut mem = Box::<SlabBlock, _>::try_new_uninit_in(HhdmAlloc)?.assume_init();
            mem.init(slab_size);
            Ok(mem)
        }
    }

    fn init(&mut self, slab_size: usize) {
        self.header.node = InvasiveListNode::new();

        assert!(slab_size >= size_of::<*mut u8>());
        assert!(slab_size.is_power_of_two());
        let overhead = size_of::<SlabBlockHeader>().div_ceil(slab_size);
        let first = overhead * slab_size - size_of::<SlabBlockHeader>();

        let mut ptr = &raw mut self.data[first] as *mut SlabLink;
        self.header.capacity = BLOCK_SIZE / slab_size - overhead;
        self.header.occupancy = 0;
        self.header.list_head = ptr;

        for _ in 0..self.header.capacity {
            let next = ptr.wrapping_byte_add(slab_size);
            unsafe { (*ptr).next = next };
            ptr = next;
        }
    }

    fn alloc(&mut self, slab_size: usize) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = NonNull::new(self.header.list_head).ok_or(AllocError)?;
        self.header.list_head = unsafe { ptr.read().next };
        self.header.occupancy += 1;
        Ok(NonNull::slice_from_raw_parts(ptr.cast(), slab_size))
    }

    unsafe fn free(&mut self, ptr: NonNull<u8>) {
        unsafe {
            let ptr = ptr.cast::<SlabLink>();
            ptr.write(SlabLink {
                next: self.header.list_head,
            });
            self.header.occupancy -= 1;
            self.header.list_head = ptr.as_ptr();
        }
    }
}

pub struct SlabPool {
    empty: Option<Box<SlabBlock, HhdmAlloc>>,
    partial: BoxInvasiveList<SlabBlock, HhdmAlloc>,
    full: BoxInvasiveList<SlabBlock, HhdmAlloc>,
}

impl SlabPool {
    const fn new() -> Self {
        Self {
            empty: None,
            partial: BoxInvasiveList::new_in(HhdmAlloc),
            full: BoxInvasiveList::new_in(HhdmAlloc),
        }
    }

    fn alloc(&mut self, slab_size: usize) -> Result<NonNull<[u8]>, AllocError> {
        assert!(slab_size.is_power_of_two());
        if let Some(block) = self.partial.front_mut() {
            // Check partial blocks first.
            let res = block.alloc(slab_size).expect("Full block in partial list");

            if block.header.occupancy == block.header.capacity {
                let block = self.partial.pop_front().unwrap();
                self.full.push_back(block);
            }

            Ok(res)
        } else if let Some(mut block) = self.empty.take() {
            let res = block.alloc(slab_size).expect("Full block in empty cache");
            let _ = self.partial.push_back(block);
            Ok(res)
        } else {
            // If the partial list is empty, create a new block.
            let mut block = SlabBlock::new(slab_size)?;
            let res = block.alloc(slab_size).expect("New block is already full");
            self.partial.push_back(block);
            Ok(res)
        }
    }

    unsafe fn free(&mut self, ptr: NonNull<u8>) {
        unsafe {
            // The blocks are naturally aligned; we can find the address by simply masking the bottom bits off.
            let block = (ptr.as_ptr() as usize & !(BLOCK_SIZE - 1)) as *mut SlabBlock;
            let was_full = (*block).header.occupancy == (*block).header.capacity;
            (*block).free(ptr);

            if was_full {
                self.full.inner.remove(block);
                let _ = self.partial.push_back(Box::from_raw_in(block, HhdmAlloc));
            } else if (*block).header.occupancy == 0 {
                self.partial.inner.remove(block);
                // Retain up to one empty block.
                let block = Box::from_raw_in(block, HhdmAlloc);
                if self.empty.is_none() {
                    self.empty = Some(block);
                }
            }
        }
    }
}

pub struct SlabsAlloc {
    blocks: [Mutex<SlabPool>; SIZES],
}

impl SlabsAlloc {
    pub const fn new() -> Self {
        Self {
            blocks: [const { Mutex::new(SlabPool::new()) }; _],
        }
    }

    pub const fn bucket_for(layout: Layout) -> Option<(usize, usize)> {
        assert!(layout.size() != 0);
        let size_pow2 = layout
            .pad_to_align()
            .size()
            .div_ceil(SMALLEST)
            .next_power_of_two();
        let log_size_mul = size_pow2.trailing_zeros() / SIZE_MUL.trailing_zeros();
        if log_size_mul as usize >= SIZES {
            return None;
        }
        let slab_size = SMALLEST * SIZE_MUL.pow(log_size_mul);
        debug_assert!(slab_size >= layout.size());
        Some((log_size_mul as usize, slab_size))
    }
}

unsafe impl Allocator for SlabsAlloc {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let (bucket, slab_size) = Self::bucket_for(layout).ok_or(AllocError)?;
        self.blocks[bucket].unintr_lock().alloc(slab_size)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let (bucket, _) =
            Self::bucket_for(layout).expect("Invalid layout given to SlabsAlloc::deallocate");
        unsafe { self.blocks[bucket].unintr_lock().free(ptr) };
    }
}

pub static GLOBAL_SLABS: SlabsAlloc = SlabsAlloc::new();

pmm_ktest! { SLABS_BASIC,
    let a0 = Box::try_new_in([0u8; 1], &GLOBAL_SLABS)?;
    let a1 = Box::try_new_in([0u8; 8], &GLOBAL_SLABS)?;
    let a2 = Box::try_new_in([0u8; 16], &GLOBAL_SLABS)?;
    let a3 = Box::try_new_in([0u8; 32], &GLOBAL_SLABS)?;
    let a4 = Box::try_new_in([0u8; 64], &GLOBAL_SLABS)?;
    let a5 = Box::try_new_in([0u8; LARGEST], &GLOBAL_SLABS)?;
    drop(a0);
    drop(a1);
    drop(a2);
    drop(a3);
    drop(a4);
    drop(a5);
}

pmm_ktest! { SLABS_EXHAUST,
    let mut block = SlabBlock::new(SMALLEST)?;
    let mut resv = Vec::<NonNull<u8>, _>::new_in(super::hhdm::HhdmAlloc);

    let cap = (*block).header.capacity;
    for _ in 0..cap {
        resv.push(block.alloc(SMALLEST)?.cast());
    }
    ktest_expect!((*block).header.occupancy, cap);

    for alloc in resv {
        unsafe { block.free(alloc) };
    }
    ktest_expect!((*block).header.occupancy, 0);
}
