use crate::traps::{TICKS};

pub fn sys_uptime() -> i32 {
    TICKS.load(core::sync::atomic::Ordering::Relaxed) as i32
}