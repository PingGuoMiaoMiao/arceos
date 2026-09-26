use core::sync::atomic::{AtomicU32, Ordering};

use crate::host::EventSource;

pub struct AtomicEvents<'a> {
    storage: &'a AtomicU32,
}

impl<'a> AtomicEvents<'a> {
    pub const fn new(storage: &'a AtomicU32) -> Self {
        Self { storage }
    }
}

impl EventSource for AtomicEvents<'_> {
    fn clear(&self) {
        self.storage.store(0, Ordering::Release);
    }

    fn take(&self) -> u32 {
        self.storage.swap(0, Ordering::AcqRel)
    }
}

pub fn accumulate_events(storage: &AtomicU32, status: u32) {
    storage.fetch_or(status, Ordering::AcqRel);
}
