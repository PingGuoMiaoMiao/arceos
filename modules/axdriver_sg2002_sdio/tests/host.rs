#[path = "../src/host.rs"]
mod host;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/registers.rs"]
mod registers;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};

use host::{DataDirection, EventSource, HostError, MonotonicClock, RegisterIo, SdhciHost};
use protocol::{Command, ResponseKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Write {
    U8(usize, u8),
    U16(usize, u16),
    U32(usize, u32),
}

#[derive(Default)]
struct FakeIo {
    values: RefCell<BTreeMap<usize, u32>>,
    writes: RefCell<Vec<Write>>,
}

impl FakeIo {
    fn preset(&self, offset: usize, value: u32) {
        self.values.borrow_mut().insert(offset, value);
    }
}

impl RegisterIo for FakeIo {
    fn read_u8(&self, offset: usize) -> u8 {
        if offset == 0x2f {
            return 0;
        }
        self.values.borrow().get(&offset).copied().unwrap_or(0) as u8
    }

    fn read_u16(&self, offset: usize) -> u16 {
        let value = self.values.borrow().get(&offset).copied().unwrap_or(0) as u16;
        if offset == 0x2c && value & 0x0001 != 0 {
            value | 0x0002
        } else {
            value
        }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        self.values.borrow().get(&offset).copied().unwrap_or(0)
    }

    fn write_u8(&self, offset: usize, value: u8) {
        self.writes.borrow_mut().push(Write::U8(offset, value));
        self.values.borrow_mut().insert(offset, value as u32);
    }

    fn write_u16(&self, offset: usize, value: u16) {
        self.writes.borrow_mut().push(Write::U16(offset, value));
        self.values.borrow_mut().insert(offset, value as u32);
    }

    fn write_u32(&self, offset: usize, value: u32) {
        self.writes.borrow_mut().push(Write::U32(offset, value));
        self.values.borrow_mut().insert(offset, value);
    }
}

struct FakeEvents {
    events: RefCell<VecDeque<u32>>,
}

impl FakeEvents {
    fn new(events: impl IntoIterator<Item = u32>) -> Self {
        Self {
            events: RefCell::new(events.into_iter().collect()),
        }
    }
}

impl EventSource for FakeEvents {
    fn clear(&self) {}

    fn take(&self) -> u32 {
        self.events.borrow_mut().pop_front().unwrap_or(0)
    }
}

#[derive(Default)]
struct FakeClock {
    now: Cell<u64>,
}

impl MonotonicClock for FakeClock {
    fn now_ns(&self) -> u64 {
        let now = self.now.get();
        self.now.set(now + 1_000_000);
        now
    }

    fn relax(&self) {}
}

#[test]
fn initialization_resets_host_applies_cv181x_sdio_phy_and_sets_400khz() {
    let host = SdhciHost::new(
        FakeIo::default(),
        FakeEvents::new([]),
        FakeClock::default(),
        375_000_000,
    );

    let actual_hz = host.initialize(400_000).unwrap();

    assert_eq!(actual_hz, 399_786);
    let writes = host.io().writes.borrow();
    assert!(writes.contains(&Write::U32(0x34, 0)));
    assert!(writes.contains(&Write::U32(0x38, 0)));
    assert!(writes.contains(&Write::U8(0x2f, 0x01)));
    assert!(writes.contains(&Write::U32(0x200, 0x0001_0002)));
    assert!(writes.contains(&Write::U32(0x24c, 0x0000_0001)));
    assert!(writes.contains(&Write::U32(0x240, 0x0100_0100)));
    assert!(writes.contains(&Write::U8(0x29, 0x0f)));
    assert!(writes.contains(&Write::U16(0x2c, 0xd541)));
    assert!(writes.contains(&Write::U16(0x2c, 0xd547)));
}

#[test]
fn command_completion_returns_response_and_uses_argument_before_command() {
    let io = FakeIo::default();
    io.preset(0x10, 0xb0ff_8000);
    let host = SdhciHost::new(
        io,
        FakeEvents::new([0x0000_0001]),
        FakeClock::default(),
        375_000_000,
    );

    let response = host
        .send_command(Command::new(5, ResponseKind::R4), 0)
        .unwrap();

    assert_eq!(response, 0xb0ff_8000);
    let writes = host.io().writes.borrow();
    let argument_index = writes
        .iter()
        .position(|write| *write == Write::U32(0x08, 0))
        .unwrap();
    let command_index = writes
        .iter()
        .position(|write| *write == Write::U16(0x0e, 0x0502))
        .unwrap();
    assert!(argument_index < command_index);
}

#[test]
fn command_error_status_is_reported_without_reading_it_as_success() {
    let io = FakeIo::default();
    io.preset(0x24, 0x00f0_0000);
    let host = SdhciHost::new(
        io,
        FakeEvents::new([0x0002_8000]),
        FakeClock::default(),
        375_000_000,
    );

    let result = host.send_command(Command::new(52, ResponseKind::R5), 0x8000_0c08);

    assert_eq!(
        result,
        Err(HostError::CommandInterrupt {
            command_index: 52,
            argument: 0x8000_0c08,
            interrupt_status: 0x0002_8000,
            present_state: 0x00f0_0000,
        })
    );
    let writes = host.io().writes.borrow();
    assert!(writes.contains(&Write::U8(0x2f, 0x02)));
    assert!(writes.contains(&Write::U8(0x2f, 0x04)));
}

#[test]
fn command_wait_has_a_bounded_timeout() {
    let host = SdhciHost::new(
        FakeIo::default(),
        FakeEvents::new([]),
        FakeClock::default(),
        375_000_000,
    );

    let result = host.send_command(Command::new(5, ResponseKind::R4), 0);

    assert_eq!(result, Err(HostError::Timeout));
}

#[test]
fn switching_to_four_bit_mode_updates_the_sdhci_host_control_register() {
    let io = FakeIo::default();
    io.preset(0x28, 0x40);
    let host = SdhciHost::new(io, FakeEvents::new([]), FakeClock::default(), 375_000_000);

    host.set_bus_width_4();

    assert!(host.io().writes.borrow().contains(&Write::U8(0x28, 0x42)));
}

#[test]
fn pio_write_programs_two_blocks_and_waits_for_data_end() {
    let host = SdhciHost::new(
        FakeIo::default(),
        FakeEvents::new([0x0000_0011, 0x0000_0010, 0x0000_0002]),
        FakeClock::default(),
        375_000_000,
    );
    let mut payload = [1_u8, 2, 3, 4, 5, 6, 7, 8];

    let response = host
        .transfer_pio(
            Command::new(53, ResponseKind::R5).with_data(),
            0x9800_2002,
            DataDirection::Write,
            &mut payload,
            4,
        )
        .unwrap();

    assert_eq!(response, 0);
    let writes = host.io().writes.borrow();
    assert!(writes.contains(&Write::U16(0x04, 4)));
    assert!(writes.contains(&Write::U16(0x06, 2)));
    assert!(writes.contains(&Write::U16(0x0c, 0x22)));
    assert!(writes.contains(&Write::U16(0x0e, 0x353a)));
    assert!(writes.contains(&Write::U32(0x20, 0x0403_0201)));
    assert!(writes.contains(&Write::U32(0x20, 0x0807_0605)));
}
