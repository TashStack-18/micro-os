//! Workloads and metrics of the R3 study ("Fairness vs Throughput in CPU
//! Scheduling"), reproduced so the kernel can run the same experiment on
//! real scheduling instead of a simulator.
//!
//! * Burst times: **uniform** U(2, 30) ms, or **bimodal** 75 % U(1, 6) ms
//!   + 25 % U(20, 80) ms (integer milliseconds = timer ticks).
//! * Arrivals: a Poisson process with rate lambda = rho / mean burst. The
//!   kernel's clock is quantised to 1 ms ticks, so arrivals are generated
//!   per tick with probability lambda (a Bernoulli process, the discrete-time
//!   form of Poisson arrivals). At lambda <= 0.073 per ms the chance of two
//!   arrivals in the same millisecond, which this cannot produce, is < 0.3 %.
//! * Fairness: Jain's index J = (sum x)^2 / (n * sum x^2), computed in
//!   integer basis points (10000 = perfectly fair).

use alloc::vec::Vec;

use crate::rng::XorShift64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dist {
    Uniform,
    Bimodal,
}

impl Dist {
    pub fn name(self) -> &'static str {
        match self {
            Dist::Uniform => "uniform",
            Dist::Bimodal => "bimodal",
        }
    }

    /// Mean burst in thousandths of a tick: U{2..30} -> 16.0;
    /// 0.75 * U{1..6} + 0.25 * U{20..80} -> 0.75 * 3.5 + 0.25 * 50 = 15.125.
    pub fn mean_milli(self) -> u64 {
        match self {
            Dist::Uniform => 16_000,
            Dist::Bimodal => 15_125,
        }
    }

    pub fn burst(self, rng: &mut XorShift64) -> u32 {
        match self {
            Dist::Uniform => rng.range(2, 31) as u32,
            Dist::Bimodal if rng.range(0, 100) < 75 => rng.range(1, 7) as u32,
            Dist::Bimodal => rng.range(20, 81) as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Job {
    /// Arrival time in ticks after the start of the run.
    pub arrival: u64,
    /// CPU burst in ticks.
    pub burst: u32,
}

/// Jobs arriving in `[0, window)` ticks at offered load `load_pct` / 100.
/// The same seed always produces the same list, so every policy is compared
/// on identical input (as in the study).
pub fn generate(dist: Dist, load_pct: u64, window: u64, seed: u64) -> Vec<Job> {
    let mut arrivals = XorShift64::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5);
    let mut bursts = XorShift64::new(seed.wrapping_add(0x5151_5151));
    // Probability of an arrival in one tick, in parts per million.
    let p_ppm = load_pct * 10_000 * 1000 / dist.mean_milli();
    (0..window)
        .filter(|_| arrivals.range(0, 1_000_000) < p_ppm)
        .map(|t| Job { arrival: t, burst: dist.burst(&mut bursts) })
        .collect()
}

/// Jain's fairness index in basis points, given sums over n allocations.
pub fn jain_bp(n: u64, sum: u64, sum_sq: u128) -> u64 {
    if n == 0 || sum_sq == 0 {
        return 10_000;
    }
    ((sum as u128 * sum as u128 * 10_000) / (n as u128 * sum_sq)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offered_load_matches_rho() {
        for dist in [Dist::Uniform, Dist::Bimodal] {
            for load in [70u64, 90, 110] {
                let window = 200_000;
                let jobs = generate(dist, load, window, 1);
                let work: u64 = jobs.iter().map(|j| j.burst as u64).sum();
                let rho_pct = work * 100 / window;
                assert!(rho_pct.abs_diff(load) <= 3, "{dist:?} load {load}: measured {rho_pct}");
            }
        }
    }

    #[test]
    fn bimodal_mix_is_75_25_and_deterministic() {
        let jobs = generate(Dist::Bimodal, 110, 100_000, 7);
        let short = jobs.iter().filter(|j| j.burst <= 6).count() as f64 / jobs.len() as f64;
        assert!((0.72..0.78).contains(&short), "short fraction {short}");
        assert!(jobs.iter().all(|j| (1..=6).contains(&j.burst) || (20..=80).contains(&j.burst)));
        assert_eq!(jobs, generate(Dist::Bimodal, 110, 100_000, 7));
        assert_ne!(jobs, generate(Dist::Bimodal, 110, 100_000, 8));
    }

    #[test]
    fn jain_index_extremes() {
        // Equal shares -> 1.0
        assert_eq!(jain_bp(4, 40, 4 * 100), 10_000);
        // One task gets everything -> 1/n
        assert_eq!(jain_bp(4, 10, 100), 2_500);
        assert_eq!(jain_bp(0, 0, 0), 10_000);
    }
}
