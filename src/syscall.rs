use crate::constants::{SYS_CLOSE, SYS_OPEN, SYS_WRITE, SYS_EXEC};
use crate::proc::myproc;

pub fn fetchint(addr: u32, ip: &mut i32) -> i32 {
    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    let end = match addr.checked_add(4) {
        Some(v) => v,
        None => return -1,
    };

    if addr >= curproc.sz || end > curproc.sz {
        return -1;
    }

    unsafe {
        // offset is a base pointer to the mapped user memory
        let ptr = curproc.offset.add(addr as usize) as *const i32;
        *ip = core::ptr::read_unaligned(ptr);
    }

    0
}

pub fn fetchstr(addr: u32, pp: &mut *const u8) -> i32 {
    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    if addr >= curproc.sz {
        return -1;
    }

    unsafe {
        let start = curproc.offset.add(addr as usize) as *const u8;
        let ep = curproc.offset.add(curproc.sz as usize) as *const u8;
        let mut s = start;

        while s < ep {
            if *s == 0 {
                *pp = start;
                return s.offset_from(start) as i32;
            }
            s = s.add(1);
        }
    }

    -1
}

pub fn argint(n: i32, ip: &mut i32) -> i32 {
    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    if curproc.tf.is_null() {
        return -1;
    }

    let esp = unsafe { (*curproc.tf).esp };

    // Compute address of nth argument on user stack
    let addr = match esp
        .checked_add(4)                       // skip saved PC
        .and_then(|v| v.checked_add((n as u32) * 4))
    {
        Some(v) => v,
        None => return -1,
    };

    fetchint(addr, ip)
}

pub fn argptr(n: i32, pp: &mut *const u8, size: i32) -> i32 {
    let mut i = 0;
    let curproc = match myproc() {
        Some(p) => p,
        None => return -1,
    };

    if argint(n, &mut i) < 0 {
        return -1;
    }

    if size < 0 {
        return -1;
    }

    let base = i as u32;
    let end = match base.checked_add(size as u32) {
        Some(v) => v,
        None => return -1,
    };
    if base >= curproc.sz || end > curproc.sz {
        return -1;
    }

    
    *pp = base as *const u8;

    0
}

pub fn argstr(n: i32, pp: &mut *const u8) -> i32 {
    let mut addr = 0;
    if argint(n, &mut addr) < 0 {
        return -1;
    }
    fetchstr(addr as u32, pp)
}



pub fn syscall() {
    let curproc = match myproc() {
        Some(p) => p,
        None => return,
    };
    if curproc.tf.is_null() {
        return;
    }

    let num = unsafe { (*curproc.tf).eax as usize };
    let ret = match num {
        SYS_OPEN =>  crate::sysfile::sys_open(),
        SYS_WRITE => crate::sysfile::sys_write(),
        SYS_CLOSE => crate::sysfile::sys_close(),
        SYS_EXEC =>  crate::sysfile::sys_exec(),
        _ => {
            let name_len = curproc
                .name
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(curproc.name.len());
            let pname = core::str::from_utf8(&curproc.name[..name_len]).unwrap_or("???");
            crate::println!("{} {}: unknown sys call {}", curproc.pid, pname, num);
            -1
        }
    };

    unsafe {
        (*curproc.tf).eax = ret as u32;
    }
}
