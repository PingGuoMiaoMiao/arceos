#[path = "../src/uart_rx.rs"]
mod uart_rx;

use std::cell::RefCell;
use std::collections::BTreeMap;

use uart_rx::{RegisterIo, UartRx};

#[derive(Default)]
struct FakeIo {
    registers: RefCell<BTreeMap<usize, u32>>,
    writes: RefCell<Vec<(usize, u32)>>,
}

impl FakeIo {
    fn preset(&self, address: usize, value: u32) {
        self.registers.borrow_mut().insert(address, value);
    }
}

impl RegisterIo for FakeIo {
    fn read_u32(&self, address: usize) -> u32 {
        self.registers.borrow().get(&address).copied().unwrap_or(0)
    }

    fn write_u32(&self, address: usize, value: u32) {
        self.registers.borrow_mut().insert(address, value);
        self.writes.borrow_mut().push((address, value));
    }
}

#[test]
fn receive_interrupt_enable_preserves_existing_ier_bits() {
    let io = FakeIo::default();
    let uart = UartRx::new(0x0414_0000, 2, io);
    uart.io().preset(0x0414_0004, 0x08);

    uart.enable_receive_interrupt();

    assert_eq!(uart.io().writes.borrow().as_slice(), &[(0x0414_0004, 0x09)]);
}

#[test]
fn receive_interrupt_disable_clears_only_rdi() {
    let io = FakeIo::default();
    let uart = UartRx::new(0x0414_0000, 2, io);
    uart.io().preset(0x0414_0004, 0x09);

    uart.disable_receive_interrupt();

    assert_eq!(uart.io().writes.borrow().as_slice(), &[(0x0414_0004, 0x08)]);
}

#[test]
fn drain_reads_every_ready_byte_and_stops_when_fifo_is_empty() {
    let io = FakeIo::default();
    let uart = UartRx::new(0x0414_0000, 2, io);
    uart.io().preset(0x0414_0014, 0x01);
    uart.io().preset(0x0414_0000, b'Z' as u32);
    let mut bytes = Vec::new();

    uart.drain_receive_fifo(|byte| {
        bytes.push(byte);
        uart.io().preset(0x0414_0014, 0x00);
    });

    assert_eq!(bytes, vec![b'Z']);
}
