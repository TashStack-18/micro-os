//! The four non-Round-Robin policies compared in the R3 research study
//! ("Fairness vs Throughput in CPU Scheduling"):
//!
//! | policy | picks                                   | preemptive |
//! |--------|-----------------------------------------|------------|
//! | FCFS   | earliest arrival                        | no         |
//! | SJF    | shortest expected burst                 | no         |
//! | SRTF   | shortest expected *remaining* time      | yes, when a shorter job becomes ready |
//! | HRRN   | highest response ratio (w + s) / s      | no         |
//!
//! Non-preemptive policies give the dispatched task a quantum of
//! `RUN_TO_COMPLETION`, so the timer never takes the CPU away; the task
//! keeps it until it blocks or exits, exactly like the study's simulator.
//! Burst lengths come from `SchedInfo::burst_est` (see there).

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::{Pick, Policy, Reason, SchedInfo, Tid};

/// Quantum meaning "never preempt on a timer tick".
pub const RUN_TO_COMPLETION: u32 = u32::MAX;

/// A waiting task with the value it is ordered by.
struct Entry {
    tid: Tid,
    key: u32,
    since: u64,
    seq: u64,
}

/// Remove the entry with the smallest (key, seq): FIFO among equal keys.
fn take_min(ready: &mut Vec<Entry>) -> Option<Entry> {
    let i = ready.iter().enumerate().min_by_key(|(_, e)| (e.key, e.seq)).map(|(i, _)| i)?;
    Some(ready.swap_remove(i))
}

// ------------------------------------------------------------------ FCFS

pub struct Fcfs {
    ready: VecDeque<Tid>,
}

impl Fcfs {
    pub fn new() -> Self {
        Self { ready: VecDeque::new() }
    }
}

impl Default for Fcfs {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for Fcfs {
    fn name(&self) -> &'static str {
        "FCFS"
    }
    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, _now: u64) {
        self.ready.push_back(info.tid);
    }
    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        self.ready.pop_front().map(|tid| Pick { tid, level: 0 })
    }
    fn quantum(&self, _info: &SchedInfo) -> u32 {
        RUN_TO_COMPLETION
    }
    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

// ------------------------------------------------------------------ SJF

/// Non-preemptive Shortest Job First.
pub struct Sjf {
    ready: Vec<Entry>,
    seq: u64,
}

impl Sjf {
    pub fn new() -> Self {
        Self { ready: Vec::new(), seq: 0 }
    }
}

impl Default for Sjf {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for Sjf {
    fn name(&self) -> &'static str {
        "SJF"
    }
    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, now: u64) {
        self.ready.push(Entry { tid: info.tid, key: info.remaining(), since: now, seq: self.seq });
        self.seq += 1;
    }
    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        take_min(&mut self.ready).map(|e| Pick { tid: e.tid, level: 0 })
    }
    fn quantum(&self, _info: &SchedInfo) -> u32 {
        RUN_TO_COMPLETION
    }
    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

// ------------------------------------------------------------------ SRTF

/// Shortest Remaining Time First: SJF that preempts when a task with less
/// remaining work becomes ready (checked on every timer tick).
pub struct Srtf {
    ready: Vec<Entry>,
    seq: u64,
}

impl Srtf {
    pub fn new() -> Self {
        Self { ready: Vec::new(), seq: 0 }
    }
}

impl Default for Srtf {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for Srtf {
    fn name(&self) -> &'static str {
        "SRTF"
    }
    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, now: u64) {
        self.ready.push(Entry { tid: info.tid, key: info.remaining(), since: now, seq: self.seq });
        self.seq += 1;
    }
    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        take_min(&mut self.ready).map(|e| Pick { tid: e.tid, level: 0 })
    }
    fn quantum(&self, _info: &SchedInfo) -> u32 {
        RUN_TO_COMPLETION
    }
    fn should_preempt(&self, running: &SchedInfo) -> bool {
        let left = running.remaining();
        self.ready.iter().any(|e| e.key < left)
    }
    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

// ------------------------------------------------------------------ HRRN

/// Highest Response Ratio Next (Stallings): ratio = (waiting + service) / service.
/// The ratio of a waiting task grows, which is a built-in form of aging.
pub struct Hrrn {
    ready: Vec<Entry>,
    seq: u64,
}

impl Hrrn {
    pub fn new() -> Self {
        Self { ready: Vec::new(), seq: 0 }
    }
}

impl Default for Hrrn {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for Hrrn {
    fn name(&self) -> &'static str {
        "HRRN"
    }
    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, now: u64) {
        self.ready.push(Entry { tid: info.tid, key: info.remaining().max(1), since: now, seq: self.seq });
        self.seq += 1;
    }
    fn pick_next(&mut self, now: u64) -> Option<Pick> {
        // Compare (w_a + s_a) / s_a against (w_b + s_b) / s_b by cross-multiplying,
        // so no floating point is needed.
        let mut best: Option<usize> = None;
        for (i, e) in self.ready.iter().enumerate() {
            let better = match best {
                None => true,
                Some(b) => {
                    let o = &self.ready[b];
                    let lhs = (now - e.since + e.key as u64) as u128 * o.key as u128;
                    let rhs = (now - o.since + o.key as u64) as u128 * e.key as u128;
                    lhs > rhs || (lhs == rhs && e.seq < o.seq)
                }
            };
            if better {
                best = Some(i);
            }
        }
        let e = self.ready.swap_remove(best?);
        Some(Pick { tid: e.tid, level: 0 })
    }
    fn quantum(&self, _info: &SchedInfo) -> u32 {
        RUN_TO_COMPLETION
    }
    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(tid: Tid, burst: u32) -> SchedInfo {
        SchedInfo { tid, burst_est: burst, ..SchedInfo::default() }
    }

    fn drain(p: &mut dyn Policy, now: u64) -> Vec<Tid> {
        core::iter::from_fn(|| p.pick_next(now).map(|x| x.tid)).collect()
    }

    #[test]
    fn fcfs_is_arrival_order_and_never_preempts() {
        let mut p = Fcfs::new();
        for (t, b) in [(1, 30), (2, 2), (3, 10)] {
            p.enqueue(&mut job(t, b), Reason::New, 0);
        }
        assert_eq!(p.quantum(&job(9, 1)), RUN_TO_COMPLETION);
        assert!(!p.should_preempt(&job(9, 1000)));
        assert_eq!(drain(&mut p, 0), vec![1, 2, 3]);
    }

    #[test]
    fn sjf_picks_shortest_fifo_on_ties() {
        let mut p = Sjf::new();
        for (t, b) in [(1, 30), (2, 5), (3, 10), (4, 5)] {
            p.enqueue(&mut job(t, b), Reason::New, 0);
        }
        assert_eq!(drain(&mut p, 0), vec![2, 4, 3, 1]);
    }

    #[test]
    fn srtf_preempts_only_for_strictly_shorter_remaining() {
        let mut p = Srtf::new();
        let mut running = job(1, 20);
        running.burst_used = 12; // 8 ticks left
        p.enqueue(&mut job(2, 8), Reason::New, 0);
        assert!(!p.should_preempt(&running), "equal remaining: keep running");
        p.enqueue(&mut job(3, 7), Reason::New, 0);
        assert!(p.should_preempt(&running));
        // A preempted task re-enters with its remaining time as the key.
        p.enqueue(&mut running, Reason::Preempted, 0);
        assert_eq!(drain(&mut p, 0), vec![3, 2, 1]);
    }

    #[test]
    fn hrrn_lets_a_long_waiting_job_overtake_short_ones() {
        let mut p = Hrrn::new();
        p.enqueue(&mut job(1, 50), Reason::New, 0); // waits from t=0
        p.enqueue(&mut job(2, 4), Reason::New, 100); // arrives at t=100
        // t=101: ratios 1 + 101/50 = 3.02 vs 1 + 1/4 = 1.25: the old long job wins.
        assert_eq!(p.pick_next(101).unwrap().tid, 1);
        p.enqueue(&mut job(1, 50), Reason::New, 101);
        // t=102: job 2 has waited 2 ticks (ratio 1.5) vs job 1's 1.02: short job wins.
        assert_eq!(p.pick_next(102).unwrap().tid, 2);
    }
}
