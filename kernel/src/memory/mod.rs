//! Layer 8 — memory management: the kernel heap.
//!
//! The region after the kernel image (`__heap_start`, from the linker script)
//! is handed to a first-fit [`FreeListAllocator`] and registered as Rust's
//! `#[global_allocator]`, so `Box`, `Vec`, `Arc` and task stacks all come
//! from it.

use core::alloc::{GlobalAlloc, Layout};

use microos_core::mem::{AllocStats, Fit, FreeError, FreeListAllocator, RegionAllocator};

use crate::sync::IrqSpinLock;

pub const HEAP_SIZE: usize = 16 * 1024 * 1024;

extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
    static __heap_start: u8;
}

pub struct KernelHeap {
    inner: IrqSpinLock<FreeListAllocator>,
}

#[global_allocator]
static HEAP: KernelHeap = KernelHeap { inner: IrqSpinLock::new("heap", FreeListAllocator::empty(Fit::First)) };

/// Hand the heap region to the allocator. Returns (start, size).
pub fn init() -> (usize, usize) {
    let start = core::ptr::addr_of!(__heap_start) as usize;
    // SAFETY: [start, start + HEAP_SIZE) lies in RAM above the kernel image
    // (see linker.ld; QEMU runs with 128 MiB) and is used by nothing else.
    unsafe { HEAP.inner.lock().init(start, HEAP_SIZE) };
    (start, HEAP_SIZE)
}

/// Kernel image bounds (start, end).
pub fn kernel_image() -> (usize, usize) {
    (core::ptr::addr_of!(__kernel_start) as usize, core::ptr::addr_of!(__kernel_end) as usize)
}

pub fn stats() -> AllocStats {
    HEAP.inner.lock().stats()
}

/// Free through the kernel heap and report whether the request was accepted
/// (`GlobalAlloc::dealloc` cannot return an error).
///
/// # Safety
/// If `ptr` is a live heap allocation it must not be used afterwards.
pub unsafe fn checked_free(ptr: *mut u8, layout: Layout) -> Result<(), FreeError> {
    HEAP.inner.lock().dealloc(ptr, layout)
}

// SAFETY: blocks returned by `alloc` are inside the heap region, aligned as
// requested (FreeListAllocator::place), and never handed out twice while
// live; the IrqSpinLock serialises every call, including ones made from the
// timer interrupt path. `dealloc` validates the pointer before reusing it.
unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = self.inner.lock().alloc(layout);
        if p.is_null() {
            kwarn!("heap: allocation of {} bytes (align {}) failed", layout.size(), layout.align());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Err(e) = self.inner.lock().dealloc(ptr, layout) {
            kerror!("heap: rejected free of {:p}: {:?}", ptr, e);
        }
    }
}
