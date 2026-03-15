use crate::mp::MP_ONCE;
use crate::constants::{NSEGS, SEG_UCODE, SEG_UDATA, DPL_USER, FL_IF, PGSIZE};
use crate::mmu::{SegDesc, TaskState};
// use crate::println;
// use crate::println;
use crate::debug;
use crate::x86::TrapFrame;
use crate::param::KSTACKSIZE;
use crate::param::NPROC;
use crate::param::NOFILE;
use crate::kalloc::kalloc;
use crate::spinlock::{Spinlock, initlock, acquire, release};
use core::ptr::null_mut;
// use core::cell::OnceCell;

// Saved registers for kernel context switches.
// Don't need to save all the segment registers (%cs, etc),
// because they are constant across kernel contexts.
// Don't need to save %eax, %ecx, %edx, because the
// x86 convention is that the caller has saved them.
// Contexts are stored at the bottom of the stack they
// describe; the stack pointer is the address of the context.
// The layout of the context matches the layout of the stack in swtch.S
// at the "Switch stacks" comment. Switch doesn't save eip explicitly,
// but it is on the stack and allocproc() manipulates it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub edi: u32,
    pub esi: u32,
    pub ebx: u32,
    pub ebp: u32,
    pub eip: u32,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Unused,
    Embryo,
    Runnable,
    Running,
}

// Per-process state
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Proc {
    pub sz: u32,                      // Size of process memory (bytes)
    pub offset: *mut u8,              // Process memory base
    pub kstack: *mut u8,              // Bottom of kernel stack for this process (unused for now)
    pub state: ProcState,             // Process state
    pub pid: i32,                     // Process ID
    pub parent: *mut Proc,            // Parent process
    pub tf: *mut TrapFrame,           // Trap frame for current syscall
    pub context: *mut Context,        // swtch() here to run process
    pub ofile: [Option<usize>; NOFILE], // Open files
    pub cwd: usize,                   // Current directory (inode number)
    pub name: [u8; 16],               // Process name (debugging)
}

impl Proc {
    pub const fn new() -> Self {
        Self {
            sz: 0,
            offset: null_mut(),
            kstack: null_mut(),
            state: ProcState::Unused,
            pid: 0,
            parent: null_mut(),
            tf: null_mut(),
            context: null_mut(),
            ofile: [None; NOFILE],
            cwd: 0,
            name: [0; 16],
        }
    }
}

// Process table
struct PTable {
    lock: Spinlock,
    proc: [Proc; NPROC],
}

// static mut PTABLE: OnceCell<PTable> = OnceCell::new();
static mut PTABLE: PTable = PTable {
    lock: Spinlock::new(),
    proc: [Proc::new(); NPROC],
};
static mut NEXTPID: i32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cpu {
    pub apicid: u8,                   // Local APIC ID
    pub scheduler: *mut Context,      // swtch() here to enter scheduler
    pub ts: TaskState,                // Used by x86 to find stack for interrupt
    pub gdt: [SegDesc; NSEGS],        // x86 global descriptor table
    pub ncli: i32,                    // Depth of pushcli nesting.
    pub intena: i32,                  // Were interrupts enabled before pushcli?
    pub proc: *mut Proc,              // The process running on this cpu or null
}

impl Cpu {
    pub const fn new() -> Self {
        Self { 
            apicid: 0,
            scheduler: null_mut(),
            ts: TaskState::new(),
            gdt: [SegDesc::new(); NSEGS],
            ncli: 0,
            intena: 0,
            proc: null_mut(),
        }
    }
}

pub fn cpuid() -> usize {
    // For now, always return 0 (single CPU)
    0
}

pub fn mycpu() -> &'static mut Cpu {
    let cpus = MP_ONCE.cpus.get().expect("CPUs not initialized");
    unsafe { &mut *(cpus.as_ptr() as *mut Cpu).add(cpuid()) }
}

// Read proc from the cpu structure
pub fn myproc() -> Option<&'static mut Proc> {
    let c = mycpu();
    if c.proc.is_null() {
        None
    } else {
        Some(unsafe { &mut *c.proc })
    }
}

// External symbols from assembly
extern "C" {
    fn trapret();
    pub fn swtch(old: *mut *mut Context, new: *mut Context);
}

// Look in the process table for an UNUSED proc.
// If found, change state to EMBRYO and initialize
// state required to run in the kernel.
// Otherwise return None.
fn allocproc() -> Option<&'static mut Proc> {
    unsafe {  
        acquire(&raw mut PTABLE.lock);
        let ptable = &raw mut PTABLE;
        
        for p in &mut (*ptable).proc {
            // debug!("process : pid {}, state {:?}", p.pid, p.state);
            if p.state == ProcState::Unused {
                // Found an unused process
                // debug!("allocproc: found unused process with pid {}", p.pid);
                p.state = ProcState::Embryo;    // Reserved for initialization
                p.pid = NEXTPID;
                NEXTPID += 1;
                release(&raw mut PTABLE.lock);

                p.offset = kalloc();
                if p.offset.is_null() {
                    p.state = ProcState::Unused;
                    return None;
                }
                p.sz = PGSIZE;

                // kstack lives on a different segment

                p.kstack = kalloc();
                if p.kstack.is_null() {
                    p.state = ProcState::Unused;
                    return None;
                } 

                let mut sp = p.kstack.add(PGSIZE as usize);

                p.ofile = [None; NOFILE];
                
                // Calculate stack pointer at the end of process memory
        
                p.kstack = sp.sub(KSTACKSIZE);

                sp = sp.sub(core::mem::size_of::<TrapFrame>());
                p.tf = sp as *mut TrapFrame;
                // Set up new context to start executing at trapret
                // which returns to trapret
                sp = sp.sub(4); 

                
                core::ptr::write(sp as *mut u32, trapret as *const () as u32);
                
                
                sp = sp.sub(core::mem::size_of::<Context>());
                p.context = sp as *mut Context;
                
                // Initialize context
                core::ptr::write_bytes(p.context, 0, 1);
                (*p.context).eip = forkret as *const ()as u32;
                // debug!("allocproc: initialized process with pid {}", p.pid);
                return Some(p);
            }
        }
        release(&raw mut PTABLE.lock);
        None
    }
}

// Set up first process.
pub fn pinit() {
    unsafe {
        initlock(&raw mut PTABLE.lock, "ptable\0".as_ptr());
        extern "C" {
            static _binary_initcode_start: u8;
            static _binary_initcode_size: u8;
        }
        // debug!("Initializing first user process");
        let p = allocproc().expect("Failed to allocate first process");
        // debug!("Returned from allocproc with pid {}", p.pid);
        // Copy initcode binary to process memory
        let dst = p.offset;
        let src = &_binary_initcode_start as *const u8;
        let size = &_binary_initcode_size as *const u8 as usize;
        debug!("Copying initcode to process memory: src={:p}, dst={:p}, size={}", src, dst, size);
        // debug!("initcode size = {}", _binary_initcode_size as usize);
        core::ptr::copy_nonoverlapping(src, dst, size);
        
        // Initialize trapframe
        core::ptr::write_bytes(p.tf, 0, 1);
        
        (*p.tf).cs = ((SEG_UCODE << 3) | DPL_USER as u16) as u16;
        (*p.tf).ds = ((SEG_UDATA << 3) | DPL_USER as u16) as u16;
        (*p.tf).es = (*p.tf).ds;
        (*p.tf).ss = (*p.tf).ds;
        (*p.tf).eflags = FL_IF;
        (*p.tf).esp = PGSIZE;
        (*p.tf).eip = 0; // beginning of initcode.S
        
        // Set process name
        let name = b"initcode";
        for (i, &byte) in name.iter().enumerate() {
            p.name[i] = byte;
        }
        
        // Set current working directory to root
        p.cwd = crate::fs::namei("/").expect("Failed to find root directory");
        
        p.state = ProcState::Runnable;
    }
}

// Process scheduler.
// Scheduler never returns. It loops, doing:
//  - choose a process to run
//  - swtch to start running that process
//  - eventually that process transfers control
//      via swtch back to the scheduler.
pub fn scheduler() -> ! {
    let c = mycpu();
    c.proc = null_mut();
    
    loop {
        // Enable interrupts on this processor.
        crate::x86::sti();
        
        // Loop over process table looking for process to run.
        unsafe {
            acquire(&raw mut PTABLE.lock);
            let ptable = &raw mut PTABLE;

            for p in &mut (*ptable).proc {
                if p.state != ProcState::Runnable {
                    continue;
                }
                // println!("{}: running {}", p.pid, core::str::from_utf8(&p.name).unwrap_or("???"));
                // Switch to chosen process.
                c.proc = p as *mut Proc;
                p.state = ProcState::Running;
                crate::vm::switchuvm(c.proc);
                swtch(&mut c.scheduler as *mut *mut Context, p.context);
                
                // Process is done running for now.
                c.proc = null_mut();
            }
            release(&raw mut PTABLE.lock);
        }
    }
}

fn sched() {
    let c = mycpu();
    if c.proc.is_null() {
        panic!("sched with no process");
    }
    let p = unsafe { &mut *c.proc };

    if p.state == ProcState::Running {
        panic!("sched running");
    }
    if (crate::x86::readeflags() & FL_IF) != 0 {
        panic!("sched interruptible");
    }

    let intena = c.intena != 0;
    unsafe {
        swtch(&mut p.context as *mut *mut Context, c.scheduler);
    }
    c.intena = intena as i32;
}

pub fn r#yield() {
    acquire(unsafe{ &raw mut PTABLE.lock });
    let p = myproc().expect("yield with no process");
    p.state = ProcState::Runnable;
    sched();
    release(unsafe {&raw mut PTABLE.lock });
}

fn forkret() {
    // Release the ptable lock held by scheduler
    release(unsafe {&raw mut PTABLE.lock});
}

pub fn procdump() {
    unsafe {
        acquire(&raw mut PTABLE.lock);
        let ptable = &raw mut PTABLE;
        for p in &(*ptable).proc {
            if p.state == ProcState::Unused {
                continue;
            }
            let state = match p.state {
                ProcState::Unused => "unused",
                ProcState::Embryo => "embryo",
                ProcState::Runnable => "runble",
                ProcState::Running => "run   ",
            };
            let name_len = p.name.iter().position(|&b| b == 0).unwrap_or(p.name.len());
            let name = core::str::from_utf8(&p.name[..name_len]).unwrap_or("???");
            crate::println!("{} {} {}", p.pid, state, name);
        }
        release(&raw mut PTABLE.lock);
    }
}
