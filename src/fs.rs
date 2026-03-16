use core::cmp::min;

use crate::bio;
use crate::buf::BSIZE;
use crate::log;
use crate::param::{NINODE, ROOTDEV, NDEV};
use crate::println;
use crate::constants::{NDIRECT, NINDIRECT, DIRSIZ, DIRENT_SIZE, DINODE_SIZE, IPB, BPB, MAXFILE};
use crate::constants::{ROOTINO, T_DEV};
use crate::file::DEVSW;
use crate::spinlock::{Spinlock, acquire, initlock, release};

pub use crate::constants::T_DIR;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Superblock {
    pub size: u32,
    pub nblocks: u32,
    pub ninodes: u32,
    pub nlog: u32,
    pub logstart: u32,
    pub inodestart: u32,
    pub bmapstart: u32,
}

impl Superblock {
    pub const fn new() -> Self {
        Self {
            size: 0,
            nblocks: 0,
            ninodes: 0,
            nlog: 0,
            logstart: 0,
            inodestart: 0,
            bmapstart: 0,
        }
    }
}

#[repr(C)]
pub struct Inode {
    pub dev: u32,
    pub inum: u32,
    pub refcnt: i32,
    pub valid: i32,

    pub type_: i16,
    pub major: i16,
    pub minor: i16,
    pub nlink: i16,
    pub size: u32,
    pub addrs: [u32; NDIRECT + 1],
}

impl Inode {
    pub const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            refcnt: 0,
            valid: 0,
            type_: 0,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT + 1],
        }
    }
}

#[repr(C)]
pub struct Stat {
    pub type_: i16,
    pub dev: i32,
    pub ino: u32,
    pub nlink: i16,
    pub size: u32,
}

impl Stat {
    pub const fn new() -> Self {
        Self {
            type_: 0,
            dev: 0,
            ino: 0,
            nlink: 0,
            size: 0,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Dirent {
    pub inum: u16,
    pub name: [u8; DIRSIZ],
}

impl Dirent {
    pub const fn new() -> Self {
        Self {
            inum: 0,
            name: [0; DIRSIZ],
        }
    }
}

struct ICache {
    lock: Spinlock,
    inode: [Inode; NINODE],
}

impl ICache {
    const fn new() -> Self {
        Self {
            inode: [const { Inode::new() }; NINODE],
            lock: Spinlock::new(),
        }
    }
}

static mut SB: Superblock = Superblock::new();
static mut ICACHE: ICache = ICache::new();

#[inline]
fn read_u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

#[inline]
fn read_i16_le(data: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([data[off], data[off + 1]])
}

#[inline]
fn read_u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[inline]
fn write_u16_le(data: &mut [u8], off: usize, val: u16) {
    data[off..off + 2].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn write_i16_le(data: &mut [u8], off: usize, val: i16) {
    data[off..off + 2].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn write_u32_le(data: &mut [u8], off: usize, val: u32) {
    data[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn iblock(inum: u32, sb: &Superblock) -> u32 {
    inum / (IPB as u32) + sb.inodestart
}

#[inline]
fn bblock(b: u32, sb: &Superblock) -> u32 {
    b / (BPB as u32) + sb.bmapstart
}

#[inline]
fn name_to_dirsiz(name: &str) -> [u8; DIRSIZ] {
    let mut out = [0u8; DIRSIZ];
    let bytes = name.as_bytes();
    let n = min(bytes.len(), DIRSIZ);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

pub fn parse_dirent(raw: &[u8]) -> Dirent {
    let mut de = Dirent::new();
    de.inum = read_u16_le(raw, 0);
    de.name.copy_from_slice(&raw[2..2 + DIRSIZ]);
    de
}

pub fn readsb(dev: u32, sb: &mut Superblock) {
    let bp = bio::bread(dev, 1);
    let data = &bio::buf_mut(bp).data;

    sb.size = read_u32_le(data, 0);
    sb.nblocks = read_u32_le(data, 4);
    sb.ninodes = read_u32_le(data, 8);
    sb.nlog = read_u32_le(data, 12);
    sb.logstart = read_u32_le(data, 16);
    sb.inodestart = read_u32_le(data, 20);
    sb.bmapstart = read_u32_le(data, 24);

    bio::brelse(bp);
}

pub fn iinit(dev: u32) {
    unsafe {
        initlock(&raw mut ICACHE.lock, "icache\0".as_ptr());
        readsb(dev, &mut *(&raw mut SB));
        let sb = &*(&raw const SB);
        println!(
            "sb: size {} nblocks {} ninodes {} nlog {} logstart {} inodestart {} bmap start {}",
            sb.size, sb.nblocks, sb.ninodes, sb.nlog, sb.logstart, sb.inodestart, sb.bmapstart
        );
    }
}

fn bzero(dev: u32, bno: u32) {
    let bp = bio::bread(dev, bno);
    bio::buf_mut(bp).data.fill(0);
    log::log_write(bp);
    bio::brelse(bp);
}

fn balloc(dev: u32) -> u32 {
    unsafe {
        let mut b = 0u32;
        while b < SB.size {
            let bp = bio::bread(dev, bblock(b, &*(&raw const SB)));
            let mut found: Option<u32> = None;
            {
                let data = &mut bio::buf_mut(bp).data;
                let mut bi = 0u32;
                while bi < (BPB as u32) && b + bi < SB.size {
                    let m: u8 = 1u8 << (bi % 8);
                    let idx = (bi / 8) as usize;
                    if (data[idx] & m) == 0 {
                        data[idx] |= m;
                        found = Some(b + bi);
                        break;
                    }
                    bi += 1;
                }
            }

            if let Some(blockno) = found {
                log::log_write(bp);
                bio::brelse(bp);
                bzero(dev, blockno);
                return blockno;
            }

            bio::brelse(bp);
            b += BPB as u32;
        }
    }

    panic!("balloc: out of blocks");
}

fn bfree(dev: u32, b: u32) {
    unsafe {
        let bp = bio::bread(dev, bblock(b, &*(&raw const SB)));
        let bi = b % (BPB as u32);
        let m: u8 = 1u8 << (bi % 8);
        let idx = (bi / 8) as usize;

        {
            let data = &mut bio::buf_mut(bp).data;
            if (data[idx] & m) == 0 {
                panic!("freeing free block");
            }
            data[idx] &= !m;
        }

        log::log_write(bp);
        bio::brelse(bp);
    }
}

pub fn ialloc(dev: u32, type_: i16) -> usize {
    unsafe {
        let mut inum = 1u32;
        while inum < SB.ninodes {
            let bp = bio::bread(dev, iblock(inum, &*(&raw const SB)));
            let off = (inum % (IPB as u32)) as usize * DINODE_SIZE;

            let free = {
                let data = &bio::buf_mut(bp).data;
                read_i16_le(data, off) == 0
            };

            if free {
                {
                    let data = &mut bio::buf_mut(bp).data;
                    data[off..off + DINODE_SIZE].fill(0);
                    write_i16_le(data, off, type_);
                }
                log::log_write(bp);
                bio::brelse(bp);
                return iget(dev, inum);
            }

            bio::brelse(bp);
            inum += 1;
        }
    }

    panic!("ialloc: no inodes");
}

fn itrunc(idx: usize) {
    unsafe {
        if idx >= NINODE {
            panic!("itrunc: bad inode index");
        }

        let dev = ICACHE.inode[idx].dev;
        for i in 0..NDIRECT {
            let addr = ICACHE.inode[idx].addrs[i];
            if addr != 0 {
                bfree(dev, addr);
                ICACHE.inode[idx].addrs[i] = 0;
            }
        }

        let indirect = ICACHE.inode[idx].addrs[NDIRECT];
        if indirect != 0 {
            let bp = bio::bread(dev, indirect);
            for j in 0..NINDIRECT {
                let off = j * 4;
                let a = {
                    let data = &bio::buf_mut(bp).data;
                    read_u32_le(data, off)
                };
                if a != 0 {
                    bfree(dev, a);
                }
            }
            bio::brelse(bp);
            bfree(dev, indirect);
            ICACHE.inode[idx].addrs[NDIRECT] = 0;
        }

        ICACHE.inode[idx].size = 0;
    }

    iupdate(idx);
}

pub fn iput(idx: usize) {
    unsafe {
        acquire(&raw mut ICACHE.lock);
        if idx >= NINODE {
            panic!("iput: bad inode index");
        }
        if ICACHE.inode[idx].refcnt < 1 {
            panic!("iput: ref underflow");
        }

        if ICACHE.inode[idx].valid != 0 && ICACHE.inode[idx].nlink == 0 && ICACHE.inode[idx].refcnt == 1 {
            release(&raw mut ICACHE.lock); // inode has no links and no other references, truncate and free

            itrunc(idx);
            ICACHE.inode[idx].type_ = 0;
            iupdate(idx);
            ICACHE.inode[idx].valid = 0;

            acquire(&raw mut ICACHE.lock);
        }

        ICACHE.inode[idx].refcnt -= 1;
        release(&raw mut ICACHE.lock);
    }
}

pub fn irelease(idx: usize) {
    iput(idx);
}

pub fn iupdate(idx: usize) {
    unsafe {
        if idx >= NINODE {
            panic!("iupdate: bad inode index");
        }

        let ip = &ICACHE.inode[idx];
        let bp = bio::bread(ip.dev, iblock(ip.inum, &*(&raw const SB)));
        let off = (ip.inum % (IPB as u32)) as usize * DINODE_SIZE;

        {
            let data = &mut bio::buf_mut(bp).data;
            write_i16_le(data, off, ip.type_);
            write_i16_le(data, off + 2, ip.major);
            write_i16_le(data, off + 4, ip.minor);
            write_i16_le(data, off + 6, ip.nlink);
            write_u32_le(data, off + 8, ip.size);
            for i in 0..(NDIRECT + 1) {
                write_u32_le(data, off + 12 + (i * 4), ip.addrs[i]);
            }
        }

        log::log_write(bp);
        bio::brelse(bp);
    }
}

pub fn iget(dev: u32, inum: u32) -> usize {
    unsafe {
        acquire(&raw mut ICACHE.lock);

        let mut empty: Option<usize> = None;

        for i in 0..NINODE {
            let ip = &mut ICACHE.inode[i];

            if ip.refcnt > 0 && ip.dev == dev && ip.inum == inum {
                ip.refcnt += 1;
                release(&raw mut ICACHE.lock);
                return i;
            }

            if empty.is_none() && ip.refcnt == 0 {
                empty = Some(i);
            }
        }

        let idx = empty.unwrap_or_else(|| panic!("iget: no inodes"));
        let ip = &mut ICACHE.inode[idx];

        ip.dev = dev;
        ip.inum = inum;
        ip.refcnt = 1;
        ip.valid = 0;

        release(&raw mut ICACHE.lock);
        idx
    }
}

pub fn iread(idx: usize) {
    unsafe {
        if idx >= NINODE {
            panic!("iread: bad inode index");
        }

        let ip = &mut ICACHE.inode[idx];
        if ip.refcnt < 1 {
            panic!("iread");
        }

        if ip.valid == 0 {
            let bp = bio::bread(ip.dev, iblock(ip.inum, &*(&raw const SB)));
            let data = &bio::buf_mut(bp).data;

            let off = (ip.inum % (IPB as u32)) as usize * DINODE_SIZE;
            ip.type_ = read_i16_le(data, off);
            ip.major = read_i16_le(data, off + 2);
            ip.minor = read_i16_le(data, off + 4);
            ip.nlink = read_i16_le(data, off + 6);
            ip.size = read_u32_le(data, off + 8);

            for i in 0..(NDIRECT + 1) {
                ip.addrs[i] = read_u32_le(data, off + 12 + (i * 4));
            }

            bio::brelse(bp);

            ip.valid = 1;
            if ip.type_ == 0 {
                panic!("iread: no type");
            }
        }
    }
}

fn bmap(idx: usize, bn: u32) -> u32 {
    unsafe {
        if idx >= NINODE {
            panic!("bmap: bad inode index");
        }

        let ip = &mut ICACHE.inode[idx];

        if (bn as usize) < NDIRECT {
            let direct = &mut ip.addrs[bn as usize];
            if *direct == 0 {
                *direct = balloc(ip.dev);
            }
            return *direct;
        }

        let bn = bn - (NDIRECT as u32);
        if (bn as usize) < NINDIRECT {
            if ip.addrs[NDIRECT] == 0 {
                ip.addrs[NDIRECT] = balloc(ip.dev);
            }

            let bp = bio::bread(ip.dev, ip.addrs[NDIRECT]);
            let mut addr = {
                let data = &bio::buf_mut(bp).data;
                read_u32_le(data, (bn as usize) * 4)
            };
            if addr == 0 {
                addr = balloc(ip.dev);
                {
                    let data = &mut bio::buf_mut(bp).data;
                    write_u32_le(data, (bn as usize) * 4, addr);
                }
                log::log_write(bp);
            }
            bio::brelse(bp);
            return addr;
        }
    }

    panic!("bmap: out of range");
}

pub fn stati(idx: usize, st: &mut Stat) {
    unsafe {
        if idx >= NINODE {
            panic!("stati: bad inode index");
        }

        let ip = &ICACHE.inode[idx];
        st.dev = ip.dev as i32;
        st.ino = ip.inum;
        st.type_ = ip.type_;
        st.nlink = ip.nlink;
        st.size = ip.size;
    }
}

pub fn readi(idx: usize, dst: &mut [u8], off: u32, n: u32) -> i32 {
    unsafe {
        if idx >= NINODE {
            panic!("readi: bad inode index");
        }

        let ip = &ICACHE.inode[idx];
        
        // Handle device files
        if ip.type_ == T_DEV as i16 {
            if ip.major < 0 || (ip.major as usize) >= NDEV || DEVSW[ip.major as usize].read.is_none() {
                return -1;
            }
            return DEVSW[ip.major as usize].read.unwrap()(idx, dst, n as i32);
        }
        
        if off > ip.size || off.checked_add(n).is_none() || ip.nlink < 1 {
            return -1;
        }

        let mut n = n;
        if off + n > ip.size {
            n = ip.size - off;
        }

        if (n as usize) > dst.len() {
            panic!("readi: destination too small");
        }

        let mut tot: u32 = 0;
        let mut cur_off = off;
        let dev = ip.dev;

        while tot < n {
            let bp = bio::bread(dev, bmap(idx, cur_off / (BSIZE as u32)));

            let m = min((n - tot) as usize, BSIZE - (cur_off as usize % BSIZE));
            let boff = cur_off as usize % BSIZE;
            dst[tot as usize..tot as usize + m]
                .copy_from_slice(&bio::buf_mut(bp).data[boff..boff + m]);
            bio::brelse(bp);

            tot += m as u32;
            cur_off += m as u32;
        }

        n as i32
    }
}

pub fn writei(idx: usize, src: &[u8], off: u32, n: u32) -> i32 {
    unsafe {
        if idx >= NINODE {
            panic!("writei: bad inode index");
        }

        let ip = &ICACHE.inode[idx];
        
        // Handle device files
        if ip.type_ == T_DEV as i16 {
            if ip.major < 0 || (ip.major as usize) >= NDEV || DEVSW[ip.major as usize].write.is_none() {
                return -1;
            }
            return DEVSW[ip.major as usize].write.unwrap()(idx, src, n as i32);
        }
        
        if off > ip.size || off.checked_add(n).is_none() {
            return -1;
        }
        if off + n > (MAXFILE * BSIZE) as u32 {
            return -1;
        }
        if (n as usize) > src.len() {
            panic!("writei: source too small");
        }

        let dev = ip.dev;
        let mut tot: u32 = 0;
        let mut cur_off = off;

        while tot < n {
            let bp = bio::bread(dev, bmap(idx, cur_off / (BSIZE as u32)));
            let m = min((n - tot) as usize, BSIZE - (cur_off as usize % BSIZE));
            let boff = cur_off as usize % BSIZE;
            bio::buf_mut(bp).data[boff..boff + m]
                .copy_from_slice(&src[tot as usize..tot as usize + m]);
            log::log_write(bp);
            bio::brelse(bp);

            tot += m as u32;
            cur_off += m as u32;
        }

        if n > 0 && cur_off > ICACHE.inode[idx].size {
            ICACHE.inode[idx].size = cur_off;
            iupdate(idx);
        }

        n as i32
    }
}

pub fn inode_inum(idx: usize) -> u32 {
    unsafe {
        if idx >= NINODE {
            panic!("inode_inum: bad inode index");
        }
        ICACHE.inode[idx].inum
    }
}

pub fn inode_dev(idx: usize) -> u32 {
    unsafe {
        if idx >= NINODE {
            panic!("inode_dev: bad inode index");
        }
        ICACHE.inode[idx].dev
    }
}

pub fn inode_type(idx: usize) -> u16 {
    unsafe {
        if idx >= NINODE {
            panic!("inode_type: bad inode index");
        }
        ICACHE.inode[idx].type_ as u16
    }
}

pub fn inode_nlink(idx: usize) -> u16 {
    unsafe {
        if idx >= NINODE {
            panic!("inode_nlink: bad inode index");
        }
        ICACHE.inode[idx].nlink as u16
    }
}

pub fn inode_size(idx: usize) -> u32 {
    unsafe {
        if idx >= NINODE {
            panic!("inode_size: bad inode index");
        }
        ICACHE.inode[idx].size
    }
}

pub fn inode_set_meta(idx: usize, major: i16, minor: i16, nlink: i16) {
    unsafe {
        if idx >= NINODE {
            panic!("inode_set_meta: bad inode index");
        }
        ICACHE.inode[idx].major = major;
        ICACHE.inode[idx].minor = minor;
        ICACHE.inode[idx].nlink = nlink;
    }
}

pub fn inode_inc_nlink(idx: usize) {
    unsafe {
        if idx >= NINODE {
            panic!("inode_inc_nlink: bad inode index");
        }
        ICACHE.inode[idx].nlink += 1;
    }
}

pub fn inode_dec_nlink(idx: usize) {
    unsafe {
        if idx >= NINODE {
            panic!("inode_dec_nlink: bad inode index");
        }
        ICACHE.inode[idx].nlink -= 1;
    }
}

pub fn namecmp(s: &str, t: &[u8; DIRSIZ]) -> bool {
    name_to_dirsiz(s) == *t
}

pub fn dirlookup(dp_idx: usize, name: &str, mut poff: Option<&mut u32>) -> Option<usize> {
    if dp_idx >= NINODE {
        panic!("dirlookup: bad inode index");
    }
    iread(dp_idx);

    unsafe {
        let dp = &ICACHE.inode[dp_idx];
        if (dp.type_ as u16) != T_DIR {
            panic!("dirlookup not DIR");
        }

        let mut off = 0u32;
        while off < dp.size {
            let mut raw = [0u8; DIRENT_SIZE];
            if readi(dp_idx, &mut raw, off, DIRENT_SIZE as u32) != DIRENT_SIZE as i32 {
                panic!("dirlookup read");
            }
            let de = parse_dirent(&raw);
            if de.inum != 0 && namecmp(name, &de.name) {
                if let Some(ref mut out_off) = poff {
                    **out_off = off;
                }
                return Some(iget(dp.dev, de.inum as u32));
            }
            off += DIRENT_SIZE as u32;
        }
    }

    None
}

pub fn dirlink(dp_idx: usize, name: &str, inum: u32) -> i32 {
    if let Some(ip_idx) = dirlookup(dp_idx, name, None) {
        iput(ip_idx);
        return -1;
    }

    let mut off = 0u32;
    unsafe {
        if dp_idx >= NINODE {
            panic!("dirlink: bad inode index");
        }
        while off < ICACHE.inode[dp_idx].size {
            let mut raw = [0u8; DIRENT_SIZE];
            if readi(dp_idx, &mut raw, off, DIRENT_SIZE as u32) != DIRENT_SIZE as i32 {
                panic!("dirlink read");
            }
            let de = parse_dirent(&raw);
            if de.inum == 0 {
                break;
            }
            off += DIRENT_SIZE as u32;
        }
    }

    let mut raw = [0u8; DIRENT_SIZE];
    write_u16_le(&mut raw, 0, inum as u16);
    raw[2..2 + DIRSIZ].copy_from_slice(&name_to_dirsiz(name));
    if writei(dp_idx, &raw, off, DIRENT_SIZE as u32) != DIRENT_SIZE as i32 {
        panic!("dirlink");
    }

    0
}

fn skipelem(path: &[u8], mut i: usize, name: &mut [u8; DIRSIZ]) -> Option<usize> {
    while i < path.len() && path[i] == b'/' {
        i += 1;
    }
    if i == path.len() {
        return None;
    }

    let start = i;
    // Find end of this path element
    while i < path.len() && path[i] != b'/' && path[i] != 0 {
        i += 1;
    }
    let len = i - start;

    name.fill(0);
    if len >= DIRSIZ {
        name.copy_from_slice(&path[start..start + DIRSIZ]);
    } else {
        name[..len].copy_from_slice(&path[start..i]);
    }

    while i < path.len() && path[i] == b'/' {
        i += 1;
    }
    Some(i)
}

fn namex(path: &str, nameiparent: bool, name: &mut [u8; DIRSIZ]) -> Option<usize> {
    let bytes = path.as_bytes();
    let mut path_idx = 0usize;
    let mut ip = iget(ROOTDEV, ROOTINO);

    while let Some(next_idx) = skipelem(bytes, path_idx, name) {
        path_idx = next_idx;
        iread(ip);

        unsafe {
            if ICACHE.inode[ip].type_ as u16 != T_DIR {
                iput(ip);
                return None;
            }
        }

        if nameiparent && path_idx == bytes.len() {
            return Some(ip);
        }

        let elem = core::str::from_utf8(name).unwrap_or("");
        let trimmed = elem.trim_end_matches('\0');
        let next = match dirlookup(ip, trimmed, None) {
            Some(x) => x,
            None => {
                iput(ip);
                return None;
            }
        };
        iput(ip);
        ip = next;
    }

    if nameiparent {
        iput(ip);
        return None;
    }
    Some(ip)
}

/// Look up the inode for a path name.
pub fn namei(path: &str) -> Option<usize> {
    let mut name = [0u8; DIRSIZ];
    namex(path, false, &mut name)
}

/// Look up the parent inode for a path name.
/// Copy the final path element into name.
pub fn nameiparent(path: &str, name: &mut [u8; DIRSIZ]) -> Option<usize> {
    namex(path, true, name)
}
