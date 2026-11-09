//! Bump allocator: a single "next free" pointer that only moves forward.
//!
//! Allocation is O(1) and branch-light, but individual frees cannot be
//! reused: memory is reclaimed only when *every* allocation has been freed.

use core::alloc::Layout;
use core::ptr::null_mut;

use super::{align_up, AllocStats, FreeError, RegionAllocator};

pub struct BumpAllocator {
    start: usize,
    end: usize,
    next: usize,
    live: usize,
    stats: AllocStats,
}

impl BumpAllocator {
    pub const fn empty() -> Self {
        Self { start: 0, end: 0, next: 0, live: 0, stats: AllocStats::new() }
    }

    /// # Safety
    /// `[start, start + size)` must be valid, writable memory that nothing
    /// else uses for as long as this allocator hands out blocks from it.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.next = start;
        self.live = 0;
        self.stats = AllocStats { capacity: size, ..AllocStats::new() };
    }

    fn update_usage(&mut self) {
        self.stats.bytes_in_use = self.next - self.start;
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.bytes_in_use);
    }
}

impl RegionAllocator for BumpAllocator {
    fn name(&self) -> &'static str {
        "bump"
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let start = align_up(self.next, layout.align());
        match start.checked_add(layout.size()) {
            Some(end) if end <= self.end => {
                self.next = end;
                self.live += 1;
                self.stats.allocs += 1;
                self.update_usage();
                start as *mut u8
            }
            _ => {
                self.stats.failed += 1;
                null_mut()
            }
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) -> Result<(), FreeError> {
        let p = ptr as usize;
        let verdict = if p < self.start || p >= self.next {
            Err(FreeError::OutOfRange)
        } else if self.live == 0 {
            Err(FreeError::NotAllocated)
        } else {
            Ok(())
        };
        if verdict.is_err() {
            self.stats.rejected_frees += 1;
            return verdict;
        }
        self.live -= 1;
        self.stats.frees += 1;
        if self.live == 0 {
            self.next = self.start;
        }
        self.update_usage();
        Ok(())
    }

    fn stats(&self) -> AllocStats {
        let free = self.end - self.next;
        AllocStats {
            free_blocks: usize::from(free > 0),
            largest_free: free,
            total_free: free,
            ..self.stats
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::testutil::Arena;

    #[test]
    fn bumps_aligns_and_resets_only_when_empty() {
        let mut arena = Arena::new(1024);
        let mut a = BumpAllocator::empty();
        let base = arena.start();
        unsafe { a.init(base, 1024) };
        let p1 = a.alloc(Layout::from_size_align(10, 1).unwrap());
        let p2 = a.alloc(Layout::from_size_align(8, 64).unwrap());
        assert_eq!(p1 as usize, base);
        assert_eq!(p2 as usize % 64, 0);
        unsafe { a.dealloc(p1, Layout::new::<u8>()).unwrap() };
        assert!(a.stats().bytes_in_use > 0, "space is not reused while p2 is live");
        unsafe { a.dealloc(p2, Layout::new::<u8>()).unwrap() };
        assert_eq!(a.stats().bytes_in_use, 0);
        assert_eq!(a.alloc(Layout::new::<u8>()) as usize, base);
    }

    #[test]
    fn exhaustion_and_rejected_frees() {
        let mut arena = Arena::new(128);
        let mut a = BumpAllocator::empty();
        unsafe { a.init(arena.start(), 128) };
        assert!(a.alloc(Layout::from_size_align(200, 8).unwrap()).is_null());
        assert_eq!(a.stats().failed, 1);
        let mut local = 0u8;
        let err = unsafe { a.dealloc(&mut local, Layout::new::<u8>()) };
        assert_eq!(err, Err(FreeError::OutOfRange));
        assert_eq!(a.stats().rejected_frees, 1);
    }
}
