use core::mem::size_of;
use core::sync::atomic::Ordering;

use crate::bio;
use crate::buf::{B_DIRTY, BSIZE};
use crate::fs;
use crate::param::LOGSIZE;

#[derive(Copy, Clone)]
struct LogHeader {
    n: i32,
    block: [u32; LOGSIZE],
}

impl LogHeader {
    const fn new() -> Self {
        Self {
            n: 0,
            block: [0; LOGSIZE],
        }
    }
}

struct Log {
    start: u32,
    size: u32,
    committing: bool,
    dev: u32,
    lh: LogHeader,
}

impl Log {
    const fn new() -> Self {
        Self {
            start: 0,
            size: 0,
            committing: false,
            dev: 0,
            lh: LogHeader::new(),
        }
    }
}

static mut LOG: Log = Log::new();

#[inline]
fn read_i32_le(data: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[inline]
fn write_i32_le(data: &mut [u8], off: usize, val: i32) {
    data[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

#[inline]
fn read_u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[inline]
fn write_u32_le(data: &mut [u8], off: usize, val: u32) {
    data[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

fn read_head() {
    unsafe {
        let buf = bio::bread(LOG.dev, LOG.start);
        let data = &bio::buf_mut(buf).data;
        LOG.lh.n = read_i32_le(data, 0);
        for i in 0..(LOG.lh.n as usize) {
            LOG.lh.block[i] = read_u32_le(data, 4 + i * 4);
        }
        bio::brelse(buf);
    }
}

fn write_head() {
    unsafe {
        let buf = bio::bread(LOG.dev, LOG.start);
        let data = &mut bio::buf_mut(buf).data;
        data.fill(0);
        write_i32_le(data, 0, LOG.lh.n);
        for i in 0..(LOG.lh.n as usize) {
            write_u32_le(data, 4 + i * 4, LOG.lh.block[i]);
        }
        bio::bwrite(buf);
        bio::brelse(buf);
    }
}

fn install_trans() {
    unsafe {
        for tail in 0..(LOG.lh.n as usize) {
            let lbuf = bio::bread(LOG.dev, LOG.start + tail as u32 + 1);
            let dbuf = bio::bread(LOG.dev, LOG.lh.block[tail]);
            let src = bio::buf_mut(lbuf).data;
            bio::buf_mut(dbuf).data.copy_from_slice(&src);
            bio::bwrite(dbuf);
            bio::brelse(lbuf);
            bio::brelse(dbuf);
        }
    }
}

fn recover_from_log() {
    read_head();
    install_trans();
    unsafe {
        LOG.lh.n = 0;
    }
    write_head();
}

fn write_log() {
    unsafe {
        for tail in 0..(LOG.lh.n as usize) {
            let to = bio::bread(LOG.dev, LOG.start + tail as u32 + 1);
            let from = bio::bread(LOG.dev, LOG.lh.block[tail]);
            let src = bio::buf_mut(from).data;
            bio::buf_mut(to).data.copy_from_slice(&src);
            bio::bwrite(to);
            bio::brelse(from);
            bio::brelse(to);
        }
    }
}

fn commit() {
    unsafe {
        if LOG.committing {
            panic!("commit");
        }
        LOG.committing = true;
        if LOG.lh.n > 0 {
            write_log();
            write_head();
            install_trans();
            LOG.lh.n = 0;
            write_head();
        }
        LOG.committing = false;
    }
}

pub fn initlog(dev: u32) {
    if size_of::<LogHeader>() >= BSIZE {
        panic!("initlog: too big logheader");
    }

    let mut sb = fs::Superblock::new();
    fs::readsb(dev, &mut sb);
    unsafe {
        LOG.start = sb.logstart;
        LOG.size = sb.nlog;
        LOG.dev = dev;
    }
    recover_from_log();
}

pub fn begin_op() {}

pub fn end_op() {
    commit();
}

pub fn log_write(idx: usize) {
    unsafe {
        let blockno = bio::buf_mut(idx).blockno;
        if LOG.lh.n as usize >= LOGSIZE || LOG.lh.n as u32 >= LOG.size.saturating_sub(1) {
            panic!("too big a transaction");
        }

        let mut i = 0usize;
        while i < LOG.lh.n as usize {
            if LOG.lh.block[i] == blockno {
                break;
            }
            i += 1;
        }

        LOG.lh.block[i] = blockno;
        if i == LOG.lh.n as usize {
            LOG.lh.n += 1;
        }

        bio::buf_mut(idx).flags.fetch_or(B_DIRTY, Ordering::AcqRel);
    }
}
