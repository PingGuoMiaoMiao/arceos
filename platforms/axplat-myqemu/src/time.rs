//! Time and timer implementation for myqemu platform

pub use core::time::Duration;
use aarch64_cpu::registers::Readable;

/// Timer frequency (approx. 62.5 MHz for QEMU generic timer)
const TIMER_FREQ: u64 = 62_500_000;

// ArceOS 期望的时间常量
pub const NANOS_PER_SEC: u64 = 1_000_000_000;
pub const NANOS_PER_MILLIS: u64 = 1_000_000;
pub const NANOS_PER_MICROS: u64 = 1_000;
pub const MILLIS_PER_SEC: u64 = 1_000;
pub const MICROS_PER_SEC: u64 = 1_000_000;

pub type TimeValue = Duration;

/// Get current time in nanoseconds
pub fn current_time_nanos() -> u64 {
    // Read from ARM generic timer
    let cnt = aarch64_cpu::registers::CNTPCT_EL0.get();
    // Convert to nanoseconds
    cnt * 1_000_000_000 / TIMER_FREQ
}

/// Get current time as Duration
pub fn current_time() -> Duration {
    Duration::from_nanos(current_time_nanos())
}

/// Busy wait for the specified duration
pub fn busy_wait(duration: Duration) {
    let start = current_time_nanos();
    let end = start + duration.as_nanos() as u64;
    
    while current_time_nanos() < end {
        core::hint::spin_loop();
    }
}

/// Busy wait for the specified microseconds
pub fn busy_wait_us(us: u64) {
    busy_wait(Duration::from_micros(us));
}

/// Busy wait until specific time
pub fn busy_wait_until(deadline: TimeValue) {
    let now = current_time();
    if deadline > now {
        busy_wait(deadline - now);
    }
}

// ArceOS 期望的时间接口
pub fn current_ticks() -> u64 {
    current_time_nanos() / (NANOS_PER_SEC / 100) // 假设 100Hz 时钟
}

pub fn nanos_to_ticks(nanos: u64) -> u64 {
    nanos / (NANOS_PER_SEC / 100)
}

pub fn ticks_to_nanos(ticks: u64) -> u64 {
    ticks * (NANOS_PER_SEC / 100)
}

pub fn monotonic_time() -> TimeValue {
    current_time()
}

pub fn monotonic_time_nanos() -> u64 {
    current_time_nanos()
}

pub fn wall_time() -> TimeValue {
    current_time()
}

pub fn wall_time_nanos() -> u64 {
    current_time_nanos()
}

pub fn epochoffset_nanos() -> u64 {
    0 // 纪元偏移，简化为 0
}