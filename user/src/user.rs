extern "C" {
    pub fn open(path: *const u8, flags: i32) -> i32;
    pub fn write(fd: i32, buf: *const u8, n: i32) -> i32;
    pub fn close(fd: i32) -> i32;
}

pub const O_RDWR: i32 = 2;