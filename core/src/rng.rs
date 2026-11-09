//! Deterministic helpers for reproducible experiments.

/// xorshift64 PRNG: tiny, fast and fully deterministic for a given seed,
/// so every workload in the kernel is repeatable run-to-run.
#[derive(Clone, Debug)]
pub struct XorShift64(u64);

impl XorShift64 {
    pub const fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform-ish value in `lo..hi` (`hi` exclusive, requires `hi > lo`).
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi - lo)
    }

    /// True with probability `percent`/100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
}

/// FNV-1a hash, used to fingerprint scheduler traces for repeatability checks.
#[derive(Clone, Copy, Debug)]
pub struct Fnv1a(u64);

impl Fnv1a {
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    pub fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (XorShift64::new(42), XorShift64::new(42));
        for _ in 0..1000 {
            let x = a.range(10, 20);
            assert_eq!(x, b.range(10, 20));
            assert!((10..20).contains(&x));
        }
    }

    #[test]
    fn fnv_distinguishes_order() {
        let mut a = Fnv1a::new();
        let mut b = Fnv1a::new();
        a.write_u64(1);
        a.write_u64(2);
        b.write_u64(2);
        b.write_u64(1);
        assert_ne!(a.finish(), b.finish());
    }
}
