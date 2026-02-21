use core::fmt::{self, Write};
use spin::Mutex;

pub struct Uart(usize);

unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub const fn new(addr: usize) -> Self { 
        Self(addr) 
    }

    pub fn putc(&self, c: u8) {
        let ptr = self.0 as *mut u8;
        unsafe { ptr.write_volatile(c); }
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

pub static PANIC_UART: Mutex<Uart> = Mutex::new(Uart::new(0x1000_0000));

pub fn _print(args: fmt::Arguments) {
    PANIC_UART.lock().write_fmt(args).unwrap();
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