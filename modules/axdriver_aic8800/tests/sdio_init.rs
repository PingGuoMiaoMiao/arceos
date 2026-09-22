use axdriver_aic8800::device::{Aic8800Product, SDIO_BLOCK_SIZE};
use axdriver_aic8800::sdio::{AicSdioIo, initialize_sdio_functions};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    SetBlockSize(u8, u16),
    Enable(u8),
    Write(u8, u32, u8),
    Read(u8, u32),
    DelayUs(u32),
    DelayMs(u32),
}

struct FakeIo {
    events: Vec<Event>,
    pending_status: u8,
}

impl FakeIo {
    fn new(pending_status: u8) -> Self {
        Self {
            events: Vec::new(),
            pending_status,
        }
    }
}

impl AicSdioIo for FakeIo {
    type Error = ();

    fn set_block_size(&mut self, function: u8, size: u16) -> Result<(), Self::Error> {
        self.events.push(Event::SetBlockSize(function, size));
        Ok(())
    }

    fn enable_function(&mut self, function: u8) -> Result<(), Self::Error> {
        self.events.push(Event::Enable(function));
        Ok(())
    }

    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error> {
        self.events.push(Event::Write(function, address, value));
        Ok(())
    }

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        self.events.push(Event::Read(function, address));
        Ok(self.pending_status)
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.events.push(Event::DelayMs(milliseconds));
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.events.push(Event::DelayUs(microseconds));
    }
}

#[test]
fn classic_products_initialize_both_functions_in_fixed_sdk_order() {
    let mut io = FakeIo::new(0);

    initialize_sdio_functions(&mut io, Aic8800Product::Aic8800Dc).unwrap();

    assert_eq!(
        io.events,
        vec![
            Event::SetBlockSize(1, SDIO_BLOCK_SIZE as u16),
            Event::Enable(1),
            Event::DelayUs(100),
            Event::SetBlockSize(2, SDIO_BLOCK_SIZE as u16),
            Event::Enable(2),
            Event::Write(2, 0x0b, 0x01),
            Event::Write(2, 0x11, 0x01),
            Event::Write(1, 0x0b, 0x01),
            Event::Write(1, 0x11, 0x01),
            Event::Write(1, 0x04, 0x07),
            Event::Write(2, 0x04, 0x07),
        ]
    );
}

#[test]
fn d80_products_apply_the_v3_function_and_interrupt_sequence() {
    let mut io = FakeIo::new(0x10);

    initialize_sdio_functions(&mut io, Aic8800Product::Aic8800D80).unwrap();

    assert_eq!(
        io.events,
        vec![
            Event::SetBlockSize(1, SDIO_BLOCK_SIZE as u16),
            Event::Enable(1),
            Event::Write(0, 0xf2, 0x7f),
            Event::Write(1, 0x07, 0x01),
            Event::Write(0, 0x04, 0x07),
            Event::Write(1, 0x00, 0x07),
        ]
    );
}

#[test]
fn aic8801_initialization_does_not_configure_function_two() {
    let mut io = FakeIo::new(0);

    initialize_sdio_functions(&mut io, Aic8800Product::Aic8801).unwrap();

    assert_eq!(
        io.events,
        vec![
            Event::SetBlockSize(1, SDIO_BLOCK_SIZE as u16),
            Event::Enable(1),
            Event::DelayUs(100),
            Event::Write(1, 0x0b, 0x01),
            Event::Write(1, 0x11, 0x01),
            Event::Write(1, 0x04, 0x07),
        ]
    );
}
