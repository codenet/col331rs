#![no_std]
#![no_main]

mod x86;
mod uart;
mod console;

use core::panic::PanicInfo;

use uart::*;
use x86::*;
use console::*;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Kernel Panic: {:?}", info);
    loop {}
}

unsafe fn halt() -> ! {
    println!("Bye COL{}\n\0", 331);
    loop {
        outw(0x604, 0x2000);
        // For older versions of QEMU, 
        outw(0xB004, 0x2000);
    }
}

#[no_mangle]
fn entryofrust() -> ! {
    uartinit();
    println!("Hello from Rust Kernel!");
    unsafe { halt(); }
}