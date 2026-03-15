use crate::{uart::*};
use core::fmt::*;
use crate::file::DEVSW;
use crate::param::CONSOLE;
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
    let mut doprocdump = false;
    loop {
        let c = getc();
        if c < 0 {
            break;
        }

        unsafe {
            let input = &raw mut INPUT;
            match c {
                x if x == C('P') => {
                    // procdump() may indirectly use console output; call after loop
                    doprocdump = true;
                }
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
                            // call myproc with the buf
                            (*input).w = (*input).e;
                        } 
                    }
                }
            }
        }
    }
    if doprocdump {
        crate::proc::procdump();
    }
}

pub fn consoleread(_ip: usize, dst: &mut [u8], n: i32) -> i32 {
    let target = n;
    let mut n = n;

    unsafe {
        let input = &raw mut INPUT;
        while n > 0 {
            // Busy wait for input - mirrors C: while(input.r == input.w);
            while core::ptr::read_volatile(&(*input).r) == core::ptr::read_volatile(&(*input).w) {
                // Spin-wait (busy wait) for input to arrive
                core::hint::spin_loop();
            }

            // Read character and increment read pointer - mirrors C: input.buf[input.r++ % INPUT_BUF]
            let c = (*input).buf[(*input).r % INPUT_BUF] as i32;
            (*input).r += 1;
            
            // Handle EOF (Ctrl-D)
            if c == CTRL_D {
                if n < target {
                    // Save ^D for next time, to make sure
                    // caller gets a 0-byte result.
                    (*input).r -= 1;
                }
                break;
            }
            
            // Copy character to destination - mirrors C: *dst++ = c;
            dst[(target - n) as usize] = c as u8;
            n -= 1;
            
            // Break on newline
            if c == '\n' as i32 {
                break;
            }
        }
    }

    target - n
}

pub fn consolewrite(_ip: usize, src: &[u8], n: i32) -> i32 {
    // Mirrors C: for(i = 0; i < n; i++) consputc(buf[i] & 0xff);
    for i in 0..n {
        consputc(src[i as usize] as i32);
    }
    n
}

pub fn consoleinit() {
    // Register console device handlers in the device switch table
    // Mirrors C: devsw[CONSOLE].write = consolewrite; devsw[CONSOLE].read = consoleread;
    unsafe {
        DEVSW[CONSOLE].read  = Some(consoleread);
        DEVSW[CONSOLE].write = Some(consolewrite);
    }
}
