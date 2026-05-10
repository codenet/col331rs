use core::{slice, mem};
use crate::param::NCPU;
use crate::x86::{outb, inb};
use crate::proc::Cpu;
use core::cell::OnceCell;

pub static mut IOAPICID: u8 = 0;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Mp {
    pub signature: [u8; 4],      // "_MP_"
    pub physaddr: *const MpConf, // phys addr of MP config table
    pub length: u8,              // 1
    pub specrev: u8,             // [14]
    pub checksum: u8,            // all bytes must add up to 0
    pub type_: u8,              // MP system config type
    pub imcrp: u8,
    pub reserved: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MpConf {
    pub signature: [u8; 4],      // "PCMP"
    pub length: u16,             // total table length
    pub version: u8,             // [14]
    pub checksum: u8,            // all bytes must add up to 0
    pub product: [u8; 20],       // product id
    pub oemtable: *const u32,    // OEM table pointer
    pub oemlength: u16,          // OEM table length
    pub entry: u16,              // entry count
    pub lapicaddr: *const u32,   // address of local APIC
    pub xlength: u16,            // extended table length
    pub xchecksum: u8,          // extended table checksum
    pub reserved: u8,
}

// #[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MpProc {
    pub type_: u8,               // entry type (0)
    pub apicid: u8,              // local APIC id
    pub version: u8,             // local APIC version
    pub flags: u8,               // CPU flags
    pub signature: [u8; 4],      // CPU signature
    pub feature: u32,            // feature flags from CPUID instruction
    pub reserved: [u8; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MpIoApic {
    pub type_: u8,               // entry type (2)
    pub apicno: u8,              // I/O APIC id
    pub version: u8,             // I/O APIC version
    pub flags: u8,               // I/O APIC flags
    pub addr: *const u32,        // I/O APIC address
}

// Processor flags
pub const MPBOOT: u8 = 0x02;     // This proc is the bootstrap processor

// Table entry types
pub const MPPROC: u8 = 0x00;     // One per processor
pub const MPBUS: u8 = 0x01;      // One per bus
pub const MPIOAPIC: u8 = 0x02;   // One per I/O APIC
pub const MPIOINTR: u8 = 0x03;   // One per bus interrupt source
pub const MPLINTR: u8 = 0x04;    // One per system interrupt source

pub struct MPWriteOnce {
  pub lapic_base: OnceCell<*mut u32>,
  pub ioapic_id: OnceCell<u8>,
  pub cpus: OnceCell<[Cpu; NCPU]>
}

unsafe impl Sync for MPWriteOnce {}

pub static MP_ONCE: MPWriteOnce = MPWriteOnce { lapic_base: OnceCell::new(), ioapic_id: OnceCell::new(), cpus: OnceCell::new() };

/// Calculates sum of bytes in a memory region
/// 
/// # Safety
/// 
/// The caller must ensure that:
/// - `addr` points to valid memory
/// - The memory region from `addr` to `addr + len` is readable
/// - `addr` is properly aligned
fn sum(addr: *const u8, len: i32) -> u8 {
  let slice = unsafe {
    slice::from_raw_parts(addr, len as usize)
  };
  slice.iter().fold(0u8, |acc, &x| acc.wrapping_add(x))
}

/// Searches for MP floating pointer structure in a memory region
/// 
/// # Safety
///
/// The caller must ensure that:
/// - `a` points to valid memory
/// - The memory region from `a` to `a + len` is readable
/// - Memory accesses are properly aligned
fn mpsearch1(a: u32, len: i32) -> Option<*mut Mp> {
    let addr = a as *const u8;
    let mut p = addr;

    unsafe {
      let e = addr.add(len as usize);
      while p < e {
          // Check for "_MP_" signature and valid checksum
          if slice::from_raw_parts(p, 4) == b"_MP_" && 
            sum(p, mem::size_of::<Mp>() as i32) == 0 {
              return Some(p as *mut Mp);
          }
          p = p.add(mem::size_of::<Mp>());
      }
    }
    None
}

/// Search for the MP Floating Pointer Structure in three locations:
/// 1) First KB of the EBDA
/// 2) Last KB of system base memory
/// 3) BIOS ROM between 0xE0000 and 0xFFFFF
/// 
/// # Safety
///
/// This function accesses raw memory locations and should only be called
/// during system initialization
fn mpsearch() -> Option<*mut Mp> {
  let bda = 0x400 as *const u8;
  let mp: Option<*mut Mp>;
  
  let p = unsafe {(((*bda.add(0x0F) as u32) << 8) | (*bda.add(0x0E) as u32)) << 4};
  if p != 0 {
      mp = mpsearch1(p, 1024);
      if mp.is_some() {
          return mp;
      }
  } else {
      let p = unsafe {(((*bda.add(0x14) as u32) << 8) | (*bda.add(0x13) as u32)) * 1024};
      mp = mpsearch1(p - 1024, 1024);
      if mp.is_some() {
          return mp;
      }
  }
  mpsearch1(0xF0000, 0x10000)
}

/// Locate and validate MP configuration table
/// 
/// # Safety
///
/// This function accesses raw memory locations and should only be called
/// durnull_muting system initialization
fn mpconfig() -> Option<Mp> {
  let mp = mpsearch()?;

  unsafe {
    if (*mp).physaddr.is_null() {
      return None;
    } 
  }

  let conf = unsafe { (*mp).physaddr as *mut MpConf };
  
  unsafe {
    // Check signature "PCMP"
    if slice::from_raw_parts(conf as *const u8, 4) != b"PCMP" {
        return None;
    }

    // Check version
    if (*conf).version != 1 && (*conf).version != 4 {
        return None;
    }

    // Verify checksum
    if sum(conf as *const u8, (*conf).length as i32) != 0 {
        return None;
    }
  }
  Some(unsafe { *mp })
}

/// Initialize multiprocessor configuration
/// 
/// # Safety
///
/// This function must only be called once during system initialization
pub fn mpinit() {
  let mp= mpconfig().unwrap();
  let conf = unsafe { *(mp.physaddr as *mut MpConf) };

  let mut ismp = true;
  MP_ONCE.lapic_base.set(conf.lapicaddr as *mut u32);

  let mut p = (mp.physaddr as usize + mem::size_of::<MpConf>()) as *const u8;
  let e = (mp.physaddr as usize + conf.length as usize) as *const u8;
  
  let mut ncpu = 0;
  let mut cpus = [Cpu::new(); NCPU];
  
  while p < e {
    match unsafe { *p } {
      MPPROC => {
        let proc = p as *const MpProc;
        if ncpu < NCPU {
          // SAFETY: We're the only thread modifying CPUS during initialization
          cpus[ncpu].apicid = unsafe { (*proc).apicid };
          ncpu += 1;
        }
        p = p.wrapping_add(mem::size_of::<MpProc>());
      }
      MPIOAPIC => {
        let ioapic = p as *const MpIoApic;
        let ioapicid = unsafe { (*ioapic).apicno };
        MP_ONCE.ioapic_id.set(ioapicid);
        p = p.wrapping_add(mem::size_of::<MpIoApic>());
      }
      MPBUS | MPIOINTR | MPLINTR => {
        p = p.wrapping_add(8);
      }
      _ => {
        ismp = false;
        break;
      }
    }
  }

  if !ismp {
      panic!("Didn't find a suitable machine");
  }
  MP_ONCE.cpus.set(cpus);

  if mp.imcrp != 0 {
    // Bochs doesn't support IMCR, so this doesn't run on Bochs.
    // But it would on real hardware.
    outb(0x22, 0x70);   // Select IMCR
    outb(0x23, inb(0x23) | 1);  // Mask external interrupts.
  }
}