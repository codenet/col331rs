use crate::x86::{outb, inb};
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3F8; // COM1 port address

static UART: AtomicBool = AtomicBool::new(false);

pub fn uartinit() {
  // Turn off the FIFO
  outb(COM1+2, 0);

  // 9600 baud, 8 data bits, 1 stop bit, parity off.
  outb(COM1+3, 0x80);    // Unlock divisor
  let b = 115200/9600;
  outb(COM1+0, b as u8);
  outb(COM1+1, 0);
  outb(COM1+3, 0x03);    // Lock divisor, 8 data bits.
  outb(COM1+4, 0);
  outb(COM1+1, 0x01);    // Enable receive interrupts.

  // If status is 0xFF, no serial port.
  if inb(COM1+5) == 0xFF {
    return;
  }
    UART.store(true, Ordering::Relaxed);

  // Announce that we're here.
  for p in "xv6...\n".chars() {
    uartputc(p);
  }
}

pub fn uartputc(c: char) {
    if !UART.load(Ordering::Relaxed) {
        return;
    }
  for _ in 0..128  {
    if inb(COM1+5) & 0x20 != 0 {
      break;
    }
  }
  outb(COM1+0, c as u8);
}