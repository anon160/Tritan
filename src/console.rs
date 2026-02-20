use core::fmt::{self, Write}; // Ensure Write is imported here
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
    // We must use .write_fmt() which is provided by the Write trait
    PANIC_UART.lock().write_fmt(args).unwrap();
}

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