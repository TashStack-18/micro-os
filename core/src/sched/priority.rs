//! Preemptive static-priority scheduling, optionally with aging.
//!
//! Lower number = higher priority. Equal priorities run round-robin (FIFO by
//! enqueue order). Without aging a steady stream of high-priority work
//! starves low-priority tasks; with aging every `interval` ticks spent
//! waiting raises a task's effective priority by one level, and the raised
//! level is kept for the slice it is dispatched with.

use alloc::vec::Vec;

use super::{Pick, Policy, PolicyEvent, Reason, SchedInfo, Tid};

pub const LOWEST_PRIO: u8 = 7;
pub const DEFAULT_PRIO: u8 = 4;
pub const AGING_INTERVAL: u64 = 10;

struct Entry {
    tid: Tid,
    eff: u8,
    since: u64,
    seq: u64,
}

pub struct PriorityPolicy {
    ready: Vec<Entry>,
    quantum: u32,
    aging: Option<u64>,
    seq: u64,
}

impl PriorityPolicy {
    pub fn new(quantum: u32, aging: Option<u64>) -> Self {
        Self { ready: Vec::new(), quantum: quantum.max(1), aging, seq: 0 }
    }
}

impl Policy for PriorityPolicy {
    fn name(&self) -> &'static str {
        if self.aging.is_some() {
            "PRIO+AGING"
        } else {
            "PRIO"
        }
    }

    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, now: u64) {
        let eff = info.base_prio.min(LOWEST_PRIO);
        self.ready.push(Entry { tid: info.tid, eff, since: now, seq: self.seq });
        self.seq += 1;
    }

    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        // Linear scan: the ready set is at most MAX_TASKS entries.
        let i = self
            .ready
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| (e.eff, e.seq))
            .map(|(i, _)| i)?;
        let e = self.ready.swap_remove(i);
        Some(Pick { tid: e.tid, level: e.eff })
    }

    fn quantum(&self, _info: &SchedInfo) -> u32 {
        self.quantum
    }

    fn should_preempt(&self, running: &SchedInfo) -> bool {
        self.ready.iter().any(|e| e.eff < running.level)
    }

    fn on_tick(&mut self, now: u64) -> Option<PolicyEvent> {
        let interval = self.aging?;
        let mut count = 0;
        for e in &mut self.ready {
            if e.eff > 0 && now.saturating_sub(e.since) >= interval {
                e.eff -= 1;
                e.since = now;
                count += 1;
            }
        }
        (count > 0).then_some(PolicyEvent::Aged { count })
    }

    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(tid: Tid, prio: u8) -> SchedInfo {
        SchedInfo::new(tid, prio)
    }

    #[test]
    fn highest_priority_first_fifo_within_level() {
        let mut p = PriorityPolicy::new(4, None);
        for (tid, prio) in [(1, 5), (2, 1), (3, 5), (4, 1)] {
            p.enqueue(&mut info(tid, prio), Reason::New, 0);
        }
        let order: Vec<_> = core::iter::from_fn(|| p.pick_next(0).map(|x| x.tid)).collect();
        assert_eq!(order, vec![2, 4, 1, 3]);
    }

    #[test]
    fn preempts_for_higher_priority_only() {
        let mut p = PriorityPolicy::new(4, None);
        let mut running = info(1, 3);
        running.level = 3;
        p.enqueue(&mut info(2, 3), Reason::New, 0);
        assert!(!p.should_preempt(&running));
        p.enqueue(&mut info(3, 2), Reason::Woken, 0);
        assert!(p.should_preempt(&running));
    }

    #[test]
    fn aging_lifts_a_starved_task() {
        let mut p = PriorityPolicy::new(4, Some(10));
        p.enqueue(&mut info(9, 7), Reason::New, 0);
        let mut ev = None;
        for now in 1..=50 {
            ev = p.on_tick(now).or(ev);
        }
        assert_eq!(ev, Some(PolicyEvent::Aged { count: 1 }));
        p.enqueue(&mut info(1, 3), Reason::New, 50);
        let pick = p.pick_next(50).unwrap();
        assert_eq!((pick.tid, pick.level), (9, 2), "aged from 7 to 2 after 50 ticks");
    }

    #[test]
    fn no_aging_without_interval() {
        let mut p = PriorityPolicy::new(4, None);
        p.enqueue(&mut info(9, 7), Reason::New, 0);
        assert_eq!(p.on_tick(1000), None);
        assert_eq!(p.pick_next(1000).unwrap().level, 7);
    }
}
