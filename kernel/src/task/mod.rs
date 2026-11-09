//! Layer 3 — task representation (the task control block).

mod stack;

use alloc::boxed::Box;

use microos_core::sched::{Leave, SchedInfo, Tid};

pub use stack::{KernelStack, STACK_SIZE};

core::arch::global_asm!(include_str!("switch.S"));

extern "C" {
    /// Save callee-saved registers into `save`, load them from `load`, and
    /// continue at `load.ra` on `load.sp`.
    pub fn __switch_context(save: *mut Context, load: *const Context);
}

/// Callee-saved register state; layout must match `switch.S`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 12],
}

impl Context {
    pub const fn zero() -> Self {
        Self { ra: 0, sp: 0, s: [0; 12] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Blocked => "BLOCKED",
            Self::Terminated => "TERMINATED",
        }
    }
}

pub type Entry = Box<dyn FnOnce() + Send + 'static>;

/// Per-task measurements, all timestamps in microseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct TaskMetrics {
    pub arrived: bool,
    pub arrival_us: u64,
    pub first_run_us: Option<u64>,
    pub finish_us: u64,
    pub cpu_ticks: u64,
    pub dispatches: u64,
    pub preemptions: u64,
    pub yields: u64,
    pub blocks: u64,
    pub ready_since_us: u64,
    pub wait_us: u64,
    pub max_wait_us: u64,
}

pub struct Task {
    pub tid: Tid,
    pub name: &'static str,
    pub state: TaskState,
    /// Saved registers while the task is not running.
    pub ctx: Context,
    pub stack: KernelStack,
    /// The task body; taken (and run) on first dispatch.
    pub entry: Option<Entry>,
    pub sched: SchedInfo,
    /// Ticks allowed in the current slice / ticks used so far.
    pub quantum: u32,
    pub slice_used: u32,
    /// Why the task last left (or is about to leave) the CPU.
    pub leave: Leave,
    /// Tick at which a sleeping task becomes ready.
    pub wake_at: u64,
    /// Set by Ctrl-C in the shell; programs check it cooperatively.
    pub cancel: bool,
    pub m: TaskMetrics,
}

impl Task {
    /// A fresh task whose first dispatch "returns" into `trampoline` on its
    /// own stack.
    pub fn new(tid: Tid, name: &'static str, prio: u8, entry: Entry, stack: KernelStack, trampoline: usize) -> Self {
        let ctx = Context { ra: trampoline, sp: stack.top(), s: [0; 12] };
        Self {
            tid,
            name,
            state: TaskState::Blocked,
            ctx,
            stack,
            entry: Some(entry),
            sched: SchedInfo::new(tid, prio),
            quantum: 0,
            slice_used: 0,
            leave: Leave::Yielded,
            wake_at: 0,
            cancel: false,
            m: TaskMetrics::default(),
        }
    }
}
