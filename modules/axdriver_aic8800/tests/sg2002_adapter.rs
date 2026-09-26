#![cfg(feature = "sg2002")]

use std::cell::RefCell;
use std::collections::BTreeMap;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::sdio::{initialize_sdio_functions, send_command_transfer};
use axdriver_aic8800::sg2002::Sg2002SdioIo;
use axdriver_sg2002_sdio::enumeration::SdioBus;
use axdriver_sg2002_sdio::function::SdioTransferBus;
use axdriver_sg2002_sdio::host::{DataDirection, HostError};
use axdriver_sg2002_sdio::protocol::Command;

struct FakeBus {
    registers: RefCell<BTreeMap<(u8, u32), u8>>,
    transfers: RefCell<Vec<(u32, DataDirection, usize, u16)>>,
}

impl FakeBus {
    fn new() -> Self {
        Self {
            registers: RefCell::new(BTreeMap::from([((0, 0x02), 0)])),
            transfers: RefCell::new(Vec::new()),
        }
    }
}

impl SdioBus for FakeBus {
    fn command(&self, command: Command, argument: u32) -> Result<u32, HostError> {
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
            Ok(*self.registers.borrow().get(&(0, 0x02)).unwrap_or(&0) as u32)
        } else {
            Ok(*self
                .registers
                .borrow()
                .get(&(function, address))
                .unwrap_or(&0) as u32)
        }
    }

    fn delay_ms(&self, _milliseconds: u32) {}

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
        self.transfers
            .borrow_mut()
            .push((argument, direction, buffer.len(), block_size));
        Ok(0)
    }
}

#[test]
fn classic_initialization_is_executed_through_real_sdio_function_operations() {
    let bus = FakeBus::new();
    let mut io = Sg2002SdioIo::new(&bus, 20).unwrap();

    initialize_sdio_functions(&mut io, Aic8800Product::Aic8800Dc).unwrap();

    let registers = bus.registers.borrow();
    assert_eq!(registers.get(&(0, 0x110)), Some(&0x00));
    assert_eq!(registers.get(&(0, 0x111)), Some(&0x02));
    assert_eq!(registers.get(&(0, 0x210)), Some(&0x00));
    assert_eq!(registers.get(&(0, 0x211)), Some(&0x02));
    assert_eq!(registers.get(&(0, 0x02)), Some(&0x06));
    assert_eq!(registers.get(&(1, 0x0b)), Some(&0x01));
    assert_eq!(registers.get(&(1, 0x11)), Some(&0x01));
    assert_eq!(registers.get(&(1, 0x04)), Some(&0x07));
    assert_eq!(registers.get(&(2, 0x0b)), Some(&0x01));
    assert_eq!(registers.get(&(2, 0x11)), Some(&0x01));
    assert_eq!(registers.get(&(2, 0x04)), Some(&0x07));
}

#[test]
fn v3_initialization_writes_function_zero_and_function_one_interrupt_registers() {
    let bus = FakeBus::new();
    bus.registers.borrow_mut().insert((1, 0x01), 0x10);
    let mut io = Sg2002SdioIo::new(&bus, 20).unwrap();

    initialize_sdio_functions(&mut io, Aic8800Product::Aic8800D80).unwrap();

    let registers = bus.registers.borrow();
    assert_eq!(registers.get(&(0, 0xf2)), Some(&0x7f));
    assert_eq!(registers.get(&(0, 0x04)), Some(&0x07));
    assert_eq!(registers.get(&(1, 0x07)), Some(&0x01));
    assert_eq!(registers.get(&(1, 0x00)), Some(&0x07));
}

#[test]
fn d80_command_transfer_reaches_function_one_fifo_through_cmd53() {
    let bus = FakeBus::new();
    bus.registers.borrow_mut().insert((1, 0x03), 1);
    let mut io = Sg2002SdioIo::new(&bus, 20).unwrap();
    let mut transfer = [0_u8; 512];

    send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut transfer).unwrap();

    assert_eq!(
        *bus.transfers.borrow(),
        vec![(0x9800_2001, DataDirection::Write, 512, 512)]
    );
}
