//! Layer 9 extension — user mode and real system calls.
//!
//! A user program runs in RISC-V **U-mode** (the unprivileged level) on its
//! own user stack. The only way it can ask the kernel for anything is the
//! `ecall` instruction, which traps into S-mode:
//!
//! ```text
//!  U-mode program          trap.S (S-mode)            this module
//!  a7 = number, a0.. args
//!  ecall  ───────────────▶ switch to kernel stack,
//!                          save TrapFrame       ────▶ syscall(): check args,
//!                                                     do the work, a0 = result
//!  continues after ecall ◀ restore, sret to U-mode ◀──
//! ```
//!
//! Privileged instructions (CSR access, `sret`, `wfi`) fault in U-mode; the
//! kernel then kills only that program (`kill_current`) and keeps running.
//!
//! Limits, stated honestly: paging is off, so this is *privilege* separation
//! (instructions), not memory isolation. Pointers passed to `write` are
//! range-checked against RAM, but a user program could still read or write
//! kernel memory directly. Memory isolation needs page tables (future work).
//!
//! System call numbers follow the Linux RISC-V ABI where one exists.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicIsize, AtomicUsize, Ordering::Relaxed};

use crate::scheduler::{self, SpawnError, SpawnOpts};
use microos_core::sched::Tid;
use crate::timer;
use crate::trap::TrapFrame;

pub const SYS_WRITE: usize = 64;
pub const SYS_EXIT: usize = 93;
pub const SYS_YIELD: usize = 124;
pub const SYS_GETPID: usize = 172;
/// Not in Linux (which has nanosleep with a struct): sleep for a0 milliseconds.
pub const SYS_SLEEP_MS: usize = 1000;
/// Not in Linux: milliseconds since boot.
pub const SYS_UPTIME_MS: usize = 1001;

pub const EFAULT: isize = -14;
pub const EINVAL: isize = -22;
pub const ENOSYS: isize = -38;
/// Exit code recorded for a program the kernel killed.
pub const KILLED: isize = -9;
/// Exit code for a program stopped with Ctrl-C / `kill` during a system call.
pub const INTERRUPTED: isize = -130;

const MAX_WRITE: usize = 1024;
/// Usable RAM on QEMU virt (128 MiB) above OpenSBI's 2 MiB.
const RAM: core::ops::Range<usize> = 0x8020_0000..0x8800_0000;

fn name_of(nr: usize) -> &'static str {
    match nr {
        SYS_WRITE => "write",
        SYS_EXIT => "exit",
        SYS_YIELD => "yield",
        SYS_GETPID => "getpid",
        SYS_SLEEP_MS => "sleep_ms",
        SYS_UPTIME_MS => "uptime_ms",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------- statistics

pub static SYSCALLS: AtomicUsize = AtomicUsize::new(0);
pub static KILLS: AtomicUsize = AtomicUsize::new(0);
pub static BYTES_WRITTEN: AtomicUsize = AtomicUsize::new(0);

const EXIT_SLOTS: usize = 32;
static EXIT_TID: [AtomicUsize; EXIT_SLOTS] = [const { AtomicUsize::new(0) }; EXIT_SLOTS];
static EXIT_CODE: [AtomicIsize; EXIT_SLOTS] = [const { AtomicIsize::new(0) }; EXIT_SLOTS];
static EXIT_NEXT: AtomicUsize = AtomicUsize::new(0);

fn record_exit(tid: Tid, code: isize) {
    let i = EXIT_NEXT.fetch_add(1, Relaxed) % EXIT_SLOTS;
    EXIT_CODE[i].store(code, Relaxed);
    EXIT_TID[i].store(tid, Relaxed);
}

/// Exit code of a finished user task (most recent record), if known.
pub fn exit_code(tid: Tid) -> Option<isize> {
    let n = EXIT_NEXT.load(Relaxed);
    (0..n.min(EXIT_SLOTS))
        .map(|k| (n - 1 - k) % EXIT_SLOTS) // newest record first
        .find(|&i| EXIT_TID[i].load(Relaxed) == tid)
        .map(|i| EXIT_CODE[i].load(Relaxed))
}

// ---------------------------------------------------------------- user stacks

const STACK_SLOTS: usize = 8;
const USER_STACK: usize = 8 * 1024;

#[repr(C, align(16))]
struct Stacks(UnsafeCell<[[u8; USER_STACK]; STACK_SLOTS]>);
// SAFETY: each slot is used by at most one task, chosen through OWNER.
unsafe impl Sync for Stacks {}
static STACKS: Stacks = Stacks(UnsafeCell::new([[0; USER_STACK]; STACK_SLOTS]));
/// Tid owning each user stack slot (0 = free).
static OWNER: [AtomicUsize; STACK_SLOTS] = [const { AtomicUsize::new(0) }; STACK_SLOTS];

/// Reserve a user stack for `tid`; returns its initial (top) stack pointer.
fn claim_stack(tid: Tid) -> Option<usize> {
    let i = (0..STACK_SLOTS).find(|&i| OWNER[i].compare_exchange(0, tid, Relaxed, Relaxed).is_ok())?;
    let base = STACKS.0.get() as usize + i * USER_STACK;
    Some(base + USER_STACK)
}

fn release_stack(tid: Tid) {
    for o in &OWNER {
        let _ = o.compare_exchange(tid, 0, Relaxed, Relaxed);
    }
}

// ---------------------------------------------------------------- starting programs

/// A program that runs in U-mode. It receives one argument and must end with
/// the `exit` system call (it cannot return: there is nothing to return to).
pub type UserProgram = extern "C" fn(usize) -> !;

extern "C" {
    fn __enter_user(entry: usize, user_sp: usize, arg: usize) -> !;
}

#[derive(Debug, PartialEq)]
pub enum StartError {
    Spawn(SpawnError),
}

/// Start `prog` as a new task that drops to U-mode immediately.
pub fn spawn(name: &'static str, prog: UserProgram, arg: usize) -> Result<Tid, StartError> {
    scheduler::spawn(name, SpawnOpts::default(), move || {
        let tid = scheduler::current_tid().unwrap_or(0);
        let Some(sp) = claim_stack(tid) else {
            kwarn!("user: no free user stack for pid {} ({} in use)", tid, STACK_SLOTS);
            record_exit(tid, EINVAL);
            return;
        };
        ktrace!("USER_ENTER", "tid={} name={} entry={:#x} usp={:#x}", tid, name, prog as usize, sp);
        // SAFETY: `prog` is a U-mode-safe function (it only executes ordinary
        // instructions and ecall) and `sp` is the top of a user stack owned by
        // this task until it exits; __enter_user never returns.
        unsafe { __enter_user(prog as usize, sp, arg) }
    })
    .map_err(StartError::Spawn)
}

// ---------------------------------------------------------------- the trap side

/// Handle `ecall` from U-mode. Called by `trap_handler` with interrupts off;
/// `tf.sepc` already points past the ecall.
pub fn syscall(tf: &mut TrapFrame) {
    let (nr, a0, a1) = (tf.regs[17], tf.regs[10], tf.regs[11]);
    let tid = scheduler::current_tid().unwrap_or(0);
    SYSCALLS.fetch_add(1, Relaxed);
    if scheduler::cancel_requested() {
        exit_current(tid, INTERRUPTED);
    }
    let ret: isize = match nr {
        SYS_WRITE => sys_write(a0, a1),
        SYS_GETPID => tid as isize,
        SYS_YIELD => {
            scheduler::yield_now();
            0
        }
        SYS_SLEEP_MS => {
            let ticks = ((a0 as u64).saturating_mul(1000)).div_ceil(timer::tick_us()).max(1);
            scheduler::sleep_ticks(ticks);
            0
        }
        SYS_UPTIME_MS => (timer::now_us() / 1000) as isize,
        SYS_EXIT => exit_current(tid, a0 as isize),
        _ => ENOSYS,
    };
    ktrace!("SYSCALL", "tid={} nr={} name={} ret={}", tid, nr, name_of(nr), ret);
    tf.regs[10] = ret as usize;
}

fn sys_write(ptr: usize, len: usize) -> isize {
    if len > MAX_WRITE {
        return EINVAL;
    }
    let Some(end) = ptr.checked_add(len) else { return EFAULT };
    if ptr < RAM.start || end > RAM.end {
        return EFAULT;
    }
    // SAFETY: [ptr, ptr+len) lies inside RAM (checked above) and paging is
    // off, so the range is readable; the bytes are only copied to the console.
    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    let Ok(text) = core::str::from_utf8(bytes) else { return EINVAL };
    print!("{}", text);
    BYTES_WRITTEN.fetch_add(len, Relaxed);
    len as isize
}

fn exit_current(tid: Tid, code: isize) -> ! {
    record_exit(tid, code);
    release_stack(tid);
    ktrace!("USER_EXIT", "tid={} code={}", tid, code);
    scheduler::exit_current()
}

/// A U-mode program caused an exception: report it and end only that task.
pub fn kill_current(what: &str, sepc: usize, stval: usize) -> ! {
    let tid = scheduler::current_tid().unwrap_or(0);
    KILLS.fetch_add(1, Relaxed);
    println!("[pid {} killed by the kernel: {} at {:#x} (stval {:#x})]", tid, what, sepc, stval);
    ktrace!("USER_KILL", "tid={} cause={} sepc={:#x}", tid, what, sepc);
    exit_current(tid, KILLED)
}

// ---------------------------------------------------------------- user programs
//
// Everything below runs in U-mode. It may only use ordinary instructions and
// `ecall`: no println!, no allocation, no locks (they touch CSRs).

#[inline(always)]
fn ecall(nr: usize, a0: usize, a1: usize) -> isize {
    let ret: isize;
    // SAFETY: ecall transfers control to the kernel's syscall handler, which
    // returns a value in a0 and preserves every other register.
    unsafe { asm!("ecall", inlateout("a0") a0 as isize => ret, in("a1") a1, in("a7") nr, options(nostack)) };
    ret
}

fn uwrite(s: &str) -> isize {
    ecall(SYS_WRITE, s.as_ptr() as usize, s.len())
}

fn uexit(code: isize) -> ! {
    ecall(SYS_EXIT, code as usize, 0);
    loop {} // never reached: exit does not return
}

/// Write `label` followed by a decimal number and a newline, without
/// formatting machinery (which would call kernel code).
fn uwrite_num(label: &str, v: u64) {
    let mut buf = [0u8; 24];
    let mut i = buf.len();
    let mut n = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 || i == 1 {
            break;
        }
    }
    uwrite(label);
    let digits = &buf[i..];
    ecall(SYS_WRITE, digits.as_ptr() as usize, digits.len());
    uwrite("\n");
}

/// `user hello [n]`: a well-behaved program using every system call.
pub extern "C" fn prog_hello(rounds: usize) -> ! {
    uwrite("hello from U-mode: this text reached the console through ecall -> write\n");
    uwrite_num("  my pid (getpid):        ", ecall(SYS_GETPID, 0, 0) as u64);
    uwrite_num("  uptime ms (uptime_ms):  ", ecall(SYS_UPTIME_MS, 0, 0) as u64);
    for i in 1..=rounds {
        ecall(SYS_SLEEP_MS, 100, 0);
        uwrite_num("  slept 100 ms, round ", i as u64);
    }
    ecall(SYS_YIELD, 0, 0);
    uexit(0)
}

/// `user priv`: tries a privileged instruction; the kernel kills it.
pub extern "C" fn prog_privileged(_: usize) -> ! {
    uwrite("user program: trying to read sstatus (a supervisor register)...\n");
    let v: usize;
    // SAFETY (for the kernel): executing this in U-mode raises an
    // illegal-instruction exception, which is the point of the demo.
    unsafe { asm!("csrr {}, sstatus", out(reg) v, options(nomem, nostack)) };
    uwrite_num("  should never print; read ", v as u64);
    uexit(1)
}

/// `user badptr`: passes a pointer outside RAM; write must refuse it.
pub extern "C" fn prog_bad_pointer(_: usize) -> ! {
    let r = ecall(SYS_WRITE, 0x10, 5);
    uwrite_num("user program: write(0x10, 5) was refused; minus the error code: ", (-r) as u64);
    uexit(r)
}

/// `user unknown`: calls a system call number the kernel does not have.
pub extern "C" fn prog_unknown(_: usize) -> ! {
    let r = ecall(4242, 0, 0);
    uwrite_num("user program: syscall 4242 returned minus ", (-r) as u64);
    uexit(r)
}

/// `user spin [millions]`: CPU-bound loop in U-mode, to show that the timer
/// interrupt preempts user code too.
pub extern "C" fn prog_spin(millions: usize) -> ! {
    let mut x: u64 = 0;
    let mut i = 0usize;
    while i < millions * 1_000_000 {
        x = core::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        i += 1;
    }
    uwrite_num("user spin finished, checksum low digits ", x % 1000);
    uexit(0)
}

pub fn program(name: &str) -> Option<UserProgram> {
    Some(match name {
        "hello" => prog_hello,
        "priv" => prog_privileged,
        "badptr" => prog_bad_pointer,
        "unknown" => prog_unknown,
        "spin" => prog_spin,
        _ => return None,
    })
}
