use core::sync::atomic::AtomicU32;

// Re-export filesystem constants from constants.rs for convenience
// (other modules currently import BSIZE and NBUF from buf)
pub use crate::constants::{BSIZE, NBUF};

// Buffer flags
pub const B_VALID: u32 = 0x2; // buffer has been read from disk
pub const B_DIRTY: u32 = 0x4; // buffer needs to be written to disk

#[repr(C)]
pub struct Buf {
    pub flags: AtomicU32,
    pub dev: u32,
    pub blockno: u32,
    pub refcnt: u32,

    // LRU list (intrusive, by index)
    pub prev: usize,
    pub next: usize,

    // disk queue (by index)
    pub qnext: Option<usize>,

    pub data: [u8; BSIZE],
}

impl Buf {
    pub const fn new() -> Self {
        Self {
            flags: AtomicU32::new(0),
            dev: 0,
            blockno: 0,
            refcnt: 0,
            prev: 0,
            next: 0,
            qnext: None,
            data: [0; BSIZE],
        }
    }
}