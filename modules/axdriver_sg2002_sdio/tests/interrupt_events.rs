#[path = "../src/host.rs"]
mod host;
#[path = "../src/interrupt.rs"]
mod interrupt;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/registers.rs"]
mod registers;

use core::sync::atomic::AtomicU32;

use host::EventSource;
use interrupt::{AtomicEvents, accumulate_events};

#[test]
fn interrupt_statuses_accumulate_until_the_command_takes_them() {
    let storage = AtomicU32::new(0);
    let events = AtomicEvents::new(&storage);

    accumulate_events(&storage, 0x0000_0001);
    accumulate_events(&storage, 0x0002_8000);

    assert_eq!(events.take(), 0x0002_8001);
    assert_eq!(events.take(), 0);
}

#[test]
fn clearing_events_starts_a_new_command_with_no_stale_status() {
    let storage = AtomicU32::new(0x0000_0001);
    let events = AtomicEvents::new(&storage);

    events.clear();

    assert_eq!(events.take(), 0);
}
