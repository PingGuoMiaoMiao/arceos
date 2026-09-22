#[path = "../src/cis.rs"]
mod cis;
#[path = "../src/enumeration.rs"]
mod enumeration;
#[path = "../src/function.rs"]
mod function;
#[path = "../src/host.rs"]
mod host;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/registers.rs"]
mod registers;

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

use enumeration::SdioBus;
use function::{SdioFunction, SdioTransferBus};
use host::{DataDirection, HostError};
use protocol::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Command(u8, u32),
    DelayMs(u32),
    Transfer {
        argument: u32,
        direction: DataDirection,
        length: usize,
        block_size: u16,
    },
}

struct FakeBus {
    registers: RefCell<BTreeMap<(u8, u32), u8>>,
    ready_values: RefCell<VecDeque<u8>>,
    calls: RefCell<Vec<Call>>,
}

impl FakeBus {
    fn new() -> Self {
        let mut registers = BTreeMap::new();
        registers.insert((0, 0x02), 0);
        Self {
            registers: RefCell::new(registers),
            ready_values: RefCell::new([0, 0x02].into()),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl SdioBus for FakeBus {
    fn command(&self, command: Command, argument: u32) -> Result<u32, HostError> {
        self.calls
            .borrow_mut()
            .push(Call::Command(command.index(), argument));
        assert_eq!(command.index(), 52);
        let write = argument & (1 << 31) != 0;
        let function = ((argument >> 28) & 0x7) as u8;
        let address = (argument >> 9) & 0x1ffff;
        let value = argument as u8;
        if write {
            self.registers
                .borrow_mut()
                .insert((function, address), value);
            Ok(value as u32)
        } else if function == 0 && address == 0x03 {
            Ok(self.ready_values.borrow_mut().pop_front().unwrap_or(0x02) as u32)
        } else {
            Ok(self
                .registers
                .borrow()
                .get(&(function, address))
                .copied()
                .unwrap_or(0) as u32)
        }
    }

    fn delay_ms(&self, milliseconds: u32) {
        self.calls.borrow_mut().push(Call::DelayMs(milliseconds));
    }

    fn delay_us(&self, _microseconds: u32) {}

    fn set_host_bus_width_4(&self) {}

    fn set_clock(&self, _requested_hz: u32) -> Result<u32, HostError> {
        unreachable!()
    }
}

impl SdioTransferBus for FakeBus {
    fn transfer(
        &self,
        _command: Command,
        argument: u32,
        direction: DataDirection,
        buffer: &mut [u8],
        block_size: u16,
    ) -> Result<u32, HostError> {
        self.calls.borrow_mut().push(Call::Transfer {
            argument,
            direction,
            length: buffer.len(),
            block_size,
        });
        Ok(0)
    }
}

#[test]
fn configures_block_size_enables_function_and_waits_for_ready() {
    let bus = FakeBus::new();
    let function = SdioFunction::new(&bus, 1, 512).unwrap();

    function.configure_block_size().unwrap();
    function.enable(10).unwrap();

    let registers = bus.registers.borrow();
    assert_eq!(registers.get(&(0, 0x110)), Some(&0x00));
    assert_eq!(registers.get(&(0, 0x111)), Some(&0x02));
    assert_eq!(registers.get(&(0, 0x02)), Some(&0x02));
    assert!(bus.calls.borrow().contains(&Call::DelayMs(1)));
}

#[test]
fn writes_two_blocks_to_a_fixed_function_fifo_with_cmd53() {
    let bus = FakeBus::new();
    let function = SdioFunction::new(&bus, 1, 512).unwrap();
    let mut payload = [0_u8; 1024];

    function.write_fifo(0x10, &mut payload).unwrap();

    assert!(bus.calls.borrow().contains(&Call::Transfer {
        argument: 0x9800_2002,
        direction: DataDirection::Write,
        length: 1024,
        block_size: 512,
    }));
}

#[test]
fn exposes_function_zero_register_access_for_device_initialization() {
    let bus = FakeBus::new();
    let function = SdioFunction::new(&bus, 1, 512).unwrap();

    function.write_function_zero_register(0xf2, 0x7f).unwrap();
    let value = function.read_function_zero_register(0xf2).unwrap();

    assert_eq!(value, 0x7f);
    assert_eq!(function.number(), 1);
    assert_eq!(function.block_size(), 512);
}
