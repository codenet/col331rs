#![no_std]
#![no_main]
use core::arch::asm;
mod uart;
mod console;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(core::format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}

fn outw(port: u16, data: u16) {
  unsafe {
    asm!("out dx, ax", in("dx") port, in("ax") data);
  }
}

fn halt() -> ! {
  outw(0x604, 0x2000);
  // For older versions of QEMU, 
  outw(0xB004, 0x2000);
  loop {}
}

#[no_mangle]
fn entryofrust() -> ! {
  uart::uartinit();
  println!("uart {}", 123);
  halt();
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}