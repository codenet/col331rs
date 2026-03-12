// <port, value>
use core::arch::asm;
pub unsafe fn inb(port: u16) -> u8 {
    let result: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") result,
        options(nomem, nostack)
    );

    result
}
pub unsafe fn outb(port: u16 , value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack)
    );
}
pub unsafe fn outw(port: u16 , value: u16) {
    asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack)
    );
}
