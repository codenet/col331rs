use modular_bitfield::prelude::*;
use core::cell::OnceCell;
use core::sync::atomic::{AtomicU32, Ordering};
use core::ptr::addr_of_mut;
use crate::proc::{cpuid, myproc, ProcState};
use crate::println;
use crate::lapic::lapiceoi;
use crate::x86::{lidt, rcr2, TrapFrame};
use crate::lapic;
use crate::constants::{DPL_USER, IRQ_COM1, IRQ_SPURIOUS, IRQ_TIMER, T_IRQ0, T_SYSCALL, SEG_KCODE, STS_IG32, STS_TG32};
use crate::uart::uartintr;

extern "C" {
    static vectors: [usize; 256]; // in vectors.S: array of 256 entry pointers
}

#[bitfield]
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct GateDesc {
    off_15_0: B16,   // low 16 bits of offset in segment
    cs: B16,         // code segment selector
    args: B5,        // # args (0 for interrupt/trap gates)
    rsv1: B3,        // reserved bits (should be zero)
    r_type: B4,      // type (e.g., STS_IG32, STS_TG32)
    s: B1,         // must be 0 (system)
    dpl: B2,         // descriptor privilege level
    p: B1,         // present
    off_31_16: B16,  // high bits of offset in segment
}
impl GateDesc {
    pub fn set_gate(&mut self, is_trap: bool, sel: u16, off: usize, dpl: u8) {
        self.set_off_15_0((off & 0xffff) as u16);
        self.set_cs(sel);
        self.set_args(0);
        self.set_rsv1(0);
        let typ = if is_trap { STS_TG32 } else { STS_IG32 };
        self.set_r_type(typ);
        self.set_s(0);
        self.set_dpl(dpl);
        self.set_p(1);
        self.set_off_31_16((off >> 16) as u16);
    }
}


static mut IDT: OnceCell<[GateDesc; 256]> = OnceCell::new();
pub static TICKS: AtomicU32 = AtomicU32::new(0);

pub fn tvinit() {
    let mut arr = [GateDesc::default(); 256];
    for i in 0..256 {
        arr[i].set_gate(
            false,
            SEG_KCODE << 3,
            unsafe { vectors[i] },
            0
        );
    }
    arr[T_SYSCALL as usize].set_gate(
        true,
        SEG_KCODE << 3,
        unsafe { vectors[T_SYSCALL as usize] },
        DPL_USER,
    );
    unsafe {
        let _ = (*addr_of_mut!(IDT)).set(arr);
    }
}

pub fn idtinit() {
    let idt = unsafe { (*addr_of_mut!(IDT)).get().expect("IDT not initialized") };
    lidt(idt, core::mem::size_of::<[GateDesc; 256]>() as usize);
}


#[no_mangle]
pub extern "C" fn trap(orig_tf: *mut TrapFrame) {

    if orig_tf.is_null() {
        panic!("Received null TrapFrame pointer");
    }

    let tf = unsafe { &mut *orig_tf };

    if tf.trapno == T_SYSCALL {
        if let Some(p) = myproc() {
            p.tf = orig_tf;
            crate::syscall::syscall();
            return;
        }
        panic!("syscall with no current process");
    }

	const TIMER: u32 = T_IRQ0 + IRQ_TIMER;
	const SPURIOUS: u32 = T_IRQ0 + IRQ_SPURIOUS;
	const SEVEN: u32 = T_IRQ0 + 7;
    match tf.trapno {
        TIMER => {
            TICKS.fetch_add(1, Ordering::Relaxed);
            lapic::lapiceoi();
        }
        x if x == T_IRQ0 + IRQ_COM1 => {
            uartintr();
            lapiceoi();
        }
        SEVEN | SPURIOUS => {
            println!(
                "cpu{}: spurious interrupt at {:x}:{:x}\n",
                cpuid(),
                tf.cs,
                tf.eip
            );
            lapiceoi();
		}
        crate::constants::IDE_TRAP => {
            crate::ide::ideintr();
            crate::lapic::lapiceoi();
        }

		_ => {
            if myproc().is_none() || (tf.cs & 3) == 0 {
                println!(
                    "unexpected trap {} from cpu {} eip {} (cr2=0x{:x})\n",
                    tf.trapno,
                    cpuid(),
                    tf.eip,
                    rcr2()
                );
                panic!("trap happened");
            }
		}
	}

    if tf.trapno == TIMER {
        if let Some(p) = myproc() {
            if p.state == ProcState::Running {
                crate::proc::r#yield();
            }
        }
    }
}
