use crate::constants::{SEG_KCODE, SEG_KDATA, SEG_UCODE, SEG_UDATA, SEG_TSS, STA_X, STA_W, STA_R, PROCSIZE, DPL_USER, STS_T32A};
use crate::mmu::SegDesc;
use crate::proc::{cpuid, mycpu, Proc};
use crate::mp::MP_ONCE;
use crate::x86::{lgdt, ltr};
use core::mem::{size_of, size_of_val};
use crate::param::KSTACKSIZE;
use crate::spinlock::{pushcli, popcli};

/// Set up CPU's kernel segment descriptors.
/// Run once on entry on each CPU.
pub fn seginit() {
    unsafe {
        // Map "logical" addresses to virtual addresses using identity map.
        let cpus = MP_ONCE.cpus.get().expect("CPUs not initialized");
        let cpu_ptr = cpus.as_ptr() as *mut crate::proc::Cpu;
        let c = &mut *cpu_ptr.add(cpuid());
        
        c.gdt[SEG_KCODE as usize] = SegDesc::seg(STA_X | STA_R, 0, 0xffffffff, 0);
        c.gdt[SEG_KDATA as usize] = SegDesc::seg(STA_W, 0, 0xffffffff, 0);
        lgdt(&c.gdt, size_of_val(&c.gdt));
    }
}

pub fn switchuvm(p: *mut Proc) {
    if p.is_null() {
        panic!("switchuvm: no process");
    }
    unsafe {
        if (*p).kstack.is_null() {
            panic!("switchuvm: no kstack");
        }
    }

    pushcli();
    unsafe {
        let c = mycpu();
        c.gdt[SEG_UCODE as usize] =
        SegDesc::seg(STA_X | STA_R, (*p).offset as u32, PROCSIZE << 12, DPL_USER);

        c.gdt[SEG_UDATA as usize] =
        SegDesc::seg(STA_W, (*p).offset as u32, PROCSIZE << 12, DPL_USER);

        c.gdt[SEG_TSS as usize] = SegDesc::seg16(
            STS_T32A,
            (&c.ts as *const _ as usize) as u32,
            (size_of::<crate::mmu::TaskState>() - 1) as u32,
            0,
            0,
        );

        c.ts.ss0 = SEG_KDATA << 3;
        c.ts.esp0 = ((*p).kstack as usize + KSTACKSIZE) as u32;
        c.ts.iomb = 0xFFFF;
        ltr(SEG_TSS << 3);
    }
    popcli();
}
