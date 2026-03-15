#![allow(unused_parens)] // False positive from bitfield macro

use modular_bitfield::prelude::*;

// Segment Descriptor

#[bitfield]
#[repr(C, packed)]
#[derive(Clone, Copy, Default, Debug)]
pub struct SegDesc {
    lim_15_0: B16,      // Low bits of segment limit
    base_15_0: B16,     // Low bits of segment base address
    base_23_16: B8,     // Middle bits of segment base address
    seg_type: B4,       // Segment type (see STA_ constants)
    s: B1,              // 0 = system, 1 = application
    dpl: B2,            // Descriptor Privilege Level
    p: B1,              // Present
    lim_19_16: B4,      // High bits of segment limit
    avl: B1,            // Unused (available for software use)
    rsv1: B1,           // Reserved
    db: B1,             // 0 = 16-bit segment, 1 = 32-bit segment
    g: B1,              // Granularity: limit scaled by 4K when set
    base_31_24: B8,     // High bits of segment base address
}

impl SegDesc {
    /// Create a normal segment descriptor
    /// Matches the C macro: SEG(type, base, lim, dpl)
    pub fn seg(seg_type: u8, base: u32, lim: u32, dpl: u8) -> Self {
        let mut seg = SegDesc::default();
        seg.set_lim_15_0(((lim >> 12) & 0xffff) as u16);
        seg.set_base_15_0((base & 0xffff) as u16);
        seg.set_base_23_16(((base >> 16) & 0xff) as u8);
        seg.set_seg_type(seg_type);
        seg.set_s(1);
        seg.set_dpl(dpl);
        seg.set_p(1);
        seg.set_lim_19_16((lim >> 28) as u8);
        seg.set_avl(0);
        seg.set_rsv1(0);
        seg.set_db(1);
        seg.set_g(1);
        seg.set_base_31_24(((base >> 24) & 0xff) as u8);
        seg
    }

    /// Create a 16-bit system segment descriptor.
    /// Matches the C macro: SEG16(type, base, lim, dpl)
    pub fn seg16(seg_type: u8, base: u32, lim: u32, dpl: u8, s_bit : u8) -> Self {
        let mut seg = SegDesc::default();
        seg.set_lim_15_0((lim & 0xffff) as u16);
        seg.set_base_15_0((base & 0xffff) as u16);
        seg.set_base_23_16(((base >> 16) & 0xff) as u8);
        seg.set_seg_type(seg_type);
        seg.set_s(s_bit);
        seg.set_dpl(dpl);
        seg.set_p(1);
        seg.set_lim_19_16(((lim >> 16) & 0xf) as u8);
        seg.set_avl(0);
        seg.set_rsv1(0);
        seg.set_db(0);
        seg.set_g(0);
        seg.set_base_31_24(((base >> 24) & 0xff) as u8);
        seg
    }

    pub fn self_set_system(&mut self) {
        self.set_s(0);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TaskState {
    pub link: u32,
    pub esp0: u32,
    pub ss0: u16,
    pub padding1: u16,
    pub esp1: u32,
    pub ss1: u16,
    pub padding2: u16,
    pub esp2: u32,
    pub ss2: u16,
    pub padding3: u16,
    pub cr3: u32,
    pub eip: u32,
    pub eflags: u32,
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esp: u32,
    pub ebp: u32,
    pub esi: u32,
    pub edi: u32,
    pub es: u16,
    pub padding4: u16,
    pub cs: u16,
    pub padding5: u16,
    pub ss: u16,
    pub padding6: u16,
    pub ds: u16,
    pub padding7: u16,
    pub fs: u16,
    pub padding8: u16,
    pub gs: u16,
    pub padding9: u16,
    pub ldt: u16,
    pub padding10: u16,
    pub t: u16,
    pub iomb: u16,
}

impl TaskState {
    pub const fn new() -> Self {
        Self {
            link: 0,
            esp0: 0,
            ss0: 0,
            padding1: 0,
            esp1: 0,
            ss1: 0,
            padding2: 0,
            esp2: 0,
            ss2: 0,
            padding3: 0,
            cr3: 0,
            eip: 0,
            eflags: 0,
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            es: 0,
            padding4: 0,
            cs: 0,
            padding5: 0,
            ss: 0,
            padding6: 0,
            ds: 0,
            padding7: 0,
            fs: 0,
            padding8: 0,
            gs: 0,
            padding9: 0,
            ldt: 0,
            padding10: 0,
            t: 0,
            iomb: 0,
        }
    }
}
