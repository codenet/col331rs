use core::str;

use crate::constants::{T_DIR, T_FILE};
use crate::fcntl::{O_CREATE, O_RDONLY};
use crate::file;
use crate::fs;
use crate::log;
use crate::param::NOFILE;
use crate::proc::myproc;
use crate::syscall::{argint, argptr, argstr};

fn argfd(n: i32, pfd: Option<&mut i32>, pf: Option<&mut usize>) -> i32 {
    let mut fd = 0;
    if argint(n, &mut fd) < 0 {
        return -1;
    }
    if fd < 0 || fd as usize >= NOFILE {
        return -1;
    }

    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    let fidx = match curproc.ofile[fd as usize] {
        Some(v) => v,
        None => return -1,
    };

    if let Some(out) = pfd {
        *out = fd;
    }
    if let Some(out) = pf {
        *out = fidx;
    }

    0
}

fn fdalloc(f_idx: usize) -> i32 {
    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    for fd in 0..NOFILE {
        if curproc.ofile[fd].is_none() {
            curproc.ofile[fd] = Some(f_idx);
            return fd as i32;
        }
    }

    -1
}

pub fn sys_write() -> i32 {
    let mut f_idx = 0usize;
    let mut p: *const u8 = core::ptr::null();

    let n = if argfd(0, None, Some(&mut f_idx)) < 0 {
        -1
    } else {
        argstr(1, &mut p)
    };

    if n < 0 {
        return -1;
    }

    let s = unsafe { core::slice::from_raw_parts(p, n as usize) };
    file::filewrite(f_idx, s, n)
}

pub fn sys_read() -> i32 {
    let mut f_idx = 0usize;
    let mut n = 0;
    let mut p: *const u8 = core::ptr::null();

    if argfd(0, None, Some(&mut f_idx)) < 0 || argint(2, &mut n) < 0 || argptr(1, &mut p, n) < 0 {
        return -1;
    }

    let dst = unsafe { core::slice::from_raw_parts_mut(p as *mut u8, n as usize) };
    file::fileread(f_idx, dst, n)
}

pub fn sys_close() -> i32 {
    let mut fd = 0;
    let mut f_idx = 0usize;

    if argfd(0, Some(&mut fd), Some(&mut f_idx)) < 0 {
        return -1;
    }

    if let Some(curproc) = myproc() {
        curproc.ofile[fd as usize] = None;
    }
    file::fileclose(f_idx);
    0
}

pub fn sys_open() -> i32 {
    let mut path_ptr: *const u8 = core::ptr::null();
    let mut omode = 0;

    let path_len = argstr(0, &mut path_ptr);
    if path_len < 0 || argint(1, &mut omode) < 0 {
        return -1;
    }

    let path_bytes = unsafe { core::slice::from_raw_parts(path_ptr, path_len as usize) };
    let path = match core::str::from_utf8(path_bytes) {
        Ok(v) => v,
        Err(_) => return -1,
    };

    log::begin_op();

    let ip = if (omode & O_CREATE) != 0 {
        match create(path, T_FILE as i16, 0, 0) {
            Some(v) => v,
            None => {
                log::end_op();
                return -1;
            }
        }
    } else {
        let ip = match fs::namei(path) {
            Some(v) => v,
            None => {
                log::end_op();
                return -1;
            }
        };
        fs::iread(ip);
        if fs::inode(ip).type_ as u16 == T_DIR && omode != O_RDONLY {
            fs::iput(ip);
            log::end_op();
            return -1;
        }
        ip
    };

    let f_idx = match file::filealloc() {
        Some(v) => v,
        None => {
            fs::iput(ip);
            log::end_op();
            return -1;
        }
    };

    let fd = fdalloc(f_idx);
    if fd < 0 {
        file::fileclose(f_idx);
        fs::iput(ip);
        log::end_op();
        return -1;
    }

    log::end_op();

    file::file_set_inode(f_idx, ip, omode);
    fd
}

fn create(path: &str, type_: i16, major: i16, minor: i16) -> Option<usize> {
    let (dp, name) = fs::nameiparent(path)?;
    fs::iread(dp);

    if let Some(ip) = fs::dirlookup(dp, name, None) {
        fs::iput(dp);
        fs::iread(ip);
        if (type_ as u16) == T_FILE && fs::inode(ip).type_ as u16 == T_FILE {
            return Some(ip);
        }
        fs::iput(ip);
        return None;
    }

    let ip = fs::ialloc(fs::inode(dp).dev, type_);

    fs::iread(ip);
    {
        let inode = fs::inode_mut(ip);
        inode.major = major;
        inode.minor = minor;
        inode.nlink = 1;
    }
    fs::iupdate(ip);

    if (type_ as u16) == T_DIR {
        fs::inode_mut(dp).nlink += 1;
        fs::iupdate(dp);
        if fs::dirlink(ip, ".", fs::inode(ip).inum) < 0 || fs::dirlink(ip, "..", fs::inode(dp).inum) < 0 {
            panic!("create dots");
        }
    }

    if fs::dirlink(dp, name, fs::inode(ip).inum) < 0 {
        panic!("create: dirlink");
    }

    fs::iput(dp);
    Some(ip)
}
