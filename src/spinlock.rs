// Mutual exclusion spin locks and debugging utilities
use crate::constants::FL_IF;
use crate::proc::mycpu;
use crate::x86::{cli, readeflags, sti};

#[repr(C)]
#[derive(Default)]
pub struct Spinlock {
    pub locked: u32,          // Is the lock held?
    pub name: *const u8,      // Name of lock
}

impl Spinlock {
    pub const fn new() -> Self {
        Self {
            locked: 0,
            name: core::ptr::null(),
        }
    }
}


/// Record the current call stack in pcs[] by following the %ebp chain.
pub fn getcallerpcs(v: *const u32, pcs: &mut [u32; 10]) {
    unsafe {
        let mut ebp = v.offset(-2);
        let mut i = 0;

        while i < 10 {
            if ebp.is_null() || ebp as usize == 0xffffffff {
                break;
            }

            // saved %eip
            pcs[i] = *ebp.add(1);

            // saved %ebp
            ebp = *ebp as *const u32;

            i += 1;
        }

        while i < 10 {
            pcs[i] = 0;
            i += 1;
        }
    }
}

/// Disable interrupts with nesting support
pub fn pushcli() {
    let eflags = readeflags();
    cli();

    let c = mycpu();

    if c.ncli == 0 {
        c.intena = if (eflags & FL_IF) != 0 { 1 } else { 0 };
    }

    c.ncli += 1;
}

/// Restore interrupt state
pub fn popcli() {
    if (readeflags() & FL_IF) != 0 {
        panic!("popcli - interruptible");
    }

    let c = mycpu();

    c.ncli -= 1;

    if c.ncli < 0 {
        panic!("popcli");
    }

    if c.ncli == 0 && c.intena != 0 {
        sti();
    }
}

/// Initialize a spinlock
pub fn initlock(lk: * mut Spinlock, name: *const u8) {
    unsafe {
    (*lk).name = name;
    (*lk).locked = 0;
    }
}

/// Acquire the lock
pub fn acquire(lk: * mut Spinlock) {
    unsafe {
    pushcli();
    (*lk).locked = 1;
    }
}

/// Release the lock
pub fn release(lk: * mut Spinlock) {
    unsafe {
    (*lk).locked = 0;
    popcli();
    }
}