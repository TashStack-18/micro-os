//! Memory-allocation mechanisms that manage a raw address range.
//!
//! The kernel heap (`#[global_allocator]`) is a [`FreeListAllocator`]; the
//! [`BumpAllocator`] exists as the simplest baseline for comparison.

pub mod bump;
pub mod freelist;

use core::alloc::Layout;

pub use bump::BumpAllocator;
pub use freelist::{Fit, FreeListAllocator};

/// Counters every allocator maintains, plus a snapshot of its free space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocStats {
    pub capacity: usize,
    pub allocs: u64,
    pub frees: u64,
    pub failed: u64,
    pub rejected_frees: u64,
    pub bytes_in_use: usize,
    pub peak_bytes: usize,
    pub free_blocks: usize,
    pub largest_free: usize,
    pub total_free: usize,
}

impl AllocStats {
    pub const fn new() -> Self {
        Self {
            capacity: 0,
            allocs: 0,
            frees: 0,
            failed: 0,
            rejected_frees: 0,
            bytes_in_use: 0,
            peak_bytes: 0,
            free_blocks: 0,
            largest_free: 0,
            total_free: 0,
        }
    }

    /// External fragmentation in basis points (1/100 %):
    /// `1 - largest_free / total_free`. 0 means all free space is one block.
    pub fn fragmentation_bp(&self) -> u64 {
        if self.total_free == 0 {
            return 0;
        }
        10_000 - (self.largest_free as u128 * 10_000 / self.total_free as u128) as u64
    }

    pub(crate) fn record_alloc(&mut self, bytes: usize) {
        self.allocs += 1;
        self.bytes_in_use += bytes;
        self.peak_bytes = self.peak_bytes.max(self.bytes_in_use);
    }
}

/// Why a free request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeError {
    /// Pointer is not inside the allocator's region.
    OutOfRange,
    /// Pointer is inside the region but not on a block boundary.
    Misaligned,
    /// No live allocation starts here (double free or foreign pointer).
    NotAllocated,
    /// Metadata is inconsistent (overlapping blocks, bad size).
    Corrupt,
}

/// Common interface so allocation strategies can be benchmarked side by side.
pub trait RegionAllocator {
    fn name(&self) -> &'static str;

    /// Allocate a block for `layout`; returns null when no block fits.
    fn alloc(&mut self, layout: Layout) -> *mut u8;

    /// Return a block. Invalid requests are detected and refused, never applied.
    ///
    /// # Safety
    /// If `ptr` is a live allocation from this allocator, the caller must not
    /// use it afterwards.
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) -> Result<(), FreeError>;

    fn stats(&self) -> AllocStats;
}

pub const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[cfg(test)]
pub(crate) mod testutil {
    /// A heap-allocated, 4 KiB-aligned arena for exercising allocators on the host.
    pub struct Arena {
        buf: Vec<u8>,
    }

    impl Arena {
        pub fn new(size: usize) -> Self {
            Self { buf: vec![0u8; size + 4096] }
        }
        pub fn start(&mut self) -> usize {
            super::align_up(self.buf.as_mut_ptr() as usize, 4096)
        }
    }
}
