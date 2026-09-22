#[path = "../src/cis.rs"]
mod cis;
#[path = "../src/enumeration.rs"]
mod enumeration;
#[path = "../src/host.rs"]
mod host;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/registers.rs"]
mod registers;

use std::cell::RefCell;
use std::collections::BTreeMap;

use enumeration::{SdioBus, enumerate};
use host::HostError;
use protocol::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Call {
    Command(u8, u32),
    DelayMs(u32),
    HostWidth4,
    Clock(u32),
}

struct FakeBus {
    bytes: RefCell<BTreeMap<u32, u8>>,
    calls: RefCell<Vec<Call>>,
    fail_reset_commands: bool,
}

impl FakeBus {
    fn new() -> Self {
        let mut bytes = BTreeMap::new();

        bytes.insert(0x00, 0x32);
        bytes.insert(0x06, 0x00);
        bytes.insert(0x07, 0x00);
        bytes.insert(0x08, 0x02);
        bytes.insert(0x09, 0x00);
        bytes.insert(0x0a, 0x10);
        bytes.insert(0x0b, 0x00);
        bytes.insert(0x13, 0x01);

        bytes.insert(0x100, 0x07);
        bytes.insert(0x109, 0x00);
        bytes.insert(0x10a, 0x11);
        bytes.insert(0x10b, 0x00);

        let common_cis = [0x20, 0x04, 0x34, 0x12, 0x78, 0x56, 0xff];
        for (index, value) in common_cis.into_iter().enumerate() {
            bytes.insert(0x1000 + index as u32, value);
        }

        let mut function_cis = vec![0x22, 0x0e, 0x01];
        function_cis.extend([0_u8; 11]);
        function_cis.extend([0x00, 0x02, 0xff]);
        for (index, value) in function_cis.into_iter().enumerate() {
            bytes.insert(0x1100 + index as u32, value);
        }

        Self {
            bytes: RefCell::new(bytes),
            calls: RefCell::new(Vec::new()),
            fail_reset_commands: false,
        }
    }

    fn with_reset_failure(mut self) -> Self {
        self.fail_reset_commands = true;
        self
    }
}

impl SdioBus for FakeBus {
    fn command(&self, command: Command, argument: u32) -> Result<u32, HostError> {
        self.calls
            .borrow_mut()
            .push(Call::Command(command.index(), argument));
        match command.index() {
            0 => Ok(0),
            3 => Ok(0x1234_0000),
            5 if argument == 0 => Ok(0x10ff_8000),
            5 => Ok(0x90ff_8000),
            7 => Ok(0),
            52 => {
                let write = argument & (1 << 31) != 0;
                let address = (argument >> 9) & 0x1ffff;
                let value = argument as u8;
                if self.fail_reset_commands && address == 0x06 {
                    return Err(HostError::Timeout);
                }
                if write {
                    self.bytes.borrow_mut().insert(address, value);
                    Ok(value as u32)
                } else {
                    Ok(self.bytes.borrow().get(&address).copied().unwrap_or(0) as u32)
                }
            }
            other => panic!("unexpected command {other}"),
        }
    }

    fn delay_ms(&self, milliseconds: u32) {
        self.calls.borrow_mut().push(Call::DelayMs(milliseconds));
    }

    fn delay_us(&self, _microseconds: u32) {}

    fn set_host_bus_width_4(&self) {
        self.calls.borrow_mut().push(Call::HostWidth4);
    }

    fn set_clock(&self, requested_hz: u32) -> Result<u32, HostError> {
        self.calls.borrow_mut().push(Call::Clock(requested_hz));
        Ok(23_437_500)
    }
}

#[test]
fn optional_cmd52_reset_failure_does_not_stop_sdio_probe() {
    let bus = FakeBus::new().with_reset_failure();

    let card = enumerate(&bus).unwrap();

    assert_eq!(card.function_count, 1);
    let calls = bus.calls.borrow();
    assert_eq!(calls[0], Call::Command(52, 0x0000_0c00));
    assert_eq!(calls[1], Call::Command(52, 0x8000_0c08));
    assert_eq!(calls[2], Call::Command(0, 0));
    assert_eq!(calls[3], Call::Command(5, 0));
}

#[test]
fn enumerates_one_sdio_function_and_reads_identity_from_cis() {
    let bus = FakeBus::new();

    let card = enumerate(&bus).unwrap();

    assert_eq!(card.function_count, 1);
    assert_eq!(card.ocr, 0x00ff_8000);
    assert_eq!(card.rca, 0x1234);
    assert_eq!(card.cccr_revision, 2);
    assert_eq!(card.sdio_revision, 3);
    assert_eq!(card.capabilities, 0x02);
    assert_eq!(card.speed, 0x01);
    assert_eq!(card.actual_clock_hz, 23_437_500);

    let function = card.functions[0].unwrap();
    assert_eq!(function.number, 1);
    assert_eq!(function.class, 0x07);
    assert_eq!(function.vendor, 0x1234);
    assert_eq!(function.device, 0x5678);
    assert_eq!(function.max_block_size, 512);

    let calls = bus.calls.borrow();
    assert_eq!(calls[0], Call::Command(52, 0x0000_0c00));
    assert_eq!(calls[1], Call::Command(52, 0x8000_0c08));
    assert_eq!(calls[2], Call::Command(0, 0));
    assert_eq!(calls[3], Call::Command(5, 0));
    assert_eq!(calls[4], Call::Command(5, 0x00ff_8000));
    assert_eq!(calls[5], Call::Command(3, 0));
    assert_eq!(calls[6], Call::Command(7, 0x1234_0000));
    assert!(calls.contains(&Call::HostWidth4));
    assert!(calls.contains(&Call::Clock(25_000_000)));
    assert_eq!(bus.bytes.borrow().get(&0x07), Some(&0x02));
}
