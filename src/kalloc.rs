use crate::constants::{pgroundup, PGSIZE};
use core::ptr::null_mut;

#[repr(C)]
struct Run {
    next: *mut Run,
}

struct KMem {
    freelist: *mut Run,
}

static mut KMEM: KMem = KMem { freelist: null_mut() };

extern "C" {
    static end: u8;
}

pub fn kinit(vstart: *mut u8, vend: *mut u8) {
    freerange(vstart, vend);
}

fn freerange(vstart: *mut u8, vend: *mut u8) {
    let mut p = pgroundup(vstart as usize) as *mut u8;
    while (p as usize) + PGSIZE as usize <= vend as usize {
        kfree(p);
        p = unsafe { p.add(PGSIZE as usize) };
    }
}

pub fn kfree(v: *mut u8) {
    if (v as usize) % PGSIZE as usize != 0 || (v as usize) < (unsafe { &end as *const u8 as usize }) {
        panic!("kfree");
    }

    unsafe {
        core::ptr::write_bytes(v, 1, PGSIZE as usize);
        let r = v as *mut Run;
        (*r).next = KMEM.freelist;
        KMEM.freelist = r;
    }
}

pub fn kalloc() -> *mut u8 {
    unsafe {
        let r = KMEM.freelist;
        if !r.is_null() {
            KMEM.freelist = (*r).next;
        }
        r as *mut u8
    }
}
