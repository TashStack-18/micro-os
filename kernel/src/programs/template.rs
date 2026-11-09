//! Copy this file to start your own program (see programs/mod.rs).
//!
//! What you can use:
//!   println!(...) / print!(...)      write to the console
//!   sys::getpid()                    your task id
//!   sys::sleep_ms(ms) -> bool        sleep; false if Ctrl-C was pressed
//!   sys::yield_now()                 let another task run
//!   sys::interrupted() -> bool       check for Ctrl-C in long loops
//!   sys::uptime_ms(), sys::cpu_ticks(), sys::heap_used()
//!   super::arg_u64(args, i, default) read a numeric argument
//!   alloc::vec::Vec, alloc::string::String, ...  heap collections
//!
//! Return 0 for success, anything else for an error.

use super::arg_u64;
use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    let times = arg_u64(args, 0, 3);
    for i in 1..=times {
        println!("template (pid {}): step {} of {}", sys::getpid(), i, times);
        if !sys::sleep_ms(200) {
            return 130;
        }
    }
    0
}
