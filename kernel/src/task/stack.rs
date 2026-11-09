//! Heap-allocated kernel stacks with overflow detection and usage measurement.
//!
//! On creation the whole stack is painted with a byte pattern and a canary
//! word is written at the lowest address. The scheduler checks the canary
//! every time the task leaves the CPU (overflow detection), and on exit the
//! untouched paint tells us the task's peak stack usage.

use alloc::alloc::{alloc, dealloc};
use core::alloc::Layout;
use core::ptr::{self, NonNull};

pub const STACK_SIZE: usize = 16 * 1024;
const PAINT: u8 = 0x5A;
const CANARY: u64 = 0x57AC_C0DE_DEAD_10CC;
const ALIGN: usize = 16;

pub struct KernelStack {
    base: NonNull<u8>,
    size: usize,
}

// SAFETY: the stack memory is exclusively owned by this value.
unsafe impl Send for KernelStack {}

impl KernelStack {
    pub fn new(size: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size, ALIGN).ok()?;
        // SAFETY: layout has non-zero size.
        let base = NonNull::new(unsafe { alloc(layout) })?;
        // SAFETY: we own the `size` freshly allocated bytes; the canary write
        // is 8-byte aligned because the block is 16-byte aligned.
        unsafe {
            ptr::write_bytes(base.as_ptr(), PAINT, size);
            (base.as_ptr() as *mut u64).write(CANARY);
        }
        Some(Self { base, size })
    }

    /// Initial stack pointer: the (16-byte-aligned) end of the block.
    pub fn top(&self) -> usize {
        self.base.as_ptr() as usize + self.size
    }

    pub fn canary_intact(&self) -> bool {
        // SAFETY: the first 8 bytes of our allocation, aligned.
        unsafe { (self.base.as_ptr() as *const u64).read_volatile() == CANARY }
    }

    /// Peak bytes ever used. Call only while the task is not running.
    pub fn high_water(&self) -> usize {
        // SAFETY: our allocation, skipping the canary word; the owning task is
        // not running, so nothing writes the memory concurrently.
        let body = unsafe { core::slice::from_raw_parts(self.base.as_ptr().add(8), self.size - 8) };
        let untouched = body.iter().take_while(|&&b| b == PAINT).count();
        self.size - 8 - untouched
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // SAFETY: allocated in `new` with exactly this layout.
        unsafe { dealloc(self.base.as_ptr(), Layout::from_size_align_unchecked(self.size, ALIGN)) };
    }
}
