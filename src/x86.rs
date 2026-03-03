use core::arch::asm;
use crate::traps::GateDesc;

pub fn inb(port: u16) -> u8 {
    let result: u8;
    unsafe { 
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") result,
            options(nomem, nostack)
        );
        result    
    }
}

pub fn outb(port: u16, value: u8) {
    unsafe { 
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack)
        );    
    }
}

pub fn outw(port: u16, value: u16) {
    unsafe { 
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack)
        );    
    }
}

pub fn readeflags() -> u32 {
    unsafe {
        let eflags: u32;
        asm!(
            "pushfd; pop eax",
            out("eax") eflags,
            options(nomem, nostack)
        );
        eflags
    }
}

pub fn cli () {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

pub fn sti () {
    unsafe{
        asm!("sti", options(nomem, nostack));
    }
}

pub fn lidt(gdt: *const [GateDesc; 256], size: usize) {
    let pd: [u16; 3] = [
        (size - 1) as u16,
        (gdt as *const _) as u16,
        ((gdt as *const _ as u32) >> 16) as u16,
    ];
    unsafe { 
        asm!(
            "lidt [{0:e}]",
            in(reg) (&pd as *const _ ) as u32,
            options(nostack, readonly)
        );
    }
}

pub fn noop() {
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

// x86.rs
pub unsafe fn insl(port: u16, addr: *mut u32, cnt: usize) {
    core::arch::asm!(
        "cld",
        "rep insd",
        in("dx") port,
        inout("edi") (addr as usize) => _,
        inout("ecx") cnt => _,
        options(nostack, preserves_flags),
    );
}


// Output string of dwords to port using rep outsd instruction.
// Matches the C implementation: outsl(port, addr, cnt)
// Note: ESI must be saved/restored as LLVM restricts its use in 32-bit mode
pub unsafe fn outsl(port: u16, addr: *const u32, cnt: usize) {
    let addr_val = addr as u32;
    core::arch::asm!(
        "push esi",
        "mov esi, {addr}",
        "cld",
        "rep outsd",
        "pop esi",
        addr = in(reg) addr_val,
        in("dx") port,
        inout("ecx") cnt => _,
    );
}

/// Halts the CPU until the next interrupt occurs.
/// The `hlt` instruction:
/// 1. Stops instruction execution and places the processor in a HALT state
/// 2. Reduces power consumption by the processor
/// 3. Execution resumes only when an enabled interrupt or RESET occurs
/// Note: Interrupts must be enabled (via `sti`) for `hlt` to resume execution on interrupt
pub fn wfi() {
    unsafe { 
        asm!("hlt", options(nomem, nostack));
    }
}

pub fn rcr2() -> u32 {
    unsafe { 
        let val: u32;
        asm!(
            "mov {0:e}, cr2",
            out(reg) val,
            options(nomem, nostack)
        );    
        val
    }
}

#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pusha
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub oesp: u32, // useless & ignored
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,

    pub trapno: u32,

    // below here defined by x86 hardware
    pub err: u32,
    pub eip: u32,
    pub cs: u16,
    pub padding5: u16,
    pub eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    pub esp: u32,
    pub ss: u16,
    pub padding6: u16,
}