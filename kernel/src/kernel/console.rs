//! Kernel console with two output channels on the same serial line:
//!
//! * human-readable log lines: `[    1234us] INFO  message`
//! * machine-readable trace events: `#T <time_us> <tick> <EVENT> key=value ...`
//!
//! `analysis/analyze.py` parses the `#T` lines; humans read the rest.

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::drivers::uart::{Uart, UART0_BASE};
use crate::sync::IrqSpinLock;
use crate::timer;

// SAFETY: UART0_BASE is the QEMU virt UART, initialised by OpenSBI.
static CONSOLE: IrqSpinLock<Uart> = IrqSpinLock::new("console", unsafe { Uart::new(UART0_BASE) });

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN ",
            Self::Info => "INFO ",
            Self::Debug => "DEBUG",
        }
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);
static TRACE: AtomicBool = AtomicBool::new(true);

pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

/// Enable or disable trace events; returns the previous setting.
pub fn set_trace(on: bool) -> bool {
    TRACE.swap(on, Ordering::Relaxed)
}

pub fn print(args: fmt::Arguments) {
    let _ = CONSOLE.lock().write_fmt(args);
}

/// A keyboard byte from the serial line, if one has arrived.
pub fn read_byte() -> Option<u8> {
    CONSOLE.lock().try_read_byte()
}

pub fn trace_enabled() -> bool {
    TRACE.load(Ordering::Relaxed)
}

pub fn level() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Level::Error,
        1 => Level::Warn,
        2 => Level::Info,
        _ => Level::Debug,
    }
}

pub fn log(level: Level, args: fmt::Arguments) {
    if level as u8 > LEVEL.load(Ordering::Relaxed) {
        return;
    }
    let us = timer::now_us();
    let mut c = CONSOLE.lock();
    let _ = write!(c, "[{:>10}us] {} ", us, level.tag());
    let _ = c.write_fmt(args);
    let _ = c.write_str("\n");
}

pub fn trace(event: &str, args: fmt::Arguments) {
    if TRACE.load(Ordering::Relaxed) {
        trace_always(event, args);
    }
}

/// Emit a trace event even while tracing is switched off. Used by the
/// shell's live monitor, whose snapshots feed the web front end.
pub fn trace_always(event: &str, args: fmt::Arguments) {
    let (us, tick) = (timer::now_us(), timer::ticks());
    let mut c = CONSOLE.lock();
    let _ = write!(c, "#T {} {} {} ", us, tick, event);
    let _ = c.write_fmt(args);
    let _ = c.write_str("\n");
}

/// A lock-free writer for the panic handler: the console lock may be held
/// by the very code that panicked.
pub fn emergency_writer() -> Uart {
    // SAFETY: same device as CONSOLE; interleaving is acceptable while panicking.
    unsafe { Uart::new(UART0_BASE) }
}

macro_rules! print {
    ($($arg:tt)*) => { $crate::kernel::console::print(format_args!($($arg)*)) };
}

macro_rules! println {
    () => { print!("\n") };
    ($($arg:tt)*) => { $crate::kernel::console::print(format_args!("{}\n", format_args!($($arg)*))) };
}

macro_rules! kerror {
    ($($arg:tt)*) => { $crate::kernel::console::log($crate::kernel::console::Level::Error, format_args!($($arg)*)) };
}

macro_rules! kwarn {
    ($($arg:tt)*) => { $crate::kernel::console::log($crate::kernel::console::Level::Warn, format_args!($($arg)*)) };
}

macro_rules! kinfo {
    ($($arg:tt)*) => { $crate::kernel::console::log($crate::kernel::console::Level::Info, format_args!($($arg)*)) };
}

macro_rules! kdebug {
    ($($arg:tt)*) => { $crate::kernel::console::log($crate::kernel::console::Level::Debug, format_args!($($arg)*)) };
}

/// Emit a structured trace event: `ktrace!("DISPATCH", "tid={} q={}", tid, q)`.
macro_rules! ktrace {
    ($ev:expr) => { $crate::kernel::console::trace($ev, format_args!("")) };
    ($ev:expr, $($arg:tt)*) => { $crate::kernel::console::trace($ev, format_args!($($arg)*)) };
}
