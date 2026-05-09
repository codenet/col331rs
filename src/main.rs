#![no_std]       // No standard library
#![no_main]      // No main function
#![allow(dead_code)]

use core::panic::PanicInfo;
use crate::x86::cli;
use crate::lapic::lapicid;

mod param;
mod x86;
mod uart;
mod console;
mod lapic;
mod ioapic;
mod picirq;
mod mp;
mod proc;
mod traps;
mod fs_h;
mod constants;  // Internal use only - no external crates
mod buf;
mod bio;
mod ide;
mod fs;
mod fcntl;
mod file;
mod log;
mod syscall;
mod sysfile;
mod mmu;
mod vm;
mod spinlock;
mod kalloc;
use crate::traps::*;
use crate::constants::PHYSTOP;

fn halt() -> ! {
    println!("Bye COL{}\n\0", 331);
    loop {
        x86::outw(0x602, 0x2000);
        x86::outw(0xB002, 0x2000);
    }
}
extern "C" {
    pub fn alltraps();
    static end: u8;
}

#[no_mangle]
pub extern "C" fn entryofrust() -> ! {
    let kernel_end = unsafe { &end as *const u8 as *mut u8 };
    kalloc::kinit(kernel_end, PHYSTOP as *mut u8);
    mp::mpinit();
    lapic::lapicinit();
    picirq::picinit();
    ioapic::ioapic_init();
    console::consoleinit();
    uart::uartinit();
    ide::ideinit();
    tvinit();
    bio::binit();
    idtinit();
    x86::sti();
    fs::iinit(param::ROOTDEV);
    log::initlog(param::ROOTDEV);
    file::mknod("/console", param::CONSOLE as i16, param::CONSOLE as i16);
    vm::seginit();       // segment descriptors
    proc::pinit();       // first process
    proc::pinit();       // another process
    proc::scheduler();   // start running processes (never returns)
}

static mut PANICKED: bool = false;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Kernel Panic: {:?}", info);
    use core::fmt::Write;
    
    // Disable interrupts to prevent interrupt handlers from interfering
    cli();
    
    // Print panic message with LAPIC ID to identify which CPU panicked
    let mut console = console::Console {};
    let _ = write!(&mut console, "lapicid {}: panic: ", lapicid());
    let _ = writeln!(&mut console, "{}", info);
    
    // Print stack trace
    let mut pcs = [0u32; 10];
    let stack_ptr = &info as *const _ as *const u32;
    spinlock::getcallerpcs(stack_ptr, &mut pcs);
    
    for &pc in &pcs {
        if pc != 0 {
            let _ = writeln!(&mut console, " {:#x}", pc);
        }
    }
    
    unsafe { PANICKED = true; }
    
    // Halt the system
    halt();
}
