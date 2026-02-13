use core::fmt;
use crate::uart;

const BACKSPACE: i32 = 0x100;

fn consputc(c: i32) {
    if c == BACKSPACE {
        uart::uartputc('\x08' as i32);
        uart::uartputc(' ' as i32);
        uart::uartputc('\x08' as i32);
    } else {
        uart::uartputc(c);
    }
}

pub struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                consputc('\r' as i32);
            }
            consputc(b as i32);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    let mut c = Console;
    let _ = c.write_fmt(args);
}
