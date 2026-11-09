//! CPU scheduling policies.
//!
//! A policy owns only the *ready queue*: which runnable task goes next and
//! for how long. The kernel owns tasks, contexts and the timer, and calls
//! into the policy at well-defined points:
//!
//! ```text
//!   task becomes runnable ──► enqueue(info, reason)
//!   CPU is free           ──► pick_next() ──► on_dispatch(info, level) ──► run for quantum(info)
//!   every timer tick      ──► on_tick(); should_preempt(running)?
//!   task leaves the CPU   ──► on_descheduled(info, ticks_ran, why)
//! ```
//!
//! Keeping policies free of hardware details lets them be unit-tested on the
//! host and swapped at boot (`policy=rr|prio|aging|mlfq|fcfs|sjf|srtf|hrrn`).
//!
//! FCFS, SJF, SRTF and HRRN are the policies compared in the R3 research
//! study ("Fairness vs Throughput in CPU Scheduling"); see `classic.rs`.

mod classic;
mod mlfq;
mod priority;
mod rr;

use alloc::boxed::Box;

pub use classic::{Fcfs, Hrrn, Sjf, Srtf, RUN_TO_COMPLETION};
pub use mlfq::{Mlfq, BOOST_PERIOD, MLFQ_LEVELS};
pub use priority::{PriorityPolicy, AGING_INTERVAL, DEFAULT_PRIO, LOWEST_PRIO};
pub use rr::RoundRobin;

pub type Tid = usize;

/// Why a task is being put on the ready queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    New,
    Preempted,
    Yielded,
    Woken,
}

/// Why a task left the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    Preempted,
    Yielded,
    Blocked,
    Exited,
}

impl Leave {
    pub fn as_str(self) -> &'static str {
        match self {
            Leave::Preempted => "preempt",
            Leave::Yielded => "yield",
            Leave::Blocked => "block",
            Leave::Exited => "exit",
        }
    }
}

/// Per-task scheduling state that lives in the kernel's task control block
/// and is handed to the policy whenever it needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedInfo {
    pub tid: Tid,
    /// Static priority, 0 = highest.
    pub base_prio: u8,
    /// Level the task was dispatched at (priority or MLFQ queue).
    pub level: u8,
    /// MLFQ: ticks consumed at the current level (the "allotment").
    pub allot_used: u32,
    /// MLFQ: boost epoch this info was last synchronised with.
    pub epoch: u64,
    /// Expected length of the current CPU burst, in ticks. Either the true
    /// value (an "oracle", as in the R3 simulator) or a prediction made by
    /// exponential averaging of past bursts (what a real OS has to do).
    pub burst_est: u32,
    /// Ticks of CPU consumed in the current burst.
    pub burst_used: u32,
}

impl SchedInfo {
    pub fn new(tid: Tid, base_prio: u8) -> Self {
        Self { tid, base_prio, ..Self::default() }
    }

    /// Expected CPU still needed in the current burst (SRTF's key).
    pub fn remaining(&self) -> u32 {
        self.burst_est.saturating_sub(self.burst_used)
    }

    /// End of a CPU burst in prediction mode: tau' = (actual + tau) / 2,
    /// i.e. exponential averaging with alpha = 1/2 (Silberschatz ch. 5).
    pub fn end_burst_predicted(&mut self) {
        self.burst_est = ((self.burst_used + self.burst_est) / 2).max(1);
        self.burst_used = 0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub tid: Tid,
    pub level: u8,
}

/// Notable policy decisions, reported to the kernel trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyEvent {
    Demoted { tid: Tid, from: u8, to: u8 },
    Boosted { moved: usize },
    Aged { count: usize },
}

pub trait Policy: Send {
    fn name(&self) -> &'static str;

    /// Add a runnable task to the ready queue.
    fn enqueue(&mut self, info: &mut SchedInfo, reason: Reason, now: u64);

    /// Remove and return the task that should run next.
    fn pick_next(&mut self, now: u64) -> Option<Pick>;

    /// Called right after `pick_next` with the picked task's info.
    fn on_dispatch(&mut self, info: &mut SchedInfo, level: u8) {
        info.level = level;
    }

    /// Time slice, in timer ticks, for a task being dispatched.
    fn quantum(&self, info: &SchedInfo) -> u32;

    /// Account for a task leaving the CPU after running `ran` ticks.
    fn on_descheduled(&mut self, _info: &mut SchedInfo, _ran: u32, _why: Leave) -> Option<PolicyEvent> {
        None
    }

    /// Should the running task be preempted before its quantum expires?
    fn should_preempt(&self, _running: &SchedInfo) -> bool {
        false
    }

    /// Periodic housekeeping (aging, priority boost).
    fn on_tick(&mut self, _now: u64) -> Option<PolicyEvent> {
        None
    }

    fn ready_len(&self) -> usize;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyKind {
    RoundRobin,
    Priority,
    PriorityAging,
    Mlfq,
    Fcfs,
    Sjf,
    Srtf,
    Hrrn,
}

impl PolicyKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "rr" | "roundrobin" => Some(Self::RoundRobin),
            "prio" | "priority" => Some(Self::Priority),
            "aging" | "prio-aging" => Some(Self::PriorityAging),
            "mlfq" => Some(Self::Mlfq),
            "fcfs" => Some(Self::Fcfs),
            "sjf" => Some(Self::Sjf),
            "srtf" => Some(Self::Srtf),
            "hrrn" => Some(Self::Hrrn),
            _ => None,
        }
    }

    /// Policies that order the queue by (expected) burst length.
    pub fn needs_bursts(self) -> bool {
        matches!(self, Self::Sjf | Self::Srtf | Self::Hrrn)
    }
}

pub fn build(kind: PolicyKind, quantum: u32) -> Box<dyn Policy> {
    match kind {
        PolicyKind::RoundRobin => Box::new(RoundRobin::new(quantum)),
        PolicyKind::Priority => Box::new(PriorityPolicy::new(quantum, None)),
        PolicyKind::PriorityAging => Box::new(PriorityPolicy::new(quantum, Some(AGING_INTERVAL))),
        PolicyKind::Mlfq => Box::new(Mlfq::new(quantum, BOOST_PERIOD)),
        PolicyKind::Fcfs => Box::new(Fcfs::new()),
        PolicyKind::Sjf => Box::new(Sjf::new()),
        PolicyKind::Srtf => Box::new(Srtf::new()),
        PolicyKind::Hrrn => Box::new(Hrrn::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names() {
        assert_eq!(PolicyKind::parse("mlfq"), Some(PolicyKind::Mlfq));
        assert_eq!(PolicyKind::parse("rr"), Some(PolicyKind::RoundRobin));
        assert_eq!(PolicyKind::parse("fifo"), None);
        assert_eq!(PolicyKind::parse("srtf"), Some(PolicyKind::Srtf));
        assert_eq!(build(PolicyKind::PriorityAging, 4).name(), "PRIO+AGING");
        assert_eq!(build(PolicyKind::Hrrn, 4).name(), "HRRN");
    }

    #[test]
    fn exponential_averaging_tracks_bursts() {
        let mut i = SchedInfo { burst_est: 8, ..SchedInfo::default() };
        for _ in 0..6 {
            i.burst_used = 2; // the task keeps running for 2 ticks per burst
            i.end_burst_predicted();
        }
        assert_eq!(i.burst_est, 2, "estimate converges to the observed burst");
        i.burst_used = 30;
        i.end_burst_predicted();
        assert_eq!(i.burst_est, 16, "one long burst moves the estimate half way");
    }
}
