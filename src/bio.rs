use core::sync::atomic::Ordering;

use crate::buf::{Buf, B_DIRTY, B_VALID, NBUF};

const HEAD: usize = NBUF; // sentinel index

struct BCache {
    buf: [Buf; NBUF],
    head_prev: usize,
    head_next: usize,
}

impl BCache {
    pub const fn new() -> Self {
        Self {
            buf: [const { Buf::new() }; NBUF],
            head_prev: HEAD,
            head_next: HEAD,
        }
    }
}

static mut BCACHE: BCache = BCache::new();

pub fn binit() {
    unsafe {
        // empty list
        BCACHE.head_prev = HEAD;
        BCACHE.head_next = HEAD;

        // insert all buffers at head (MRU side)
        for i in 0..NBUF {
            insert_at_head(i);
        }
    }
}

#[inline]
fn insert_at_head(i: usize) {
    unsafe {
        let first = BCACHE.head_next;

        BCACHE.buf[i].prev = HEAD;
        BCACHE.buf[i].next = first;

        if first == HEAD {
            // list was empty
            BCACHE.head_prev = i;
        } else {
            BCACHE.buf[first].prev = i;
        }

        BCACHE.head_next = i;
    }
}

#[inline]
fn remove_from_list(i: usize) {
    unsafe {
        let prev = BCACHE.buf[i].prev;
        let next = BCACHE.buf[i].next;

        if prev == HEAD {
            BCACHE.head_next = next;
        } else {
            BCACHE.buf[prev].next = next;
        }

        if next == HEAD {
            BCACHE.head_prev = prev;
        } else {
            BCACHE.buf[next].prev = prev;
        }
    }
}

// Return mutable buf by index.
// Safe to call only when you “own” the buffer logically (like xv6 “locked buf”).
pub fn buf_mut(idx: usize) -> &'static mut Buf {
    unsafe { &mut BCACHE.buf[idx] }
}

// Look for cached block; else recycle an unused non-dirty buffer.
fn bget(dev: u32, blockno: u32) -> usize {
    unsafe {
        // Is the block already cached?
        let mut b = BCACHE.head_next;
        while b != HEAD {
            if BCACHE.buf[b].dev == dev && BCACHE.buf[b].blockno == blockno {
                BCACHE.buf[b].refcnt += 1;
                return b;
            }
            b = BCACHE.buf[b].next;
        }

        // Not cached; recycle from LRU end.
        let mut b = BCACHE.head_prev;
        while b != HEAD {
            let flags = BCACHE.buf[b].flags.load(Ordering::Acquire);
            if BCACHE.buf[b].refcnt == 0 && (flags & B_DIRTY) == 0 {
                BCACHE.buf[b].dev = dev;
                BCACHE.buf[b].blockno = blockno;
                BCACHE.buf[b].flags.store(0, Ordering::Release);
                BCACHE.buf[b].refcnt = 1;
                BCACHE.buf[b].qnext = None;
                return b;
            }
            b = BCACHE.buf[b].prev;
        }

        panic!("bget: no buffers");
    }
}

// Return buffer index with contents of block.
pub fn bread(dev: u32, blockno: u32) -> usize {
    let idx = bget(dev, blockno);

    let flags = buf_mut(idx).flags.load(Ordering::Acquire);
    if (flags & B_VALID) == 0 {
        crate::ide::iderw(idx);
    }

    idx
}

// Mark dirty + write to disk.
pub fn bwrite(idx: usize) {
    let b = buf_mut(idx);
    b.flags.fetch_or(B_DIRTY, Ordering::AcqRel);
    crate::ide::iderw(idx);
}

// Release buffer. If refcnt hits 0, move to MRU head.
pub fn brelse(idx: usize) {
    unsafe {
        let b = &mut BCACHE.buf[idx];
        if b.refcnt == 0 {
            panic!("brelse: refcnt underflow");
        }

        b.refcnt -= 1;
        if b.refcnt == 0 {
            remove_from_list(idx);
            insert_at_head(idx);
        }
    }
}