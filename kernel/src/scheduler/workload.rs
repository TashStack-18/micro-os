//! Synthetic task bodies with controlled CPU and sleep behaviour.

use super::{current_cpu_ticks, current_cpu_us, sleep_ticks, yield_now};

#[derive(Clone, Copy, Debug)]
pub enum Work {
    /// Compute for `n` timer ticks without ever yielding.
    Cpu(u64),
    /// Compute for `n` ticks, yielding after every tick (a well-behaved
    /// cooperative task).
    CoopCpu(u64),
    /// Interactive/I-O-like: `rounds` x (compute `cpu` ticks, sleep `sleep` ticks).
    Interactive { rounds: u32, cpu: u64, sleep: u64 },
}

impl Work {
    pub fn run(self) {
        match self {
            Work::Cpu(n) => cpu_burst(n),
            Work::CoopCpu(n) => {
                for _ in 0..n {
                    cpu_burst(1);
                    yield_now();
                }
            }
            Work::Interactive { rounds, cpu, sleep } => {
                for _ in 0..rounds {
                    cpu_burst(cpu);
                    sleep_ticks(sleep);
                }
            }
        }
    }

    /// CPU ticks this workload needs.
    pub fn cpu_demand(self) -> u64 {
        match self {
            Work::Cpu(n) | Work::CoopCpu(n) => n,
            Work::Interactive { rounds, cpu, .. } => rounds as u64 * cpu,
        }
    }
}

/// Busy-loop until the calling task has been charged `ticks` more timer
/// ticks. Charging happens in the timer interrupt, so this measures CPU time
/// actually received, not wall-clock time.
pub fn cpu_burst(ticks: u64) {
    let start = current_cpu_ticks();
    while current_cpu_ticks() - start < ticks {
        spin(500);
    }
}

/// Busy-loop until the calling task has received `us` microseconds of CPU,
/// measured exactly. Used by the research workload, where time spent in
/// context switches must not count as the job's own work.
pub fn cpu_burst_us(us: u64) {
    let start = current_cpu_us();
    while current_cpu_us() - start < us {
        spin(200);
    }
}

/// Burn roughly `iters` loop iterations of CPU.
#[inline(never)]
pub fn spin(iters: u32) {
    for i in 0..iters {
        core::hint::black_box(i);
        core::hint::spin_loop();
    }
}
