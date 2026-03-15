use crate::user::write;

unsafe fn putc(fd: i32, c: u8) {
    write(fd, &c as *const u8, 1);
}

pub enum PrintfArg<'a> {
    Int(i32),
    Uint(u32),
    Str(&'a str),
    Char(u8),
}

fn printint(fd: i32, mut x: i32, base: i32, sgn: bool) {
    let digits = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut neg = false;

    if sgn && x < 0 {
        neg = true;
        x = -x;
    }

    let mut ux = x as u32;

    loop {
        buf[i] = digits[(ux % base as u32) as usize];
        i += 1;
        ux /= base as u32;
        if ux == 0 { break; }
    }

    if neg {
        buf[i] = b'-';
        i += 1;
    }

    while i > 0 {
        i -= 1;
        unsafe {
            putc(fd, buf[i]);
        }
    }
}

pub fn printf(fd: i32, fmt: &[u8], args: &[PrintfArg]) {
    let mut state = false;
    let mut arg_i = 0;

    for &c in fmt {
        if c == 0 {
            break;
        }

        if !state {
            if c == b'%' {
                state = true;
            } else {
                unsafe {
                    putc(fd, c);
                }
            }
            continue;
        }

        match c {
            b'd' => {
                if let PrintfArg::Int(v) = args[arg_i] {
                    printint(fd, v, 10, true);
                }
                arg_i += 1;
            }

            b'x' | b'p' => {
                if let PrintfArg::Uint(v) = args[arg_i] {
                    printint(fd, v as i32, 16, false);
                }
                arg_i += 1;
            }

            b's' => {
                if let PrintfArg::Str(s) = args[arg_i] {
                    for &b in s.as_bytes() {
                        if b == 0 { break; }
                        unsafe {
                            putc(fd, b);
                        }
                    }
                }
                arg_i += 1;
            }

            b'c' => {
                if let PrintfArg::Char(ch) = args[arg_i] {
                    unsafe {
                        putc(fd, ch);
                    }
                }
                arg_i += 1;
            }

            b'%' =>     unsafe {
                        putc(fd, b'%');
                    }

            _ => {
                unsafe{
                    putc(fd, b'%');
                    putc(fd, c);
                }
            }
        }

        state = false;
    }
}

