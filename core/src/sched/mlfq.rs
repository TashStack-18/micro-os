//! Multi-Level Feedback Queue (after OSTEP ch. 8).
//!
//! * Rule 1/2: higher queue wins; equal queues run round-robin.
//! * Rule 3: new tasks start in the top queue.
//! * Rule 4: once a task has used its allotment at a level (summed across
//!   yields and blocks, so it cannot game the scheduler by yielding just
//!   before the quantum ends) it moves down one level.
//! * Rule 5: every `boost_period` ticks all tasks return to the top queue.
//!
//! Level `l` has quantum `base << l`, and its allotment equals that quantum.
//! The boost is implemented with an epoch counter so tasks that are running
//! or blocked at boost time are reset lazily the next time we see them.

use alloc::collections::VecDeque;

use super::{Leave, Pick, Policy, PolicyEvent, Reason, SchedInfo, Tid};

pub const MLFQ_LEVELS: usize = 3;
pub const BOOST_PERIOD: u64 = 50;

pub struct Mlfq {
    queues: [VecDeque<Tid>; MLFQ_LEVELS],
    base_quantum: u32,
    boost_period: u64,
    last_boost: u64,
    epoch: u64,
}

impl Mlfq {
    pub fn new(base_quantum: u32, boost_period: u64) -> Self {
        Self {
            queues: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            base_quantum: base_quantum.max(1),
            boost_period: boost_period.max(1),
            last_boost: 0,
            epoch: 0,
        }
    }

    fn quantum_at(&self, level: u8) -> u32 {
        self.base_quantum << level
    }

    /// Bring `info` up to date with the latest boost. Returns true if it was stale.
    fn sync_epoch(&self, info: &mut SchedInfo) -> bool {
        if info.epoch == self.epoch {
            return false;
        }
        info.epoch = self.epoch;
        info.level = 0;
        info.allot_used = 0;
        true
    }
}

impl Policy for Mlfq {
    fn name(&self) -> &'static str {
        "MLFQ"
    }

    fn enqueue(&mut self, info: &mut SchedInfo, reason: Reason, _now: u64) {
        self.sync_epoch(info);
        if reason == Reason::New {
            info.level = 0;
            info.allot_used = 0;
        }
        let level = (info.level as usize).min(MLFQ_LEVELS - 1);
        self.queues[level].push_back(info.tid);
    }

    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        self.queues
            .iter_mut()
            .enumerate()
            .find_map(|(l, q)| q.pop_front().map(|tid| Pick { tid, level: l as u8 }))
    }

    fn on_dispatch(&mut self, info: &mut SchedInfo, level: u8) {
        if info.epoch != self.epoch {
            info.epoch = self.epoch;
            info.allot_used = 0;
        }
        info.level = level;
    }

    fn quantum(&self, info: &SchedInfo) -> u32 {
        self.quantum_at(info.level)
    }

    fn on_descheduled(&mut self, info: &mut SchedInfo, ran: u32, why: Leave) -> Option<PolicyEvent> {
        if why == Leave::Exited || self.sync_epoch(info) {
            return None;
        }
        info.allot_used += ran;
        if info.allot_used < self.quantum_at(info.level) {
            return None;
        }
        info.allot_used = 0;
        if (info.level as usize) < MLFQ_LEVELS - 1 {
            let from = info.level;
            info.level += 1;
            return Some(PolicyEvent::Demoted { tid: info.tid, from, to: info.level });
        }
        None
    }

    fn should_preempt(&self, running: &SchedInfo) -> bool {
        let level = (running.level as usize).min(MLFQ_LEVELS);
        self.queues[..level].iter().any(|q| !q.is_empty())
    }

    fn on_tick(&mut self, now: u64) -> Option<PolicyEvent> {
        if now.saturating_sub(self.last_boost) < self.boost_period {
            return None;
        }
        self.last_boost = now;
        self.epoch += 1;
        let mut moved = 0;
        for l in 1..MLFQ_LEVELS {
            while let Some(t) = self.queues[l].pop_front() {
                self.queues[0].push_back(t);
                moved += 1;
            }
        }
        Some(PolicyEvent::Boosted { moved })
    }

    fn ready_len(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate one dispatch of `info` that runs `ran` ticks and leaves with `why`.
    fn run(p: &mut Mlfq, info: &mut SchedInfo, ran: u32, why: Leave) -> Option<PolicyEvent> {
        let pick = p.pick_next(0).unwrap();
        assert_eq!(pick.tid, info.tid);
        p.on_dispatch(info, pick.level);
        let ev = p.on_descheduled(info, ran, why);
        if why != Leave::Exited {
            p.enqueue(info, if why == Leave::Preempted { Reason::Preempted } else { Reason::Woken }, 0);
        }
        ev
    }

    #[test]
    fn cpu_bound_task_sinks_level_by_level() {
        let mut p = Mlfq::new(2, 1000);
        let mut t = SchedInfo::new(1, 4);
        p.enqueue(&mut t, Reason::New, 0);
        assert_eq!(p.quantum(&t), 2);
        assert_eq!(run(&mut p, &mut t, 2, Leave::Preempted), Some(PolicyEvent::Demoted { tid: 1, from: 0, to: 1 }));
        assert_eq!(p.quantum(&t), 4);
        assert_eq!(run(&mut p, &mut t, 4, Leave::Preempted), Some(PolicyEvent::Demoted { tid: 1, from: 1, to: 2 }));
        assert_eq!(run(&mut p, &mut t, 8, Leave::Preempted), None, "bottom level stays put");
        assert_eq!(t.level, 2);
    }

    #[test]
    fn yielding_early_cannot_game_the_allotment() {
        let mut p = Mlfq::new(4, 1000);
        let mut t = SchedInfo::new(1, 4);
        p.enqueue(&mut t, Reason::New, 0);
        for _ in 0..3 {
            assert_eq!(run(&mut p, &mut t, 1, Leave::Blocked), None);
        }
        assert_eq!(run(&mut p, &mut t, 1, Leave::Blocked), Some(PolicyEvent::Demoted { tid: 1, from: 0, to: 1 }));
    }

    #[test]
    fn higher_queue_preempts_and_boost_resets() {
        let mut p = Mlfq::new(1, 10);
        let mut hog = SchedInfo::new(1, 4);
        p.enqueue(&mut hog, Reason::New, 0);
        run(&mut p, &mut hog, 1, Leave::Preempted);
        run(&mut p, &mut hog, 2, Leave::Preempted);
        assert_eq!(hog.level, 2);
        let mut fresh = SchedInfo::new(2, 4);
        p.enqueue(&mut fresh, Reason::New, 0);
        assert!(p.should_preempt(&hog));
        assert!(!p.should_preempt(&fresh));

        assert_eq!(p.on_tick(5), None);
        assert_eq!(p.on_tick(10), Some(PolicyEvent::Boosted { moved: 1 }));
        // The hog's info is stale; it is reset to level 0 when next seen.
        let picks: Vec<_> = core::iter::from_fn(|| p.pick_next(10)).collect();
        assert!(picks.iter().all(|x| x.level == 0));
        p.on_dispatch(&mut hog, 0);
        assert_eq!((hog.level, hog.allot_used), (0, 0));
    }
}
