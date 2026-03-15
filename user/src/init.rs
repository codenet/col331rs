use crate::printf::{printf, PrintfArg};
use crate::user::{open, close, O_RDWR};

#[no_mangle]
pub extern "C" fn main(_argc: i32, argv: *const *const u8) -> i32 {
    unsafe {
        let fd = open(b"console\0".as_ptr(), O_RDWR);

        let arg0 = if argv.is_null() {
            "(null)"
        } else {
            let mut len = 0;
            let p = *argv;

            while *p.add(len) != 0 {
                len += 1;
            }

            core::str::from_utf8_unchecked(
                core::slice::from_raw_parts(p, len)
            )
        };

        printf(
            fd,
            b"Hello %s from init.rs\n",
            &[PrintfArg::Str(arg0)],
        );

        close(fd);

        loop {}
    }
}