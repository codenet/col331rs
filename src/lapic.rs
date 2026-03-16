use core::ptr::{read_volatile, write_volatile};
use crate::mp::MP_ONCE;
use crate::constants::{IRQ_ERROR, IRQ_SPURIOUS, IRQ_TIMER, T_IRQ0};

const ID: isize = 0x0020 / 4;
const VER: isize = 0x0030 / 4;
const TPR: isize = 0x0080 / 4;
const EOI: isize = 0x00B0 / 4;
const SVR: isize = 0x00F0 / 4;

const ENABLE: u32 = 0x00000100;

const ESR: isize = 0x0280 / 4;
const ICRLO: isize = 0x0300 / 4;

const INIT: u32 = 0x00000500;
// const STARTUP: u32 = 0x00000600;  // Unused in p3 - for AP startup
const DELIVS: u32 = 0x00001000;
// const ASSERT: u32 = 0x00004000;    // Unused in p3 - for IPI
// const DEASSERT: u32 = 0x00000000;  // Unused in p3 - for IPI
const LEVEL: u32 = 0x00008000;
const BCAST: u32 = 0x00080000;
// const BUSY: u32 = 0x00001000;      // Unused in p3
// const FIXED: u32 = 0x00000000;     // Unused in p3

const ICRHI: isize = 0x0310 / 4;
const TIMER: isize = 0x0320 / 4;
const X1: u32 = 0x0000000B;
const PERIODIC: u32 = 0x00020000;

const PCINT: isize = 0x0340 / 4;
const LINT0: isize = 0x0350 / 4;
const LINT1: isize = 0x0360 / 4;
const ERROR: isize = 0x0370 / 4;
const MASKED: u32 = 0x00010000;

const TICR: isize = 0x0380 / 4;
// const TCCR: isize = 0x0390 / 4;  // Unused in p3 - timer current count
const TDCR: isize = 0x03E0 / 4;

// Volatile write to LAPIC
fn lapicw(index: isize, value: u32) {
	// lapic_base is a global address so multiple threads can write to it. 
	// most likely multiple cpus can't as they will write to their own lapic.
	// but we dont have AP processors running nor we have multiple threads. 
	let b = MP_ONCE.lapic_base.get().unwrap();
	unsafe {
		// unsafe because write_volatile itself is unsafe. 
		write_volatile(b.offset(index), value);
	}
}

fn lapic_read(offset: isize) -> u32 {
	let b = MP_ONCE.lapic_base.get().unwrap();
	unsafe { read_volatile(b.offset(offset)) }
}

// Get LAPIC ID
pub fn lapicid() -> u32 {
	lapic_read(ID) >> 24
}

pub fn lapicinit() {
	// Enable local APIC; set spurious interrupt vector.
	lapicw(SVR, ENABLE | (T_IRQ0 + IRQ_SPURIOUS));

	// The timer repeatedly counts down at bus frequency
	// from lapic[TICR] and then issues an interrupt.
	// If xv6 cared more about precise timekeeping,
	// TICR would be calibrated using an external time source.
	lapicw(TDCR, X1);
	lapicw(TIMER, PERIODIC | (T_IRQ0 + IRQ_TIMER));
	lapicw(TICR, 10000000);


	// Disable logical interrupt lines.
	lapicw(LINT0, MASKED);
	lapicw(LINT1, MASKED);


	// Disable performance counter overflow interrupts
	// on machines that provide that interrupt entry.
	if (lapic_read(VER) >> 16) & 0xFF >= 4 {
		lapicw(PCINT, MASKED);
	}

	// Map error interrupt to IRQ_ERROR.
	lapicw(ERROR, T_IRQ0 + IRQ_ERROR);


	// Clear error status register (requires back-to-back writes).
	lapicw(ESR, 0);
	lapicw(ESR, 0);

	// Ack any outstanding interrupts.
	lapicw(EOI, 0);


	// Send an Init Level De-Assert to synchronize arbitration IDs
	lapicw(ICRHI, 0);
	lapicw(ICRLO, BCAST | INIT | LEVEL);
	while lapic_read(ICRLO) & DELIVS != 0 {}

	// Enable interrupts on the APIC (but not on the processor).
	lapicw(TPR, 0);
}


// Acknowledge interrupt
pub fn lapiceoi() {
	lapicw(EOI, 0);
}

// Spin for a given number of microseconds.
// On real hardware would want to tune this dynamically.
pub fn microdelay(_us: u32) {
	// For real hardware, you would need to implement a proper delay
}
