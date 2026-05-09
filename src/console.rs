use crate::uart::*;
use core::fmt::*;

// Console output.
// Output is written to the screen and serial port.

pub struct Console {}
impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
         for c in s.chars() {
            consputc(c);
        }
        Ok(())
    }
}


#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => ({
        let mut console = Console {};
        let _ = writeln!(&mut console, $($arg)*);
    });
}

const BACKSPACE: char = '\x08';

fn consputc(c: char) {
  if c == BACKSPACE {
    uartputc(BACKSPACE);
    uartputc(' ');
    uartputc(BACKSPACE);
  } else {
    uartputc(c);
  }
}
