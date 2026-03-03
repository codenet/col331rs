use crate::mp::MP_ONCE;  // Import the MP_ONCE static from mp.rs
// use core::ptr;
use crate::x86::readeflags;
use crate::constants::{FL_IF,NCPU};
use crate::lapic;

#[derive(Debug, Clone, Copy)]
pub struct Cpu {
    pub apicid: u8,  // Local APIC ID
}

impl Cpu {
    pub const fn new() -> Self {
        Self { apicid: 0 }
    }
}

pub fn cpuid() -> usize {
    let cpus = MP_ONCE.cpus.get().expect("CPUs not initialized");
    unsafe { (mycpu() as *const Cpu).offset_from(cpus.as_ptr()) as usize }
}

pub fn mycpu() -> &'static Cpu {
    let apicid: usize;
    let mut i: usize = 0;

    if readeflags() & FL_IF != 0 {
        panic!("mycpu called with interrupts enabled\n");
    }

    apicid = lapic::lapicid() as usize;

    // Access the cpus array via MP_ONCE
    let cpus = MP_ONCE.cpus.get().expect("CPUs not initialized");

    while i < NCPU {
        if (cpus[i].apicid as usize) == apicid {
            return &cpus[i];
        }
        i += 1;
    }
    panic!("unknown apicid\n");
}