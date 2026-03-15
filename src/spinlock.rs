// Mutual exclusion spin locks and debugging utilities
use crate::constants::FL_IF;
use crate::proc::mycpu;
use crate::x86::{cli, readeflags, sti};

/// Record the current call stack in pcs[] by following the %ebp chain.
/// This function walks the stack frame pointers to collect return addresses.
pub fn getcallerpcs(v: *const u32, pcs: &mut [u32; 10]) {
    unsafe {
        let mut ebp = v.offset(-2) as *const u32;
        let mut i = 0;
        
        while i < 10 {
            // Check for invalid ebp values
            // if(ebp == 0 || ebp < (uint*)KERNBASE || ebp == (uint*)0xffffffff)
            if ebp.is_null() || ebp == 0xffffffff as *const u32 {
                break;
            }
            
            // pcs[i] = ebp[1]; // saved %eip
            pcs[i] = *ebp.offset(1);
            
            // ebp = (uint*)ebp[0]; // saved %ebp
            ebp = *ebp as *const u32;
            
            i += 1;
        }
        
        // Fill rest with zeros
        while i < 10 {
            pcs[i] = 0;
            i += 1;
        }
    }
}

// Pushcli/popcli are like cli/sti except that they are matched:
// it takes two popcli to undo two pushcli.
pub fn pushcli() {
    let eflags = readeflags();
    cli();
    let c = mycpu();
    if c.ncli == 0 {
        c.intena = ((eflags & FL_IF) != 0) as i32;
    }
    c.ncli += 1;
}

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
