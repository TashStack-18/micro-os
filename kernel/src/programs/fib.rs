use super::arg_u64;

pub fn main(args: &[&str]) -> i32 {
    let n = arg_u64(args, 0, 40);
    if n > 186 {
        println!("fib: n must be <= 186 (larger values overflow u128)");
        return 1;
    }
    let (mut a, mut b) = (0u128, 1u128);
    for i in 0..=n {
        if i < 10 || i == n {
            println!("fib({}) = {}", i, a);
        } else if i == 10 {
            println!("...");
        }
        (a, b) = (b, a + b);
    }
    0
}
