use core::str;

use crate::buf::BSIZE;
use crate::constants::{T_DIR, T_FILE, T_DEV, DIRSIZ, DIRENT_SIZE};
use crate::fcntl::{O_CREATE, O_RDONLY, O_RDWR, O_WRONLY};
use crate::fs;
use crate::log;
use crate::param::{MAXOPBLOCKS, NFILE, NDEV};
use crate::spinlock::{Spinlock, acquire, initlock, release};

// Device switch table entry
#[derive(Copy, Clone)]
pub struct Devsw {
    pub read: Option<fn(usize, &mut [u8], i32) -> i32>,
    pub write: Option<fn(usize, &[u8], i32) -> i32>,
}

impl Devsw {
    pub const fn new() -> Self {
        Self {
            read: None,
            write: None,
        }
    }
}

pub static mut DEVSW: [Devsw; NDEV] = [const { Devsw::new() }; NDEV];

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FileType {
    None,
    Inode,
    Device,
}

#[derive(Copy, Clone)]
pub struct File {
    pub type_: FileType,
    pub refcnt: i32,
    pub readable: bool,
    pub writable: bool,
    pub ip: usize,
    pub off: u32,
}

impl File {
    pub const fn new() -> Self {
        Self {
            type_: FileType::None,
            refcnt: 0,
            readable: false,
            writable: false,
            ip: 0,
            off: 0,
        }
    }
}

struct FTable {
    file: [File; NFILE],
    lock: Spinlock,
}

impl FTable {
    const fn new() -> Self {
        Self {
            file: [const { File::new() }; NFILE],
            lock: Spinlock::new(),
        }
    }
}

static mut FTABLE: FTable = FTable::new();

fn dirsiz_to_str(name: &[u8; DIRSIZ]) -> &str {
    let len = name.iter().position(|&b| b == 0).unwrap_or(DIRSIZ);
    str::from_utf8(&name[..len]).unwrap_or("")
}

pub fn fileinit() {
    unsafe {
        initlock(&raw mut FTABLE.lock, "ftable\0".as_ptr());
        for i in 0..NFILE {
            FTABLE.file[i] = File::new();
        }
    }
}

pub fn filealloc() -> Option<usize> {
    unsafe {
        acquire(&raw mut FTABLE.lock);
        for i in 0..NFILE {
            if FTABLE.file[i].refcnt == 0 {
                FTABLE.file[i].refcnt = 1;
                release(&raw mut FTABLE.lock);
                return Some(i);
            }
        }
        release(&raw mut FTABLE.lock);
    }
    None
}

pub fn file_set_inode(f_idx: usize, ip: usize, omode: i32) {
    unsafe {
        if f_idx >= NFILE {
            panic!("file_set_inode: bad file index");
        }
        let f = &mut FTABLE.file[f_idx];
        f.type_ = FileType::Inode;
        f.ip = ip;
        f.off = 0;
        f.readable = (omode & O_WRONLY) == 0;
        f.writable = (omode & O_WRONLY) != 0 || (omode & O_RDWR) != 0;
    }
}

pub fn filedup(f_idx: usize) -> usize {
    unsafe {
        acquire(&raw mut FTABLE.lock);
        if f_idx >= NFILE {
            panic!("filedup: bad file index");
        }
        if FTABLE.file[f_idx].refcnt < 1 {
            panic!("filedup");
        }
        FTABLE.file[f_idx].refcnt += 1;
        release(&raw mut FTABLE.lock);
    }
    f_idx
}

pub fn fileclose(f_idx: usize) {
    let ff: File;
    unsafe {
        acquire(&raw mut FTABLE.lock);
        if f_idx >= NFILE {
            panic!("fileclose: bad file index");
        }

        let f = &mut FTABLE.file[f_idx];
        if f.refcnt < 1 {
            panic!("fileclose");
        }
        f.refcnt -= 1;
        if f.refcnt > 0 {
            release(&raw mut FTABLE.lock);
            return;
        }

        ff = *f;
        *f = File::new();
        release(&raw mut FTABLE.lock);
    }

    if ff.type_ == FileType::Inode {
        log::begin_op();
        fs::iput(ff.ip);
        log::end_op();
    }
}

pub fn filestat(f_idx: usize, st: &mut fs::Stat) -> i32 {
    let f: File;
    unsafe {
        if f_idx >= NFILE {
            panic!("filestat: bad file index");
        }
        f = FTABLE.file[f_idx];
    }

    if f.type_ == FileType::Inode {
        fs::iread(f.ip);
        fs::stati(f.ip, st);
        return 0;
    }

    -1
}

pub fn fileread(f_idx: usize, dst: &mut [u8], n: i32) -> i32 {
    if n < 0 || (n as usize) > dst.len() {
        return -1;
    }

    let f: File;
    unsafe {
        if f_idx >= NFILE {
            panic!("fileread: bad file index");
        }
        f = FTABLE.file[f_idx];
    }

    if !f.readable {
        return -1;
    }

    if f.type_ == FileType::Inode {
        fs::iread(f.ip);
        let r = fs::readi(f.ip, dst, f.off, n as u32);
        if r > 0 {
            unsafe {
                FTABLE.file[f_idx].off += r as u32;
            }
        }
        return r;
    }

    panic!("fileread");
}

pub fn filewrite(f_idx: usize, src: &[u8], n: i32) -> i32 {
    if n < 0 || (n as usize) > src.len() {
        return -1;
    }

    let f: File;
    unsafe {
        if f_idx >= NFILE {
            panic!("filewrite: bad file index");
        }
        f = FTABLE.file[f_idx];
    }

    if !f.writable {
        return -1;
    }

    if f.type_ == FileType::Inode {
        let max = (((MAXOPBLOCKS - 1 - 1 - 2) / 2) * BSIZE) as i32;
        let mut i = 0i32;

        while i < n {
            let mut n1 = n - i;
            if n1 > max {
                n1 = max;
            }

            log::begin_op();
            fs::iread(f.ip);
            let off = unsafe { FTABLE.file[f_idx].off };
            let r = fs::writei(f.ip, &src[i as usize..], off, n1 as u32);
            if r > 0 {
                unsafe {
                    FTABLE.file[f_idx].off += r as u32;
                }
            }
            log::end_op();

            if r < 0 {
                break;
            }
            if r != n1 {
                panic!("short filewrite");
            }
            i += r;
        }

        if i == n {
            n
        } else {
            -1
        }
    } else {
        panic!("filewrite");
    }
}

pub fn isdirempty(dp_idx: usize) -> bool {
    fs::iread(dp_idx);

    let mut off = (2 * DIRENT_SIZE) as u32;
    while off < fs::inode_size(dp_idx) {
        let mut raw = [0u8; DIRENT_SIZE];
        if fs::readi(dp_idx, &mut raw, off, DIRENT_SIZE as u32) != DIRENT_SIZE as i32 {
            panic!("isdirempty: readi");
        }
        let de = fs::parse_dirent(&raw);
        if de.inum != 0 {
            return false;
        }
        off += DIRENT_SIZE as u32;
    }

    true
}

pub fn unlink(path: &str, name: &mut [u8; DIRSIZ]) -> i32 {
    log::begin_op();
    
    let dp = match fs::nameiparent(path, name) {
        Some(idx) => idx,
        None => {
            log::end_op();
            return -1;
        }
    };

    fs::iread(dp);

    let name_str = dirsiz_to_str(name);

    if name_str == "." || name_str == ".." {
        fs::iput(dp);
        log::end_op();
        return -1;
    }

    let mut off = 0u32;
    let ip = match fs::dirlookup(dp, name_str, Some(&mut off)) {
        Some(idx) => idx,
        None => {
            fs::iput(dp);
            log::end_op();
            return -1;
        }
    };

    fs::iread(ip);

    if fs::inode_nlink(ip) < 1 {
        panic!("unlink: nlink < 1");
    }

    if fs::inode_type(ip) == T_DIR && !isdirempty(ip) {
        fs::iput(ip);
        fs::iput(dp);
        log::end_op();
        return -1;
    }

    let de = [0u8; DIRENT_SIZE];
    if fs::writei(dp, &de, off, DIRENT_SIZE as u32) != DIRENT_SIZE as i32 {
        panic!("unlink: writei");
    }

    if fs::inode_type(ip) == T_DIR {
        fs::inode_dec_nlink(dp);
        fs::iupdate(dp);
    }
    fs::iput(dp);

    fs::inode_dec_nlink(ip);
    fs::iupdate(ip);
    fs::iput(ip);

    log::end_op();
    0
}

fn create(path: &str, type_: i16, major: i16, minor: i16) -> Option<usize> {
    let mut name = [0u8; DIRSIZ];

    let dp = fs::nameiparent(path, &mut name)?;
    fs::iread(dp);

    let name_str = dirsiz_to_str(&name);

    if let Some(ip) = fs::dirlookup(dp, name_str, None) {
        fs::iput(dp);
        fs::iread(ip);
        if (type_ as u16) == T_FILE && fs::inode_type(ip) == T_FILE {
            return Some(ip);
        }
        fs::iput(ip);
        return None;
    }

    let ip = fs::ialloc(fs::inode_dev(dp), type_);

    fs::iread(ip);
    fs::inode_set_meta(ip, major, minor, 1);
    fs::iupdate(ip);

    if (type_ as u16) == T_DIR {
        fs::inode_inc_nlink(dp);
        fs::iupdate(dp);

        if fs::dirlink(ip, ".", fs::inode_inum(ip)) < 0 || fs::dirlink(ip, "..", fs::inode_inum(dp)) < 0 {
            panic!("create dots");
        }
    }

    if fs::dirlink(dp, name_str, fs::inode_inum(ip)) < 0 {
        panic!("create: dirlink");
    }

    fs::iput(dp);

    Some(ip)
}

pub fn open(path: &str, omode: i32) -> Option<usize> {
    log::begin_op();
    
    let ip = if (omode & O_CREATE) != 0 {
        let ip = match create(path, T_FILE as i16, 0, 0) {
            Some(ip) => ip,
            None => {
                log::end_op();
                return None;
            }
        };
        ip
    } else {
        let ip = match fs::namei(path) {
            Some(ip) => ip,
            None => {
                log::end_op();
                return None;
            }
        };
        fs::iread(ip);
        if fs::inode_type(ip) == T_DIR && omode != O_RDONLY {
            fs::iput(ip);
            log::end_op();
            return None;
        }
        ip
    };

    let f_idx = match filealloc() {
        Some(idx) => idx,
        None => {
            fs::iput(ip);
            log::end_op();
            return None;
        }
    };

    unsafe {
        let f = &mut FTABLE.file[f_idx];
        f.type_ = FileType::Inode;
        f.ip = ip;
        f.off = 0;
        f.readable = (omode & O_WRONLY) == 0;
        f.writable = (omode & O_WRONLY) != 0 || (omode & O_RDWR) != 0;
    }

    log::end_op();
    Some(f_idx)
}

pub fn mkdir(path: &str) -> i32 {
    log::begin_op();
    
    let ip = match create(path, T_DIR as i16, 0, 0) {
        Some(ip) => ip,
        None => {
            log::end_op();
            return -1;
        }
    };
    
    fs::iput(ip);
    log::end_op();
    0
}

pub fn mknod(path: &str, major: i16, minor: i16) -> i32 {
    log::begin_op();
    
    let ip = match create(path, T_DEV as i16, major, minor) {
        Some(ip) => ip,
        None => {
            log::end_op();
            return -1;
        }
    };
    
    fs::iput(ip);
    log::end_op();
    0
}
