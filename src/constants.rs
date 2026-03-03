// ------------------------------------ MEMORY RELATED ----------------------------------------
// Eflags register
pub const FL_IF: u32 = 0x00000200; // Interrupt Enable

// Control Register flags
pub const CR0_PE: u32 = 0x00000001; // Protection Enable

// Various segment selectors.
pub const SEG_KCODE: u16 = 1; // kernel code
pub const SEG_KDATA: u16 = 2; // kernel data+stack
pub const SEG_UCODE: u16 = 3; // user code
pub const SEG_UDATA: u16 = 4; // user data+stack
pub const SEG_TSS: u16 = 5;   // this process's task state

// System segment type bits
pub const STS_T32A: u8 = 0x9; // Available 32-bit TSS
pub const STS_IG32: u8 = 0xE; // 32-bit Interrupt Gate
pub const STS_TG32: u8 = 0xF; // 32-bit Trap Gate


// ----------------------------------------------------- CPU RELATED ----------------------------------------------
pub const KSTACKSIZE: usize = 4096; // size of per-process kernel stack
pub const NCPU: usize = 8; // maximum number of CPUs

// ----------------------------------------------- TRAPS -------------------------------------------------------
// x86 trap and interrupt constants

// Processor-defined
pub const T_DIVIDE: u32 = 0;      // divide error
pub const T_DEBUG: u32 = 1;       // debug exception
pub const T_NMI: u32 = 2;         // non-maskable interrupt
pub const T_BRKPT: u32 = 3;       // breakpoint
pub const T_OFLOW: u32 = 4;       // overflow
pub const T_BOUND: u32 = 5;       // bounds check
pub const T_ILLOP: u32 = 6;       // illegal opcode
pub const T_DEVICE: u32 = 7;      // device not available
pub const T_DBLFLT: u32 = 8;      // double fault
// pub const T_COPROC: u32 = 9;   // reserved (not used since 486)
pub const T_TSS: u32 = 10;        // invalid task switch segment
pub const T_SEGNP: u32 = 11;      // segment not present
pub const T_STACK: u32 = 12;      // stack exception
pub const T_GPFLT: u32 = 13;      // general protection fault
pub const T_PGFLT: u32 = 14;      // page fault
// pub const T_RES: u32 = 15;    // reserved
pub const T_FPERR: u32 = 16;      // floating point error
pub const T_ALIGN: u32 = 17;      // alignment check
pub const T_MCHK: u32 = 18;       // machine check
pub const T_SIMDERR: u32 = 19;    // SIMD floating point error

// Arbitrarily chosen, but with care not to overlap
// processor defined exceptions or interrupt vectors
pub const T_SYSCALL: u32 = 64;    // system call
pub const T_DEFAULT: u32 = 500;   // catchall
pub const T_IRQ0: u32 = 32;
pub const IRQ_TIMER: u32 = 0;
pub const IRQ_KBD: u32 = 1;
pub const IRQ_COM1: u32 = 4;
pub const IRQ_IDE: u32 = 14;
pub const IRQ_ERROR: u32 = 19;
pub const IRQ_SPURIOUS: u32 = 31;
pub const IDE_TRAP: u32 = T_IRQ0 + IRQ_IDE;
// ------------------------------------------------------ MP RELATED  -------------------------------------------------------

// Processor flags
pub const MPBOOT: u8 = 0x02;     // This proc is the bootstrap processor

// Table entry types
pub const MPPROC: u8 = 0x00;     // One per processor
pub const MPBUS: u8 = 0x01;      // One per bus
pub const MPIOAPIC: u8 = 0x02;   // One per I/O APIC
pub const MPIOINTR: u8 = 0x03;   // One per bus interrupt source
pub const MPLINTR: u8 = 0x04;    // One per system interrupt source




// ------------------------------------------------------ LAPIC RELATED ------------------------------- 

// Local APIC registers, divided by 4 for use as u32[] indices
pub const ID: usize = 0x0020/4;    // ID
pub const VER: usize = 0x0030/4;   // Version
pub const TPR: usize = 0x0080/4;   // Task Priority
pub const EOI: usize = 0x00B0/4;   // EOI
pub const SVR: usize = 0x00F0/4;   // Spurious Interrupt Vector
pub const ESR: usize = 0x0280/4;   // Error Status
pub const ICRLO: usize = 0x0300/4; // Interrupt Command
pub const ICRHI: usize = 0x0310/4; // Interrupt Command [63:32]
pub const TIMER: usize = 0x0320/4; // Local Vector Table 0 (TIMER)
pub const PCINT: usize = 0x0340/4; // Performance Counter LVT
pub const LINT0: usize = 0x0350/4; // Local Vector Table 1 (LINT0)
pub const LINT1: usize = 0x0360/4; // Local Vector Table 2 (LINT1)
pub const ERROR: usize = 0x0370/4; // Local Vector Table 3 (ERROR)
pub const TICR: usize = 0x0380/4;  // Timer Initial Count
pub const TCCR: usize = 0x0390/4;  // Timer Current Count
pub const TDCR: usize = 0x03E0/4;  // Timer Divide Configuration

// SVR and other control bits
pub const ENABLE: u32 = 0x00000100;   // Unit Enable

// ICRLO control bits
pub const INIT: u32 = 0x00000500;     // INIT/RESET
pub const STARTUP: u32 = 0x00000600;  // Startup IPI
pub const DELIVS: u32 = 0x00001000;   // Delivery status
pub const ASSERT: u32 = 0x00004000;   // Assert interrupt (vs deassert)
pub const DEASSERT: u32 = 0x00000000;
pub const LEVEL: u32 = 0x00008000;    // Level triggered
pub const BCAST: u32 = 0x00080000;    // Send to all APICs, including self
pub const BUSY: u32 = 0x00001000;
pub const FIXED: u32 = 0x00000000;

// Timer configuration
pub const X1: u32 = 0x0000000B;       // divide counts by 1
pub const PERIODIC: u32 = 0x00020000;  // Periodic

// Error handling
pub const MASKED: u32 = 0x00010000;   // Interrupt masked