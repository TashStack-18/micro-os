//! Exercises the kernel heap (the free-list allocator) from a program.

use alloc::vec::Vec;

use super::arg_u64;
use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    let kib = arg_u64(args, 0, 256) as usize;
    let before = sys::heap_used();
    let mut buf: Vec<u8> = Vec::new();
    if buf.try_reserve_exact(kib * 1024).is_err() {
        println!("memtest: could not allocate {} KiB (heap exhausted)", kib);
        return 1;
    }
    buf.resize(kib * 1024, 0);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let ok = buf.iter().enumerate().all(|(i, &b)| b == (i % 251) as u8);
    let during = sys::heap_used();
    drop(buf);
    let after = sys::heap_used();
    println!("memtest: {} KiB written and verified: {}", kib, if ok { "OK" } else { "CORRUPTED" });
    println!("  heap in use: before={} B  during={} B  after={} B", before, during, after);
    if ok {
        0
    } else {
        2
    }
}
