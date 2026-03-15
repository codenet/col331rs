#![no_std]

pub mod printf;
pub mod user;
pub mod init;

pub use init::main;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}