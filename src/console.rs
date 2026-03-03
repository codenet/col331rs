use crate::uart::*;
use core::fmt::*;
pub struct Console {}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.chars() {
            consputc(c as i32);
        }
        Ok(())
    }
}

const BACKSPACE: i32 = 0x100;
const INPUT_BUF: usize = 128;
const CTRL_D: i32 = C('D');

#[allow(non_snake_case)]
const fn C(c: char) -> i32 {
    (c as i32) - ('@' as i32)
}

#[derive(Clone, Copy)]
struct Input {
    buf: [u8; INPUT_BUF],
    r: usize,
    w: usize,
    e: usize,
}

static mut INPUT: Input = Input {
    buf: [0; INPUT_BUF],
    r: 0,
    w: 0,
    e: 0,
};

pub fn consputc(c: i32) {
    if c == BACKSPACE {
        uartputc('\x08' as i32);
        uartputc(' ' as i32);
        uartputc('\x08' as i32);
    } else {
        uartputc(c);
    }
}

pub fn consoleintr(getc: fn() -> i32) {
    loop {
        let c = getc();
        if c < 0 {
            break;
        }

        unsafe {
            let input = &raw mut INPUT;
            match c {
                x if x == C('U') => {
                    while (*input).e != (*input).w && (*input).buf[((*input).e - 1) % INPUT_BUF] != b'\n' {
                        (*input).e -= 1;
                        consputc(BACKSPACE);
                    }
                }
                x if x == C('H') || x == 0x7f => {
                    if (*input).e != (*input).w {
                        (*input).e -= 1;
                        consputc(BACKSPACE);
                    }
                }
                _ => {
                    if c != 0 && (*input).e.wrapping_sub((*input).r) < INPUT_BUF {
                        let c = if c == '\r' as i32 { '\n' as i32 } else { c };
                        (*input).buf[(*input).e % INPUT_BUF] = c as u8;
                        (*input).e += 1;
                        consputc(c);
                        if c == '\n' as i32 || c == CTRL_D || (*input).e == (*input).r + INPUT_BUF {
                            (*input).w = (*input).e;
                        }
                    }
                }
            }
        }
    }
}