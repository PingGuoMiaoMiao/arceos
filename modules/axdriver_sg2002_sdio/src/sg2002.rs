use axhal::mem::{pa, phys_to_virt};
use core::sync::atomic::AtomicU32;

use crate::host::{MonotonicClock, RegisterIo, SdhciHost};
use crate::interrupt::{AtomicEvents, accumulate_events};

pub const SDIO1_PADDR: usize = 0x0432_0000;
pub const SDIO1_IRQ: usize = 38;
pub const SDIO1_BASE_CLOCK_HZ: u32 = 375_000_000;

const INT_STATUS: usize = 0x30;

static SDIO1_EVENTS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
pub struct VolatileMmio {
    base: usize,
}

impl VolatileMmio {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    fn address(&self, offset: usize) -> usize {
        self.base + offset
    }
}

impl RegisterIo for VolatileMmio {
    fn read_u8(&self, offset: usize) -> u8 {
        // SAFETY: The host only requests byte registers within the mapped
        // SDIO1 MMIO page and volatile access is required for device memory.
        unsafe { (self.address(offset) as *const u8).read_volatile() }
    }

    fn read_u16(&self, offset: usize) -> u16 {
        // SAFETY: Every 16-bit host register offset is naturally aligned and
        // lies inside the mapped SDIO1 MMIO page.
        unsafe { (self.address(offset) as *const u16).read_volatile() }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        // SAFETY: Every 32-bit host register offset is naturally aligned and
        // lies inside the mapped SDIO1 MMIO page.
        unsafe { (self.address(offset) as *const u32).read_volatile() }
    }

    fn write_u8(&self, offset: usize, value: u8) {
        // SAFETY: See read_u8; this is the corresponding volatile write.
        unsafe { (self.address(offset) as *mut u8).write_volatile(value) }
    }

    fn write_u16(&self, offset: usize, value: u16) {
        // SAFETY: See read_u16; this is the corresponding volatile write.
        unsafe { (self.address(offset) as *mut u16).write_volatile(value) }
    }

    fn write_u32(&self, offset: usize, value: u32) {
        // SAFETY: See read_u32; this is the corresponding volatile write.
        unsafe { (self.address(offset) as *mut u32).write_volatile(value) }
    }
}

#[derive(Clone, Copy)]
pub struct AxClock;

impl MonotonicClock for AxClock {
    fn now_ns(&self) -> u64 {
        axhal::time::monotonic_time_nanos()
    }

    fn relax(&self) {
        core::hint::spin_loop();
    }
}

pub type Sdio1Host = SdhciHost<VolatileMmio, AtomicEvents<'static>, AxClock>;

pub fn host() -> Sdio1Host {
    let base = phys_to_virt(pa!(SDIO1_PADDR)).as_usize();
    SdhciHost::new(
        VolatileMmio::new(base),
        AtomicEvents::new(&SDIO1_EVENTS),
        AxClock,
        SDIO1_BASE_CLOCK_HZ,
    )
}

pub fn register_interrupt() -> bool {
    axhal::irq::register(SDIO1_IRQ, interrupt_handler)
}

fn interrupt_handler() {
    let io = VolatileMmio::new(phys_to_virt(pa!(SDIO1_PADDR)).as_usize());
    let status = io.read_u32(INT_STATUS);
    if status != 0 {
        io.write_u32(INT_STATUS, status);
        accumulate_events(&SDIO1_EVENTS, status);
    }
}
