use core::fmt::{self, Write};
use spin::Mutex;
use crate::dtb;

pub struct Uart;

impl Uart {
    /// Returns the current hardware address for the UART.
    /// It queries the DTB config, falling back to the QEMU default if not yet init.
    fn addr(&self) -> usize {
        dtb::get_uart_addr()
    }

    pub fn putc(&self, c: u8) {
        let ptr = self.addr() as *mut u8;
        unsafe {
            // Standard 8250/16550a UART: Write to the Transmitter Holding Register (THR)
            ptr.write_volatile(c);
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.putc(byte); 
        }
        Ok(())
    }
}

/// We no longer need to store the address inside the Uart struct.
/// The address is fetched dynamically from the DTB module.
pub static PANIC_UART: Mutex<Uart> = Mutex::new(Uart);

pub fn _print(args: fmt::Arguments) {
    // Note: In a multicore system, this Mutex prevents Harts from 
    // scrambling each other's characters in the serial output.
    if let Some(mut guard) = PANIC_UART.try_lock() {
        guard.write_fmt(args).unwrap();
    }
}

// --- Basic Print Macros ---

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}

// --- Logging Macros with ANSI Colors ---

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::print!("\x1b[32m[INFO]\x1b[0m ");
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::print!("\x1b[33m[WARN]\x1b[0m ");
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::print!("\x1b[31m[ERROR]\x1b[0m ");
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            $crate::print!("\x1b[34m[DEBUG]\x1b[0m ");
            $crate::println!($($arg)*);
        }
    };
}