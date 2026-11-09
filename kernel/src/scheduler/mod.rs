//! Layers 4–6 — ready queue, dispatcher and timer-driven preemption.
//!
//! Structure (xv6-style): the boot context in `kmain` becomes the
//! *scheduler loop*. Tasks never switch directly to each other; a task that
//! stops running (preempted, yielded, blocked, exited) switches back to the
//! scheduler loop, which retires it and dispatches the next one:
//!
//! ```text
//!   scheduler loop ──pick_next()──► __switch_context ──► task runs
//!         ▲                                                 │
//!         └──── retire_current() ◄── __switch_context ◄─────┘
//!                                    (timer tick / yield / block / exit)
//! ```
//!
//! Which task is next is decided by a pluggable [`Policy`] from
//! `microos-core` (RR, priority, priority+aging, MLFQ).
//!
//! Concurrency model: one hart; kernel state lives in `SCHED`, an
//! `IrqSpinLock`, and every path that switches context runs with interrupts
//! disabled. The lock is never held across a context switch.

pub mod workload;

use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::cell::UnsafeCell;

use microos_core::rng::Fnv1a;
use microos_core::sched::{self, Leave, Pick, Policy, PolicyEvent, PolicyKind, Reason, Tid, LOWEST_PRIO};

use crate::sync::{IrqGuard, IrqSpinLock};
use crate::task::{Context, KernelStack, Task, TaskState, __switch_context, STACK_SIZE};
use crate::{arch, timer};

pub const MAX_TASKS: usize = 64;
const ORDER_CAP: usize = 8192;

#[derive(Clone, Debug)]
pub struct Config {
    pub label: String,
    pub policy: PolicyKind,
    pub quantum: u32,
    pub preemptive: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SpawnOpts {
    pub prio: u8,
    pub delay_ticks: u64,
}

impl Default for SpawnOpts {
    fn default() -> Self {
        Self { prio: sched::DEFAULT_PRIO, delay_ticks: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnError {
    TooManyTasks,
    OutOfMemory,
    NotInitialised,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SchedStats {
    pub ticks: u64,
    pub idle_ticks: u64,
    pub dispatches: u64,
    pub preemptions: u64,
    pub yields: u64,
    pub blocks: u64,
    pub wakeups: u64,
    pub policy_events: u64,
    /// Time spent inside the scheduler loop (picking + retiring), raw units.
    pub sched_raw: u64,
}

#[derive(Clone, Debug)]
pub struct TaskReport {
    pub tid: Tid,
    pub name: &'static str,
    pub prio: u8,
    pub arrival_us: u64,
    pub first_run_us: u64,
    pub finish_us: u64,
    pub cpu_ticks: u64,
    pub dispatches: u64,
    pub preemptions: u64,
    pub yields: u64,
    pub blocks: u64,
    pub wait_us: u64,
    pub max_wait_us: u64,
    pub stack_used: usize,
}

impl TaskReport {
    fn from_task(t: &Task) -> Self {
        Self {
            tid: t.tid,
            name: t.name,
            prio: t.sched.base_prio,
            arrival_us: t.m.arrival_us,
            first_run_us: t.m.first_run_us.unwrap_or(t.m.finish_us),
            finish_us: t.m.finish_us,
            cpu_ticks: t.m.cpu_ticks,
            dispatches: t.m.dispatches,
            preemptions: t.m.preemptions,
            yields: t.m.yields,
            blocks: t.m.blocks,
            wait_us: t.m.wait_us,
            max_wait_us: t.m.max_wait_us,
            stack_used: t.stack.high_water(),
        }
    }

    pub fn turnaround_us(&self) -> u64 {
        self.finish_us - self.arrival_us
    }

    pub fn response_us(&self) -> u64 {
        self.first_run_us - self.arrival_us
    }

    /// Mean time spent in the ready queue per dispatch (scheduling latency).
    pub fn latency_us(&self) -> u64 {
        self.wait_us / self.dispatches.max(1)
    }
}

pub struct RunReport {
    pub label: String,
    pub policy: &'static str,
    pub quantum: u32,
    pub preemptive: bool,
    pub stats: SchedStats,
    /// Finished tasks in completion order.
    pub tasks: Vec<TaskReport>,
    /// Dispatch sequence (tids), capped at ORDER_CAP entries.
    pub order: Vec<Tid>,
    pub elapsed_us: u64,
    pub deadlock: bool,
}

impl RunReport {
    pub fn task(&self, name: &str) -> Option<&TaskReport> {
        self.tasks.iter().find(|t| t.name == name)
    }

    fn avg(&self, f: impl Fn(&TaskReport) -> u64) -> u64 {
        if self.tasks.is_empty() {
            return 0;
        }
        self.tasks.iter().map(f).sum::<u64>() / self.tasks.len() as u64
    }

    pub fn avg_turnaround_us(&self) -> u64 {
        self.avg(TaskReport::turnaround_us)
    }

    pub fn avg_response_us(&self) -> u64 {
        self.avg(TaskReport::response_us)
    }

    pub fn avg_wait_us(&self) -> u64 {
        self.avg(|t| t.wait_us)
    }

    /// Fingerprint of the dispatch sequence, for repeatability checks.
    pub fn order_hash(&self) -> u64 {
        let mut h = Fnv1a::new();
        self.order.iter().for_each(|&t| h.write_u64(t as u64));
        h.finish()
    }

    pub fn completion_order(&self) -> Vec<&'static str> {
        self.tasks.iter().map(|t| t.name).collect()
    }
}

struct Sched {
    cfg: Config,
    policy: Box<dyn Policy>,
    tasks: BTreeMap<Tid, Box<Task>>,
    current: Option<Tid>,
    /// Blocked tasks waiting for a tick (sleep or delayed arrival).
    sleepers: Vec<Tid>,
    next_tid: Tid,
    stats: SchedStats,
    finished: Vec<TaskReport>,
    order: Vec<Tid>,
    start_us: u64,
    deadlock: bool,
}

static SCHED: IrqSpinLock<Option<Sched>> = IrqSpinLock::new("scheduler", None);

/// Saved context of the scheduler loop (it runs on the boot stack in kmain).
struct SchedulerContext(UnsafeCell<Context>);

// SAFETY: only read/written by __switch_context on the single hart, with
// interrupts disabled, from either the scheduler loop or the task leaving it.
unsafe impl Sync for SchedulerContext {}

static SCHEDULER_CTX: SchedulerContext = SchedulerContext(UnsafeCell::new(Context::zero()));

// ---------------------------------------------------------------------------
// Run lifecycle (called from kmain / scenarios)
// ---------------------------------------------------------------------------

/// Start a new scheduling run with a fresh task table and policy.
pub fn begin(cfg: Config) {
    let policy = sched::build(cfg.policy, cfg.quantum);
    ktrace!("RUN_BEGIN", "run={} policy={} q={} preempt={}", cfg.label, policy.name(), cfg.quantum, cfg.preemptive as u8);
    let mut g = SCHED.lock();
    assert!(g.is_none(), "scheduler::begin while a run is active");
    *g = Some(Sched {
        cfg,
        policy,
        tasks: BTreeMap::new(),
        current: None,
        sleepers: Vec::new(),
        next_tid: 1,
        stats: SchedStats::default(),
        finished: Vec::new(),
        order: Vec::new(),
        start_us: timer::now_us(),
        deadlock: false,
    });
}

/// Create a task running `f`. It becomes READY now, or after `delay_ticks`.
pub fn spawn<F>(name: &'static str, opts: SpawnOpts, f: F) -> Result<Tid, SpawnError>
where
    F: FnOnce() + Send + 'static,
{
    if SCHED.lock().is_none() {
        return Err(SpawnError::NotInitialised);
    }
    // Allocate before taking the scheduler lock.
    let stack = KernelStack::new(STACK_SIZE).ok_or(SpawnError::OutOfMemory)?;
    let entry = Box::new(f);
    let mut g = SCHED.lock();
    let s = g.as_mut().ok_or(SpawnError::NotInitialised)?;
    if s.tasks.len() >= MAX_TASKS {
        ktrace!("SPAWN_REJECT", "name={} reason=too_many_tasks live={}", name, s.tasks.len());
        return Err(SpawnError::TooManyTasks);
    }
    let tid = s.next_tid;
    s.next_tid += 1;
    let prio = opts.prio.min(LOWEST_PRIO);
    let mut task = Box::new(Task::new(tid, name, prio, entry, stack, task_trampoline as extern "C" fn() -> ! as usize));
    ktrace!("SPAWN", "tid={} name={} prio={} delay={}", tid, name, prio, opts.delay_ticks);
    if opts.delay_ticks > 0 {
        task.wake_at = timer::ticks() + opts.delay_ticks;
        s.sleepers.push(tid);
        s.tasks.insert(tid, task);
    } else {
        s.tasks.insert(tid, task);
        make_ready(s, tid, Reason::New);
    }
    Ok(tid)
}

/// Run the scheduler loop until every task has terminated (or deadlocked),
/// then tear the run down and return its measurements.
pub fn run() -> RunReport {
    let _irq = IrqGuard::new();
    loop {
        match pick() {
            Step::Dispatch(ctx) => {
                // SAFETY: `ctx` points into a boxed Task that stays alive and
                // unmoved while it is `current`; SCHEDULER_CTX is ours; the
                // scheduler lock is released and interrupts are off.
                unsafe { __switch_context(SCHEDULER_CTX.0.get(), ctx) };
                retire_current();
            }
            Step::Idle => {
                // Nothing runnable but some task is sleeping: wait for the
                // next tick. wfi wakes on a pending enabled interrupt even
                // with SIE clear; we then briefly enable interrupts to take it.
                arch::wfi();
                arch::intr_on();
                arch::intr_off();
            }
            Step::Deadlock => {
                report_deadlock();
                break;
            }
            Step::Done => break,
        }
    }
    finish()
}

enum Step {
    Dispatch(*const Context),
    Idle,
    Deadlock,
    Done,
}

fn pick() -> Step {
    let t0 = timer::now_raw();
    let mut g = SCHED.lock();
    let s = g.as_mut().expect("scheduler::run without begin");
    if s.tasks.is_empty() {
        return Step::Done;
    }
    let Some(Pick { tid, level }) = s.policy.pick_next(timer::ticks()) else {
        return if s.sleepers.is_empty() { Step::Deadlock } else { Step::Idle };
    };
    let now = timer::now_us();
    let t = s.tasks.get_mut(&tid).expect("policy picked an unknown tid");
    assert_eq!(t.state, TaskState::Ready, "policy picked tid {} which is {}", tid, t.state.as_str());
    s.policy.on_dispatch(&mut t.sched, level);
    t.state = TaskState::Running;
    t.quantum = s.policy.quantum(&t.sched);
    t.slice_used = 0;
    let waited = now - t.m.ready_since_us;
    t.m.wait_us += waited;
    t.m.max_wait_us = t.m.max_wait_us.max(waited);
    t.m.first_run_us.get_or_insert(now);
    t.m.dispatches += 1;
    s.current = Some(tid);
    s.stats.dispatches += 1;
    if s.order.len() < ORDER_CAP {
        s.order.push(tid);
    }
    ktrace!("DISPATCH", "tid={} name={} lvl={} q={} waited={}", tid, t.name, level, t.quantum, waited);
    let ctx: *const Context = &t.ctx;
    s.stats.sched_raw += timer::now_raw() - t0;
    Step::Dispatch(ctx)
}

/// The current task has just switched back to us: account for it and put
/// it wherever its new state says it belongs.
fn retire_current() {
    let t0 = timer::now_raw();
    let mut exited: Option<Box<Task>> = None;
    {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("scheduler state vanished");
        let tid = s.current.take().expect("switched back without a current task");
        let t = s.tasks.get_mut(&tid).expect("current task missing");
        if !t.stack.canary_intact() {
            panic!("stack overflow in task {} '{}': canary overwritten", tid, t.name);
        }
        let (ran, why, state) = (t.slice_used, t.leave, t.state);
        if let Some(ev) = s.policy.on_descheduled(&mut t.sched, ran, why) {
            trace_policy_event(ev);
            s.stats.policy_events += 1;
        }
        ktrace!("OFF", "tid={} why={} ran={}", tid, why.as_str(), ran);
        match state {
            TaskState::Ready => {
                let reason = if why == Leave::Preempted {
                    t.m.preemptions += 1;
                    s.stats.preemptions += 1;
                    Reason::Preempted
                } else {
                    t.m.yields += 1;
                    s.stats.yields += 1;
                    Reason::Yielded
                };
                make_ready(s, tid, reason);
            }
            TaskState::Blocked => {
                t.m.blocks += 1;
                s.stats.blocks += 1;
            }
            TaskState::Terminated => {
                let task = s.tasks.remove(&tid).expect("terminated task missing");
                let r = TaskReport::from_task(&task);
                ktrace!(
                    "EXIT",
                    "tid={} name={} prio={} arrival={} first={} finish={} cpu={} disp={} pre={} yld={} blk={} wait={} maxwait={} stack={}",
                    r.tid, r.name, r.prio, r.arrival_us, r.first_run_us, r.finish_us, r.cpu_ticks,
                    r.dispatches, r.preemptions, r.yields, r.blocks, r.wait_us, r.max_wait_us, r.stack_used
                );
                s.finished.push(r);
                exited = Some(task);
            }
            TaskState::Running => panic!("task {} returned to the scheduler still RUNNING", tid),
        }
        s.stats.sched_raw += timer::now_raw() - t0;
    }
    // Free the stack outside the scheduler lock.
    drop(exited);
}

/// Every remaining task is BLOCKED on a lock or semaphore and none is
/// sleeping: nothing can ever wake them. Report and abandon the run.
fn report_deadlock() {
    let abandoned: Vec<Box<Task>> = {
        let mut g = SCHED.lock();
        let s = g.as_mut().unwrap();
        kerror!("DEADLOCK: {} task(s) blocked forever, nothing runnable or sleeping", s.tasks.len());
        for t in s.tasks.values() {
            kerror!("  tid={} name={} state={}", t.tid, t.name, t.state.as_str());
        }
        ktrace!("DEADLOCK", "blocked={}", s.tasks.len());
        s.deadlock = true;
        core::mem::take(&mut s.tasks).into_values().collect()
    };
    // Their stacks are freed; values living on those stacks (lock guards,
    // Arc clones) are leaked, since their frames never unwind.
    drop(abandoned);
}

fn finish() -> RunReport {
    let s = SCHED.lock().take().expect("scheduler::finish without a run");
    let elapsed_us = timer::now_us() - s.start_us;
    let st = s.stats;
    let sched_ns = timer::raw_to_ns(st.sched_raw) / st.dispatches.max(1);
    ktrace!(
        "RUN_END",
        "run={} elapsed_us={} dispatches={} preemptions={} yields={} blocks={} wakeups={} ticks={} idle={} sched_ns={} deadlock={}",
        s.cfg.label, elapsed_us, st.dispatches, st.preemptions, st.yields, st.blocks, st.wakeups,
        st.ticks, st.idle_ticks, sched_ns, s.deadlock as u8
    );
    RunReport {
        label: s.cfg.label,
        policy: s.policy.name(),
        quantum: s.cfg.quantum,
        preemptive: s.cfg.preemptive,
        stats: st,
        tasks: s.finished,
        order: s.order,
        elapsed_us,
        deadlock: s.deadlock,
    }
}

fn make_ready(s: &mut Sched, tid: Tid, reason: Reason) {
    let now = timer::now_us();
    let t = s.tasks.get_mut(&tid).expect("make_ready: unknown tid");
    let reason = if t.m.arrived {
        reason
    } else {
        t.m.arrived = true;
        t.m.arrival_us = now;
        Reason::New
    };
    t.state = TaskState::Ready;
    t.m.ready_since_us = now;
    s.policy.enqueue(&mut t.sched, reason, timer::ticks());
}

fn trace_policy_event(ev: PolicyEvent) {
    match ev {
        PolicyEvent::Demoted { tid, from, to } => ktrace!("DEMOTE", "tid={} from={} to={}", tid, from, to),
        PolicyEvent::Boosted { moved } => ktrace!("BOOST", "moved={}", moved),
        PolicyEvent::Aged { count } => ktrace!("AGE", "count={}", count),
    }
}

// ---------------------------------------------------------------------------
// Timer interrupt (called from trap_handler with interrupts disabled)
// ---------------------------------------------------------------------------

pub fn on_timer_interrupt() {
    let tick = timer::handle_tick();
    let mut resched = false;
    {
        let mut g = SCHED.lock();
        let Some(s) = g.as_mut() else { return };
        s.stats.ticks += 1;

        let mut i = 0;
        while i < s.sleepers.len() {
            let tid = s.sleepers[i];
            if s.tasks[&tid].wake_at <= tick {
                s.sleepers.remove(i);
                s.stats.wakeups += 1;
                ktrace!("WAKE", "tid={} by=timer", tid);
                make_ready(s, tid, Reason::Woken);
            } else {
                i += 1;
            }
        }

        if let Some(ev) = s.policy.on_tick(tick) {
            trace_policy_event(ev);
            s.stats.policy_events += 1;
        }

        match s.current {
            None => s.stats.idle_ticks += 1,
            Some(tid) => {
                let t = s.tasks.get_mut(&tid).expect("current task missing");
                t.m.cpu_ticks += 1;
                t.slice_used += 1;
                let expired = t.slice_used >= t.quantum;
                if s.cfg.preemptive && t.state == TaskState::Running && (expired || s.policy.should_preempt(&t.sched)) {
                    t.state = TaskState::Ready;
                    t.leave = Leave::Preempted;
                    resched = true;
                }
            }
        }
    }
    if resched {
        // Returns when this task is dispatched again; trap.S then restores
        // its registers and `sret`s back into the interrupted code.
        switch_to_scheduler();
    }
}

// ---------------------------------------------------------------------------
// Task-side API
// ---------------------------------------------------------------------------

/// First code a new task executes (its initial `ra`).
extern "C" fn task_trampoline() -> ! {
    let entry = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("task started without a run");
        let tid = s.current.expect("task started without being current");
        s.tasks.get_mut(&tid).and_then(|t| t.entry.take())
    };
    // The scheduler dispatched us with interrupts off; tasks run with them on.
    arch::intr_on();
    if let Some(f) = entry {
        f();
    }
    exit_current()
}

/// Save the running task's context and resume the scheduler loop.
/// The caller has already set the task's next state; interrupts must be off.
pub(crate) fn switch_to_scheduler() {
    debug_assert!(!arch::intr_enabled(), "context switch with interrupts enabled");
    let ctx: *mut Context = {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("switch without a run");
        let tid = s.current.expect("switch_to_scheduler outside a task");
        &mut s.tasks.get_mut(&tid).expect("current task missing").ctx as *mut Context
    };
    // SAFETY: the boxed Task is not freed or moved while it is current;
    // SCHEDULER_CTX holds the loop's state saved by run(); interrupts are
    // off and the scheduler lock is released, so nothing observes the
    // half-switched state.
    unsafe { __switch_context(ctx, SCHEDULER_CTX.0.get()) };
}

pub fn exit_current() -> ! {
    arch::intr_off();
    {
        let mut g = SCHED.lock();
        let s = g.as_mut().unwrap();
        let tid = s.current.expect("exit outside a task");
        let t = s.tasks.get_mut(&tid).unwrap();
        t.state = TaskState::Terminated;
        t.leave = Leave::Exited;
        t.m.finish_us = timer::now_us();
    }
    switch_to_scheduler();
    unreachable!("a terminated task was resumed");
}

/// Give up the CPU voluntarily; the task stays READY.
pub fn yield_now() {
    let _irq = IrqGuard::new();
    {
        let mut g = SCHED.lock();
        let Some(s) = g.as_mut() else { return };
        let Some(tid) = s.current else { return };
        let t = s.tasks.get_mut(&tid).unwrap();
        t.state = TaskState::Ready;
        t.leave = Leave::Yielded;
    }
    switch_to_scheduler();
}

/// Block for `ticks` timer ticks.
pub fn sleep_ticks(ticks: u64) {
    if ticks == 0 {
        yield_now();
        return;
    }
    let _irq = IrqGuard::new();
    {
        let mut g = SCHED.lock();
        let s = g.as_mut().expect("sleep outside a run");
        let tid = s.current.expect("sleep outside a task");
        let t = s.tasks.get_mut(&tid).unwrap();
        t.state = TaskState::Blocked;
        t.leave = Leave::Blocked;
        t.wake_at = timer::ticks() + ticks;
        s.sleepers.push(tid);
    }
    switch_to_scheduler();
}

/// Mark the current task BLOCKED. The caller (a sync primitive) must have
/// queued it somewhere that will `wake` it, keep interrupts disabled, and
/// call `switch_to_scheduler` next.
pub(crate) fn mark_current_blocked() {
    let mut g = SCHED.lock();
    let s = g.as_mut().expect("block outside a run");
    let tid = s.current.expect("block outside a task");
    let t = s.tasks.get_mut(&tid).unwrap();
    t.state = TaskState::Blocked;
    t.leave = Leave::Blocked;
}

/// Make a BLOCKED task READY (no-op for any other state).
pub(crate) fn wake(tid: Tid) {
    let mut g = SCHED.lock();
    let Some(s) = g.as_mut() else { return };
    if s.tasks.get(&tid).map(|t| t.state) != Some(TaskState::Blocked) {
        return;
    }
    s.sleepers.retain(|&x| x != tid);
    s.stats.wakeups += 1;
    ktrace!("WAKE", "tid={} by={}", tid, s.current.unwrap_or(0));
    make_ready(s, tid, Reason::Woken);
}

pub fn current_tid() -> Option<Tid> {
    SCHED.lock().as_ref().and_then(|s| s.current)
}

/// CPU ticks consumed so far by the calling task.
pub fn current_cpu_ticks() -> u64 {
    let g = SCHED.lock();
    g.as_ref().and_then(|s| s.current.map(|tid| s.tasks[&tid].m.cpu_ticks)).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Introspection (used by the shell)
// ---------------------------------------------------------------------------

/// One row of `ps`.
#[derive(Clone, Copy, Debug)]
pub struct TaskInfo {
    pub tid: Tid,
    pub name: &'static str,
    pub state: TaskState,
    pub prio: u8,
    pub level: u8,
    pub cpu_ticks: u64,
    pub dispatches: u64,
}

pub fn snapshot() -> Vec<TaskInfo> {
    let g = SCHED.lock();
    let Some(s) = g.as_ref() else { return Vec::new() };
    s.tasks
        .values()
        .map(|t| TaskInfo {
            tid: t.tid,
            name: t.name,
            state: t.state,
            prio: t.sched.base_prio,
            level: t.sched.level,
            cpu_ticks: t.m.cpu_ticks,
            dispatches: t.m.dispatches,
        })
        .collect()
}

/// Policy name, quantum, preemptive flag, live statistics and ready-queue
/// length of the active run.
pub fn run_info() -> Option<(&'static str, u32, bool, SchedStats, usize)> {
    let g = SCHED.lock();
    g.as_ref().map(|s| (s.policy.name(), s.cfg.quantum, s.cfg.preemptive, s.stats, s.policy.ready_len()))
}

pub fn is_alive(tid: Tid) -> bool {
    SCHED.lock().as_ref().is_some_and(|s| s.tasks.contains_key(&tid))
}

/// Ask a task to stop. Cooperative: the task notices via `cancel_requested`.
pub fn request_cancel(tid: Tid) -> bool {
    let mut g = SCHED.lock();
    match g.as_mut().and_then(|s| s.tasks.get_mut(&tid)) {
        Some(t) => {
            t.cancel = true;
            true
        }
        None => false,
    }
}

pub fn cancel_requested() -> bool {
    let g = SCHED.lock();
    g.as_ref().and_then(|s| s.current.map(|tid| s.tasks[&tid].cancel)).unwrap_or(false)
}

/// For the panic handler: `None` if the scheduler lock is held, otherwise
/// the current task if there is one.
pub fn try_current() -> Option<Option<(Tid, &'static str)>> {
    let g = SCHED.try_lock()?;
    Some(g.as_ref().and_then(|s| s.current.map(|tid| (tid, s.tasks[&tid].name))))
}
