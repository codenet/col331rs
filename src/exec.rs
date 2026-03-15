use crate::constants::*;
use crate::fs;
use crate::kalloc;
use crate::log;
use crate::proc;
use crate::vm;
use crate::param::MAXARG;
use crate::println;
use core::mem::size_of;
use core::ptr;

#[repr(C)]
#[derive(Default)]
pub struct ElfHdr {
    pub magic:     u32,
    pub elf:      [u8; 12],
    pub type_:     u16,
    pub machine:   u16,
    pub version:   u32,
    pub entry:     u32,
    pub phoff:     u32,
    pub shoff:     u32,
    pub flags:     u32,
    pub ehsize:    u16,
    pub phentsize: u16,
    pub phnum:     u16,
    pub shentsize: u16,
    pub shnum:     u16,
    pub shstrndx:  u16,
}

#[repr(C)]
#[derive(Default)]
pub struct ProgHdr {
    pub type_:  u32,
    pub off:    u32,
    pub vaddr:  u32,
    pub paddr:  u32,
    pub filesz: u32,
    pub memsz:  u32,
    pub flags:  u32,
    pub align:  u32,
}

pub const ELF_PROG_LOAD: u32 = 1;

pub fn exec(path: &[u8], argv: &[*const u8]) -> i32 {
    let mut elf: ElfHdr = Default::default();
    let mut ph: ProgHdr = Default::default();
    let offset;
    let mut usp: u32;
    let mut ustack = [0u32; 3 * MAXARG + 1];

    // Prepare new address space
    // let pg = kalloc::kalloc();
    offset = kalloc::kalloc();

    if offset == core::ptr::null_mut() {
        return -1;
    }
    // Zeroing the Page
    unsafe {
        ptr::write_bytes(offset, 0, PGSIZE as usize);
    }

    log::begin_op();
    
    let str_path = core::str::from_utf8(path).unwrap_or("<invalid utf-8>");
    let ip_opt = fs::namei(str_path);
    
    if ip_opt.is_none() {
        log::end_op();
        println!("exec: fail");
        kalloc::kfree(offset);
        return -1;
    }

    let ip_idx = ip_opt.unwrap();
    fs::iread(ip_idx);

    // Avoid GOTO, Helper to cleanup and return
    let bad = || -> i32 {
        // if(ip_idx != 0) {
        fs::iput(ip_idx);
        log::end_op();
        // }
        kalloc::kfree(offset);
        -1
    };

    // Check ELF header
    let elf_slice = unsafe {
        core::slice::from_raw_parts_mut(&mut elf as *mut _ as *mut u8, size_of::<ElfHdr>())
    };
    if fs::readi(ip_idx, elf_slice, 0, size_of::<ElfHdr>() as u32) as usize != size_of::<ElfHdr>() {
        return bad();
    }
    if elf.magic != ELF_MAGIC {
        return bad();
    }

    // Load program into memory
    let mut off = elf.phoff;
    for _ in 0..elf.phnum {
        let ph_slice = unsafe {
            core::slice::from_raw_parts_mut(&mut ph as *mut _ as *mut u8, size_of::<ProgHdr>())
        };
        if fs::readi(ip_idx, ph_slice, off, size_of::<ProgHdr>() as u32) as usize != size_of::<ProgHdr>() {
            return bad();
        }
        off += size_of::<ProgHdr>() as u32;

        if ph.type_ != ELF_PROG_LOAD {
            continue;
        }
        if ph.memsz < ph.filesz {
            return bad();
        }
        if ph.vaddr + ph.memsz < ph.vaddr {
            return bad();
        }
        if ph.vaddr % PGSIZE != 0 {
            return bad();
        }
        if ph.vaddr + ph.filesz > PGSIZE {
            return bad();
        }
        
        let dest_slice = unsafe {
            core::slice::from_raw_parts_mut(offset.add(ph.vaddr as usize), ph.filesz as usize)
        };
        if fs::readi(ip_idx, dest_slice, ph.off, ph.filesz) as usize != ph.filesz as usize {
            return bad();
        }
    }
    fs::iput(ip_idx);
    log::end_op();

    let last = path
                        .rsplit(|&b| b == b'/')
                        .find(|s| !s.is_empty())
                        .unwrap_or(path);
    
    let mut name_idx = 0;

    let proc = match proc::myproc() {
        Some(p) => p,
        None => panic!("exec: no current process"),
    };

      
    for &b in last {
        if b == 0 || name_idx >= proc.name.len() - 1 {
            break;
        }
        proc.name[name_idx] = b;
        name_idx += 1;
    }
    proc.name[name_idx] = 0;

    // Push argument strings, prepare rest of stack in ustack
    usp = PGSIZE;
    let mut argc = 0;
    while argc < argv.len() && !argv[argc].is_null() {
        if argc >= MAXARG {
            kalloc::kfree(offset);
            return -1;
        }
        // find length of string
        let mut len = 0;
        unsafe {
            let p = argv[argc];
            while *p.add(len) != 0 {
                len += 1;
            }
        }
        len += 1; // for null terminator

        usp = usp - len as u32;
        unsafe {
            ptr::copy_nonoverlapping(argv[argc], offset.add(usp as usize), len);
        }
        ustack[3 + argc] = usp;
        argc += 1;
    }
    ustack[3 + argc] = 0;

    ustack[0] = 0xffffffff; // fake return PC
    ustack[1] = argc as u32;
    ustack[2] = usp - ((argc as u32 + 1) * 4); // argv pointer
    
    usp -= (3 + argc as u32 + 1) * 4;
    unsafe {
        ptr::copy_nonoverlapping(
            ustack.as_ptr() as *const u8,
            offset.add(usp as usize),
            ((3 + argc + 1) * 4) as usize,
        );
    }

    unsafe {
        let tf = &mut *proc.tf;
        tf.eip = elf.entry; // main
        tf.esp = usp;
    }

    // Free the old address space
    kalloc::kfree(proc.offset as *mut u8);
    proc.offset = offset;
    vm::switchuvm(proc);
    
    0
}