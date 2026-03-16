use core::sync::atomic::Ordering;
use crate::constants::BSIZE;
use crate::buf::{B_DIRTY, B_VALID};
use crate::spinlock::{Spinlock, initlock, release, acquire};
use crate::x86;

const SECTOR_SIZE: usize = 512;

const IDE_BSY: u8 = 0x80;
const IDE_DRDY: u8 = 0x40;
const IDE_DF: u8 = 0x20;
const IDE_ERR: u8 = 0x01;

const IDE_CMD_READ: u8 = 0x20;
const IDE_CMD_WRITE: u8 = 0x30;
const IDE_CMD_RDMUL: u8 = 0xC4;
const IDE_CMD_WRMUL: u8 = 0xC5;

// If you later build a real FS image, keep this consistent with your mkfs.
// xv6 uses 1000.
const FSSIZE: u32 = 1000;

static mut IDEQUEUE: Option<usize> = None;
static mut HAVEDISK1: bool = false;
static mut IDELOCK: Spinlock = Spinlock::new();

fn idewait(checkerr: bool) -> i32 {
    let mut r: u8;
    loop {
        r = x86::inb(0x1F7);
        if (r & (IDE_BSY | IDE_DRDY)) == IDE_DRDY {
            break;
        }
    }
    if checkerr && (r & (IDE_DF | IDE_ERR)) != 0 {
        return -1;
    }
    0
}

pub fn ideinit() {
    // Route IDE IRQ somewhere; simplest is CPU 0 for now.

    initlock(&raw mut IDELOCK, b"ide\0".as_ptr());
    
    crate::ioapic::ioapic_enable(crate::constants::IRQ_IDE, 0);

    idewait(false);

    // Check if disk 1 is present
    unsafe {
        x86::outb(0x1F6, 0xE0 | (1 << 4));
        for _ in 0..1000 {
            if x86::inb(0x1F7) != 0 {
                HAVEDISK1 = true;
                break;
            }
        }
        // Switch back to disk 0
        x86::outb(0x1F6, 0xE0 | (0 << 4));
    }
}

fn idestart(idx: usize) {
    let b = crate::bio::buf_mut(idx);

    if b.blockno >= FSSIZE {
        panic!("idestart: incorrect blockno");
    }

    let sector_per_block = BSIZE / SECTOR_SIZE; // usually 1
    let sector = (b.blockno as usize) * sector_per_block;

    let read_cmd = if sector_per_block == 1 { IDE_CMD_READ } else { IDE_CMD_RDMUL };
    let write_cmd = if sector_per_block == 1 { IDE_CMD_WRITE } else { IDE_CMD_WRMUL };

    if sector_per_block > 7 {
        panic!("idestart: sector_per_block > 7");
    }

    idewait(false);
    x86::outb(0x3F6, 0); // generate interrupt

    x86::outb(0x1F2, sector_per_block as u8); // number of sectors
    x86::outb(0x1F3, (sector & 0xFF) as u8);
    x86::outb(0x1F4, ((sector >> 8) & 0xFF) as u8);
    x86::outb(0x1F5, ((sector >> 16) & 0xFF) as u8);
    x86::outb(
        0x1F6,
        0xE0 | (((b.dev & 1) as u8) << 4) | (((sector >> 24) & 0x0F) as u8),
    );

    let flags = b.flags.load(Ordering::Acquire);
    if (flags & B_DIRTY) != 0 {
        x86::outb(0x1F7, write_cmd);

        // write BSIZE bytes as u32 words
        unsafe {
            x86::outsl(0x1F0, b.data.as_ptr() as *const u32, BSIZE / 4);
        }
    } else {
        x86::outb(0x1F7, read_cmd);
    }
}

// Interrupt handler.
pub fn ideintr() {
    let idx = unsafe {
        match IDEQUEUE {
            None => return,
            Some(i) => {
                let b = crate::bio::buf_mut(i);
                IDEQUEUE = b.qnext;
                b.qnext = None;
                i
            }
        }
    };

    let b = crate::bio::buf_mut(idx);

    let flags = b.flags.load(Ordering::Acquire);

    // Read data if needed.
    if (flags & B_DIRTY) == 0 && idewait(true) >= 0 {
        unsafe {
            x86::insl(0x1F0, b.data.as_mut_ptr() as *mut u32, BSIZE / 4);
        }
    }

    b.flags.fetch_or(B_VALID, Ordering::AcqRel);
    b.flags.fetch_and(!B_DIRTY, Ordering::AcqRel);

    // Start next buffer in queue.
    unsafe {
        if let Some(next) = IDEQUEUE {
            idestart(next);
        }
    }
}

// Sync buf with disk.
// If B_DIRTY is set, write buf to disk, clear B_DIRTY, set B_VALID.
// Else if B_VALID is not set, read buf from disk, set B_VALID.
pub fn iderw(idx: usize) {

    acquire(&raw mut IDELOCK);

    let b = crate::bio::buf_mut(idx);

    let flags = b.flags.load(Ordering::Acquire);
    if (flags & (B_VALID | B_DIRTY)) == B_VALID {
        panic!("iderw: nothing to do");
    }

    unsafe {
        if b.dev != 0 && !HAVEDISK1 {
            panic!("iderw: ide disk 1 not present");
        }
    }

    // Append b to idequeue.
    b.qnext = None;
    unsafe {
        let mut pp: *mut Option<usize> = &raw mut IDEQUEUE;
        while let Some(next_idx) = *pp {
            pp = &raw mut crate::bio::buf_mut(next_idx).qnext;
        }
        *pp = Some(idx);
    }

    // Start disk if necessary.
    unsafe {
        if IDEQUEUE == Some(idx) {
            idestart(idx);
        }
    }
    
    release(&raw mut IDELOCK);
    // Wait for request to finish.
    // The interrupt handler will set B_VALID when done.
    loop {
        let flags = b.flags.load(Ordering::Acquire);
        if (flags & (B_VALID | B_DIRTY)) == B_VALID {
            break;
        }
        // Force compiler to re-read b->flags which is modified by ideintr()
        x86::noop();
    }
}