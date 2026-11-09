//! Polled NS16550A UART driver (serial console). OpenSBI has already set the
//! baud rate and line format, so the kernel only reads and writes bytes.

use core::fmt;
use core::ptr::{read_volatile, write_volatile};

pub const UART0_BASE: usize = 0x1000_0000;
const THR: usize = 0; // transmit holding register (write)
const RBR: usize = 0; // receive buffer register (read)
const LSR: usize = 5; // line status register
const LSR_DATA_READY: u8 = 1 << 0;
const LSR_THR_EMPTY: u8 = 1 << 5;

pub struct Uart {
    base: usize,
}

impl Uart {
    /// # Safety
    /// `base` must be the MMIO base of an initialised 16550-compatible UART.
    pub const unsafe fn new(base: usize) -> Self {
        Self { base }
    }

    pub fn put_byte(&mut self, byte: u8) {
        // SAFETY: `new`'s contract guarantees these are the UART's registers.
        unsafe {
            while read_volatile((self.base + LSR) as *const u8) & LSR_THR_EMPTY == 0 {
                core::hint::spin_loop();
            }
            write_volatile((self.base + THR) as *mut u8, byte);
        }
    }

    /// Return a received byte if one is waiting (non-blocking).
    pub fn try_read_byte(&mut self) -> Option<u8> {
        // SAFETY: `new`'s contract guarantees these are the UART's registers.
        unsafe {
            if read_volatile((self.base + LSR) as *const u8) & LSR_DATA_READY == 0 {
                return None;
            }
            Some(read_volatile((self.base + RBR) as *const u8))
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                self.put_byte(b'\r');
            }
            self.put_byte(b);
        }
        Ok(())
    }
}
