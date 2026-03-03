#![no_std]
#![no_main]

mod param;
mod x86;
mod uart;
mod console;
mod lapic;
mod ioapic;
mod picirq;
mod mp;
mod proc;

use core::panic::PanicInfo;

use uart::*;
use x86::*;
use lapic::*;
use ioapic::*;
use picirq::*;
use mp::*;

static mut PANICKED: bool = false;

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => ({
        use core::fmt::*;
        use crate::console::Console;
        let mut c = Console {};
        let _ = writeln!(&mut c, $($arg)*);
    });
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts to prevent interrupt handlers from interfering
    cli();
    
    // Print panic message with LAPIC ID to identify which CPU panicked
    println!("lapicid {}:\n{:#?}", lapicid(), info);
    
    // TODO: Add stack trace via getcallerpcs() once spinlock.rs is implemented
    
    // Set panicked flag (freeze other CPUs in full xv6)
    unsafe { PANICKED = true; }
    
    // Halt the system
    loop {}
}

fn halt() -> ! {
    println!("Bye COL{}\n\0", 331);
    loop {
        outw(0x604, 0x2000);
        outw(0xB004, 0x2000); // for older qemu.
    }
}

#[no_mangle]
fn entryofrust() -> ! {
    uartinit();
    println!("Hello from Rust Kernel!");
    mpinit();
    lapicinit();
    println!("lapics initialized. !!");
    picinit();
    println!("pics disabled !!");
    ioapic_init();
    println!("ioapics initialized !!");

    // Test panic handler
    // panic!("Testing enhanced panic handler");

    halt();
}