use core::ptr::{read_volatile, write_volatile};
use crate::mp::MP_ONCE;

use crate::println;

// I/O APIC default physical address
const IOAPIC_BASE: *mut u32 = 0xFEC00000 as *mut u32;

// Register offsets in 4-byte words
const REG_ID: u32 = 0;        // Register index: ID (0x00 / 4 = 0)
const REG_VER: u32 = 1;       // Register index: version (0x01 / 4 = 1)
const REG_TABLE: u32 = 0x10 / 4;     // Redirection table base (0x10 / 4)

// Redirection table configuration bits
const INT_DISABLED: u32 = 0x00010000;  // Interrupt disabled
// const INT_LEVEL: u32 = 0x00008000;     // Unused in p3 - Level-triggered
// const INT_ACTIVELOW: u32 = 0x00002000; // Unused in p3 - Active low
// const INT_LOGICAL: u32 = 0x00000800;   // Unused in p3 - Destination is CPU ID

const T_IRQ0: u32 = 32;

const ID_REG_OFFSET: isize = 0;      // ID register offset (0x00 / 4)
const DATA_REG_OFFSET: isize = 4;    // Data register offset (0x10 / 4 = 4)

fn ioapic_read(reg: u32) -> u32 {
  unsafe {
      write_volatile(IOAPIC_BASE.offset(ID_REG_OFFSET), reg);
      read_volatile(IOAPIC_BASE.offset(DATA_REG_OFFSET))
  }
}

fn ioapic_write(reg: u32, value: u32) {
  unsafe {
      write_volatile(IOAPIC_BASE.offset(ID_REG_OFFSET), reg);
      write_volatile(IOAPIC_BASE.offset(DATA_REG_OFFSET), value);
  }
}

pub fn ioapic_init() {
    let maxintr = (ioapic_read(REG_VER) >> 16) & 0xFF;
    let id = ioapic_read(REG_ID) >> 24;
    if id != (*MP_ONCE.ioapic_id.get().unwrap() as u32) {
      println!("ioapicinit: id isn't equal to ioapicid; not a MP\n");
    }

    for i in 0..=maxintr {
        ioapic_write(REG_TABLE + 2 * i, INT_DISABLED | (T_IRQ0 + i));
        ioapic_write(REG_TABLE + 2 * i + 1, 0);
    }
}

// Unused in p3 - needed for enabling specific interrupts in p4+
// fn ioapic_enable(irq: u32, cpunum: u32) {
//     ioapic_write(REG_TABLE + 2 * irq, T_IRQ0 + irq);
//     ioapic_write(REG_TABLE + 2 * irq + 1, cpunum << 24);
// }