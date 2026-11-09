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
//! host and swapped at boot (`policy=rr|prio|aging|mlfq`).

mod mlfq;
mod priority;
mod rr;

use alloc::boxed::Box;

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
}

impl SchedInfo {
    pub fn new(tid: Tid, base_prio: u8) -> Self {
        Self { tid, base_prio, ..Self::default() }
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
}

impl PolicyKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "rr" | "roundrobin" => Some(Self::RoundRobin),
            "prio" | "priority" => Some(Self::Priority),
            "aging" | "prio-aging" => Some(Self::PriorityAging),
            "mlfq" => Some(Self::Mlfq),
            _ => None,
        }
    }
}

pub fn build(kind: PolicyKind, quantum: u32) -> Box<dyn Policy> {
    match kind {
        PolicyKind::RoundRobin => Box::new(RoundRobin::new(quantum)),
        PolicyKind::Priority => Box::new(PriorityPolicy::new(quantum, None)),
        PolicyKind::PriorityAging => Box::new(PriorityPolicy::new(quantum, Some(AGING_INTERVAL))),
        PolicyKind::Mlfq => Box::new(Mlfq::new(quantum, BOOST_PERIOD)),
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
        assert_eq!(build(PolicyKind::PriorityAging, 4).name(), "PRIO+AGING");
    }
}
