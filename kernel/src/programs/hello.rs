use crate::sys;

pub fn main(args: &[&str]) -> i32 {
    println!("Hello from a Rust program running on the Micro-OS!");
    println!("  my task id (pid) : {}", sys::getpid());
    println!("  uptime           : {} ms", sys::uptime_ms());
    if args.is_empty() {
        println!("  no arguments (try: run hello world 42)");
    }
    for (i, a) in args.iter().enumerate() {
        println!("  arg[{}]           : {}", i, a);
    }
    0
}
