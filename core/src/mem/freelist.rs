//! Free-list allocator with block headers, splitting and coalescing.
//!
//! Memory layout (all addresses and sizes are multiples of 16):
//!
//! ```text
//!  allocated block:  [ Header{size, LIVE_MAGIC} ][ payload ........ ]
//!  free block:       [ FreeNode{size, next}     ][ unused ......... ]
//! ```
//!
//! Free blocks form a singly linked list sorted by address, which makes
//! coalescing with both neighbours a local operation on insert. The header
//! records the real block size, so `dealloc` does not trust the caller's
//! `Layout`, and the magic word lets us refuse double and foreign frees.

use core::alloc::Layout;
use core::ptr::null_mut;

use super::{align_up, AllocStats, FreeError, RegionAllocator};

/// Size of the per-block header and the granularity of every block.
pub const HEADER: usize = 16;
/// Smallest block worth tracking: header plus 16 bytes of payload.
pub const MIN_BLOCK: usize = 32;
/// Marks a live allocation. It is odd, so it can never equal the 16-aligned
/// (or null) `next` pointer that overwrites it once the block is freed.
const LIVE_MAGIC: usize = 0xA110_CA7E_D0C5_B10B;

#[repr(C)]
struct FreeNode {
    size: usize,
    next: *mut FreeNode,
}

#[repr(C)]
struct Header {
    size: usize,
    magic: usize,
}

/// Placement strategy when several free blocks could satisfy a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Lowest-addressed block that fits: fast, tends to fragment the front.
    First,
    /// Smallest block that fits: scans the whole list, keeps big blocks intact.
    Best,
}

pub struct FreeListAllocator {
    start: usize,
    end: usize,
    head: *mut FreeNode,
    fit: Fit,
    stats: AllocStats,
}

// SAFETY: the allocator exclusively owns its region and the raw pointers in
// the list only ever point into that region, so moving it to another
// context does not create shared mutable access.
unsafe impl Send for FreeListAllocator {}

/// Where a request would be placed inside one free block.
struct Placement {
    hdr: usize,
    front: usize,
    need: usize,
}

impl FreeListAllocator {
    pub const fn empty(fit: Fit) -> Self {
        Self { start: 0, end: 0, head: null_mut(), fit, stats: AllocStats::new() }
    }

    /// # Safety
    /// `[start, start + size)` must be valid, writable memory that nothing
    /// else uses for as long as this allocator manages it.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        let s = align_up(start, HEADER);
        let e = (start + size) & !(HEADER - 1);
        self.start = s;
        self.end = e;
        self.head = null_mut();
        self.stats = AllocStats { capacity: e.saturating_sub(s), ..AllocStats::new() };
        if e > s && e - s >= MIN_BLOCK {
            let node = s as *mut FreeNode;
            node.write(FreeNode { size: e - s, next: null_mut() });
            self.head = node;
        }
    }

    pub fn fit(&self) -> Fit {
        self.fit
    }

    /// Heap map: split the region into `out.len()` equal slices and write how
    /// full each one is, 0 (all free) to 255 (no free byte), by walking the
    /// free list. Reads only; allocates nothing, so it is safe to call while
    /// the heap lock is held. Block headers count as used.
    pub fn occupancy(&self, out: &mut [u8]) {
        let n = out.len();
        if n == 0 {
            return;
        }
        let span = self.end.saturating_sub(self.start);
        if span == 0 {
            out.fill(0);
            return;
        }
        // Free bytes per slice (at most 256 slices, a fixed stack array, so
        // nothing is allocated); each free block is split across the slices
        // it spans.
        let slice = span.div_ceil(n);
        let mut free = [0usize; 256];
        let buckets = n.min(free.len());
        let mut node = self.head;
        while !node.is_null() {
            // SAFETY: every node on the list was written by insert_free/init
            // inside [start, end) and is not part of a live block; the list is
            // only changed through &mut self, which this &self call excludes.
            let (addr, size, next) = unsafe { (node as usize, (*node).size, (*node).next) };
            let (mut a, b) = (addr - self.start, addr - self.start + size);
            while a < b {
                let i = (a / slice).min(buckets - 1);
                let edge = ((i + 1) * slice).min(b);
                free[i] += edge - a;
                a = edge;
            }
            node = next;
        }
        for (i, o) in out.iter_mut().enumerate().take(buckets) {
            let len = slice.min(span - (i * slice).min(span)).max(1);
            let used = len.saturating_sub(free[i].min(len));
            *o = ((used * 255 + len / 2) / len) as u8;
        }
        out[buckets..].fill(0);
    }

    /// Try to place `layout` in the free block `[block, block + size)`.
    fn place(block: usize, size: usize, layout: Layout) -> Option<Placement> {
        let align = layout.align().max(HEADER);
        let need = HEADER + align_up(layout.size().max(1), HEADER);
        let mut payload = align_up(block + HEADER, align);
        // A gap in front of the header must be 0 or big enough to stay a free block.
        while {
            let front = payload - HEADER - block;
            front != 0 && front < MIN_BLOCK
        } {
            payload += align;
        }
        let hdr = payload - HEADER;
        let end = hdr.checked_add(need)?;
        (end <= block + size).then_some(Placement { hdr, front: hdr - block, need })
    }

    /// Insert `[addr, addr + size)` into the address-ordered list and merge it
    /// with adjacent free neighbours.
    ///
    /// # Safety
    /// The range must lie inside the region and not be part of a live block.
    unsafe fn insert_free(&mut self, addr: usize, size: usize) -> Result<(), FreeError> {
        let mut prev: *mut FreeNode = null_mut();
        let mut cur = self.head;
        while !cur.is_null() && (cur as usize) < addr {
            prev = cur;
            cur = (*cur).next;
        }
        // Overlap with a neighbour means a double free or corrupted metadata.
        if !prev.is_null() && prev as usize + (*prev).size > addr {
            return Err(FreeError::Corrupt);
        }
        if !cur.is_null() && addr + size > cur as usize {
            return Err(FreeError::Corrupt);
        }
        let node = addr as *mut FreeNode;
        node.write(FreeNode { size, next: cur });
        if prev.is_null() {
            self.head = node;
        } else {
            (*prev).next = node;
        }
        if !cur.is_null() && addr + size == cur as usize {
            (*node).size += (*cur).size;
            (*node).next = (*cur).next;
        }
        if !prev.is_null() && prev as usize + (*prev).size == addr {
            (*prev).size += (*node).size;
            (*prev).next = (*node).next;
        }
        Ok(())
    }

    /// Validate `ptr` and put its block back on the free list.
    ///
    /// # Safety
    /// The header read at `ptr - HEADER` is only trusted after the range and
    /// alignment checks prove it lies inside our region; the magic check then
    /// rejects anything that is not a live block. The caller must not use
    /// `ptr` again if the free succeeds.
    unsafe fn free_block(&mut self, ptr: *mut u8) -> Result<usize, FreeError> {
        let p = ptr as usize;
        if p < self.start + HEADER || p >= self.end {
            return Err(FreeError::OutOfRange);
        }
        if p % HEADER != 0 {
            return Err(FreeError::Misaligned);
        }
        let hdr = (p - HEADER) as *mut Header;
        if (*hdr).magic != LIVE_MAGIC {
            return Err(FreeError::NotAllocated);
        }
        let size = (*hdr).size;
        if size < MIN_BLOCK || size % HEADER != 0 || hdr as usize + size > self.end {
            return Err(FreeError::Corrupt);
        }
        // Writing the FreeNode overwrites `magic`, so a second free is caught.
        self.insert_free(hdr as usize, size)?;
        Ok(size)
    }

    /// Walk the free list: (block count, largest block, total free bytes).
    fn scan(&self) -> (usize, usize, usize) {
        let (mut n, mut largest, mut total) = (0, 0, 0);
        let mut cur = self.head;
        while !cur.is_null() {
            // SAFETY: list nodes are written only by `init`/`insert_free`
            // inside the region this allocator owns.
            let (size, next) = unsafe { ((*cur).size, (*cur).next) };
            n += 1;
            largest = largest.max(size);
            total += size;
            cur = next;
        }
        (n, largest, total)
    }
}

impl RegionAllocator for FreeListAllocator {
    fn name(&self) -> &'static str {
        match self.fit {
            Fit::First => "first-fit",
            Fit::Best => "best-fit",
        }
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        // SAFETY: every node reached from `head` was written by `init` or
        // `insert_free` inside our region; blocks we split are removed from
        // the list before their memory is reused.
        unsafe {
            let mut prev: *mut FreeNode = null_mut();
            let mut cur = self.head;
            let mut chosen: Option<(*mut FreeNode, *mut FreeNode, Placement)> = None;
            let mut best_size = usize::MAX;
            while !cur.is_null() {
                let size = (*cur).size;
                if let Some(p) = Self::place(cur as usize, size, layout) {
                    match self.fit {
                        Fit::First => {
                            chosen = Some((prev, cur, p));
                            break;
                        }
                        Fit::Best if size < best_size => {
                            best_size = size;
                            chosen = Some((prev, cur, p));
                        }
                        Fit::Best => {}
                    }
                }
                prev = cur;
                cur = (*cur).next;
            }
            let Some((prev, node, p)) = chosen else {
                self.stats.failed += 1;
                return null_mut();
            };

            let block = node as usize;
            let block_size = (*node).size;
            if prev.is_null() {
                self.head = (*node).next;
            } else {
                (*prev).next = (*node).next;
            }

            let mut used = p.need;
            let tail = block + block_size - (p.hdr + p.need);
            if tail >= MIN_BLOCK {
                let _ = self.insert_free(p.hdr + p.need, tail);
            } else {
                used += tail; // too small to track: hand it out with the block
            }
            if p.front > 0 {
                let _ = self.insert_free(block, p.front);
            }
            (p.hdr as *mut Header).write(Header { size: used, magic: LIVE_MAGIC });
            self.stats.record_alloc(used);
            (p.hdr + HEADER) as *mut u8
        }
    }

    // Safety: the Allocator::dealloc contract (no use of `ptr` afterwards).
    // free_block checks range, alignment and the header magic before
    // writing anything, so a bad pointer is refused rather than freed.
    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) -> Result<(), FreeError> {
        match self.free_block(ptr) {
            Ok(size) => {
                self.stats.frees += 1;
                self.stats.bytes_in_use -= size;
                Ok(())
            }
            Err(e) => {
                self.stats.rejected_frees += 1;
                Err(e)
            }
        }
    }

    fn stats(&self) -> AllocStats {
        let (free_blocks, largest_free, total_free) = self.scan();
        AllocStats { free_blocks, largest_free, total_free, ..self.stats }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::testutil::Arena;
    use crate::rng::XorShift64;

    fn new(arena: &mut Arena, size: usize, fit: Fit) -> FreeListAllocator {
        let mut a = FreeListAllocator::empty(fit);
        unsafe { a.init(arena.start(), size) };
        a
    }

    fn l(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn alloc_free_coalesces_back_to_one_block() {
        let mut arena = Arena::new(4096);
        let mut a = new(&mut arena, 4096, Fit::First);
        let ptrs: Vec<_> = (0..8).map(|i| a.alloc(l(40 + i * 8, 8))).collect();
        assert!(ptrs.iter().all(|p| !p.is_null()));
        // free in an interleaved order to force merges on both sides
        for &i in &[1, 3, 5, 7, 0, 2, 6, 4] {
            unsafe { a.dealloc(ptrs[i], l(1, 1)).unwrap() };
        }
        let s = a.stats();
        assert_eq!((s.free_blocks, s.largest_free, s.bytes_in_use), (1, 4096, 0));
        assert_eq!(s.fragmentation_bp(), 0);
    }

    #[test]
    fn honours_large_alignment() {
        let mut arena = Arena::new(64 * 1024);
        let mut a = new(&mut arena, 64 * 1024, Fit::First);
        let small = a.alloc(l(24, 8));
        for align in [32, 64, 256, 4096] {
            let p = a.alloc(l(100, align));
            assert!(!p.is_null());
            assert_eq!(p as usize % align, 0, "align {align}");
        }
        assert!(!small.is_null());
    }

    #[test]
    fn rejects_double_foreign_and_misaligned_frees() {
        let mut arena = Arena::new(1024);
        let mut a = new(&mut arena, 1024, Fit::First);
        let p = a.alloc(l(64, 8));
        let q = a.alloc(l(64, 8));
        unsafe {
            assert_eq!(a.dealloc(p, l(64, 8)), Ok(()));
            assert_eq!(a.dealloc(p, l(64, 8)), Err(FreeError::NotAllocated));
            assert_eq!(a.dealloc(q.add(8), l(64, 8)), Err(FreeError::Misaligned));
            let mut local = 0u8;
            assert_eq!(a.dealloc(&mut local, l(1, 1)), Err(FreeError::OutOfRange));
            assert_eq!(a.dealloc(q, l(64, 8)), Ok(()));
        }
        assert_eq!(a.stats().rejected_frees, 3);
        assert_eq!(a.stats().free_blocks, 1);
    }

    #[test]
    fn exhaustion_returns_null_and_recovers() {
        let mut arena = Arena::new(1024);
        let mut a = new(&mut arena, 1024, Fit::First);
        let mut live = Vec::new();
        loop {
            let p = a.alloc(l(100, 8));
            if p.is_null() {
                break;
            }
            live.push(p);
        }
        assert!(a.stats().failed >= 1);
        assert!(a.alloc(l(2000, 8)).is_null());
        for p in live {
            unsafe { a.dealloc(p, l(100, 8)).unwrap() };
        }
        assert_eq!(a.stats().largest_free, 1024);
    }

    #[test]
    fn best_fit_picks_the_tightest_hole() {
        for (fit, expect_small_hole) in [(Fit::First, false), (Fit::Best, true)] {
            let mut arena = Arena::new(8192);
            let mut a = new(&mut arena, 8192, fit);
            // Layout: [big 512][sep][small 96][sep][rest]
            let big = a.alloc(l(496, 16));
            let _s1 = a.alloc(l(16, 16));
            let small = a.alloc(l(80, 16));
            let _s2 = a.alloc(l(16, 16));
            unsafe {
                a.dealloc(big, l(1, 1)).unwrap();
                a.dealloc(small, l(1, 1)).unwrap();
            }
            let p = a.alloc(l(64, 16));
            assert_eq!(p == small, expect_small_hole, "{fit:?}");
        }
    }

    /// Random alloc/free against a shadow model: blocks never overlap and
    /// their contents are never clobbered by allocator metadata.
    #[test]
    fn randomized_against_shadow_model() {
        for fit in [Fit::First, Fit::Best] {
            let mut arena = Arena::new(256 * 1024);
            let mut a = new(&mut arena, 256 * 1024, fit);
            let mut rng = XorShift64::new(7);
            let mut live: Vec<(*mut u8, usize, u8)> = Vec::new();
            for step in 0..20_000u32 {
                if live.is_empty() || (live.len() < 300 && rng.chance(55)) {
                    let size = rng.range(1, 3000) as usize;
                    let align = 1usize << rng.range(0, 7);
                    let p = a.alloc(l(size, align));
                    if p.is_null() {
                        continue;
                    }
                    assert_eq!(p as usize % align, 0);
                    let tag = step as u8;
                    unsafe { core::ptr::write_bytes(p, tag, size) };
                    live.push((p, size, tag));
                } else {
                    let i = rng.range(0, live.len() as u64) as usize;
                    let (p, size, tag) = live.swap_remove(i);
                    let ok = unsafe { core::slice::from_raw_parts(p, size) }.iter().all(|&b| b == tag);
                    assert!(ok, "payload corrupted");
                    unsafe { a.dealloc(p, l(size, 1)).unwrap() };
                }
            }
            for (p, size, _) in live.drain(..) {
                unsafe { a.dealloc(p, l(size, 1)).unwrap() };
            }
            let s = a.stats();
            assert_eq!((s.free_blocks, s.bytes_in_use, s.rejected_frees), (1, 0, 0));
        }
    }

    #[test]
    fn occupancy_maps_used_and_free_slices() {
        let mut buf = vec![0u8; 64 * 1024 + 16];
        let mut a = FreeListAllocator::empty(Fit::First);
        unsafe { a.init(buf.as_mut_ptr() as usize, buf.len()) };
        let mut map = [0u8; 8];
        a.occupancy(&mut map);
        assert!(map.iter().all(|&v| v == 0), "empty heap must map as free: {map:?}");
        // First-fit fills from the front: about one slice of 8 KiB.
        let p = a.alloc(Layout::from_size_align(8 * 1024 - 64, 16).unwrap());
        assert!(!p.is_null());
        a.occupancy(&mut map);
        assert!(map[0] > 240, "front slice should be (nearly) full: {map:?}");
        assert!(map[2..].iter().all(|&v| v == 0), "rest stays free: {map:?}");
        unsafe { a.dealloc(p, Layout::from_size_align(8 * 1024 - 64, 16).unwrap()).unwrap() };
        a.occupancy(&mut map);
        assert!(map.iter().all(|&v| v == 0), "freed heap maps as free again: {map:?}");
    }
}
