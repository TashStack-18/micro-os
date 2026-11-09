//! Hardware-independent core of the Micro-OS.
//!
//! Everything in this crate is plain `no_std` Rust with no hardware access,
//! so the same code is unit-tested on the development host (`cargo test`)
//! and linked unchanged into the RISC-V kernel.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod args;
pub mod fdt;
pub mod mem;
pub mod research;
pub mod rng;
pub mod sched;
pub mod spin;
