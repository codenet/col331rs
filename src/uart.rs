use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3f8;

static UART_PRESENT: AtomicBool = AtomicBool::new(false);

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al",
        in("dx") port,
        in("al") val,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let ret: u8;
    asm!("in al, dx",
        in("dx") port,
        out("al") ret,
        options(nomem, nostack, preserves_flags)
    );
    ret
}

pub fn uartinit() {
    unsafe {
        // Turn off the FIFO
        outb(COM1 + 2, 0);

        // 9600 baud, 8 data bits, 1 stop bit, parity off.
        outb(COM1 + 3, 0x80); // Unlock divisor (DLAB=1)

        // 115200/9600 = 12
        outb(COM1 + 0, (115200u32 / 9600u32) as u8);
        outb(COM1 + 1, 0);

        outb(COM1 + 3, 0x03); // Lock divisor (DLAB=0), 8 data bits.
        outb(COM1 + 4, 0);

        outb(COM1 + 1, 0x01); // Enable receive interrupts.

        // If status is 0xFF, no serial port.
        if inb(COM1 + 5) == 0xFF {
            return;
        }
    }

    UART_PRESENT.store(true, Ordering::Release);

    // Announce that we're here.
    for &b in b"xv6...\n" {
        uartputc(b as i32);
    }
}

pub fn uartputc(c: i32) {
    if !UART_PRESENT.load(Ordering::Acquire) {
        return;
    }

    unsafe {
        // Wait for Transmit Holding Register empty (LSR bit 5 == 0x20),
        // but only spin up to 128 iterations like xv6.
        let mut i = 0;
        while i < 128 && (inb(COM1 + 5) & 0x20) == 0 {
            i += 1;
        }

        outb(COM1 + 0, (c & 0xFF) as u8);
    }
}
