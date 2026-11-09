//! Round Robin: one FIFO ready queue and a fixed time quantum.

use alloc::collections::VecDeque;

use super::{Pick, Policy, Reason, SchedInfo, Tid};

pub struct RoundRobin {
    ready: VecDeque<Tid>,
    quantum: u32,
}

impl RoundRobin {
    pub fn new(quantum: u32) -> Self {
        Self { ready: VecDeque::new(), quantum: quantum.max(1) }
    }
}

impl Policy for RoundRobin {
    fn name(&self) -> &'static str {
        "RR"
    }

    fn enqueue(&mut self, info: &mut SchedInfo, _reason: Reason, _now: u64) {
        self.ready.push_back(info.tid);
    }

    fn pick_next(&mut self, _now: u64) -> Option<Pick> {
        self.ready.pop_front().map(|tid| Pick { tid, level: 0 })
    }

    fn quantum(&self, _info: &SchedInfo) -> u32 {
        self.quantum
    }

    fn ready_len(&self) -> usize {
        self.ready.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_rotation() {
        let mut p = RoundRobin::new(0);
        assert_eq!(p.quantum(&SchedInfo::default()), 1, "quantum is clamped to >= 1");
        let mut infos: Vec<_> = (1..=3).map(|t| SchedInfo::new(t, 4)).collect();
        for i in &mut infos {
            p.enqueue(i, Reason::New, 0);
        }
        let first = p.pick_next(0).unwrap().tid;
        p.enqueue(&mut infos[0], Reason::Preempted, 1);
        let order: Vec<_> = core::iter::from_fn(|| p.pick_next(0).map(|x| x.tid)).collect();
        assert_eq!((first, order), (1, vec![2, 3, 1]));
        assert_eq!(p.ready_len(), 0);
    }
}
