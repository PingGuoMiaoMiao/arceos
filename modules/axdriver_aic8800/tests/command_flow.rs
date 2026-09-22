use std::collections::VecDeque;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::sdio::{
    AicCommandError, AicCommandIo, poll_command_flow_control, send_command_transfer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(u8, u32),
    WriteFifo(u8, u32, usize),
    DelayUs(u32),
    DelayMs(u32),
}

struct FakeIo {
    flow_values: VecDeque<u8>,
    events: Vec<Event>,
}

impl FakeIo {
    fn new(flow_values: impl IntoIterator<Item = u8>) -> Self {
        Self {
            flow_values: flow_values.into_iter().collect(),
            events: Vec::new(),
        }
    }
}

impl AicCommandIo for FakeIo {
    type Error = ();

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        self.events.push(Event::Read(function, address));
        Ok(self.flow_values.pop_front().unwrap_or(0))
    }

    fn write_fifo(
        &mut self,
        function: u8,
        address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.events
            .push(Event::WriteFifo(function, address, data.len()));
        Ok(())
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.events.push(Event::DelayUs(microseconds));
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.events.push(Event::DelayMs(milliseconds));
    }
}

#[test]
fn d80_flow_control_uses_function_one_v3_register_without_masking() {
    let mut io = FakeIo::new([0x80]);

    let count = poll_command_flow_control(&mut io, Aic8800Product::Aic8800D80).unwrap();

    assert_eq!(count, 0x80);
    assert_eq!(io.events, vec![Event::Read(1, 0x03)]);
}

#[test]
fn aic8801_flow_control_masks_bit_seven_and_waits_200_microseconds() {
    let mut io = FakeIo::new([0x80, 0x81]);

    let count = poll_command_flow_control(&mut io, Aic8800Product::Aic8801).unwrap();

    assert_eq!(count, 1);
    assert_eq!(
        io.events,
        vec![
            Event::Read(1, 0x0a),
            Event::DelayUs(200),
            Event::Read(1, 0x0a),
        ]
    );
}

#[test]
fn exhausted_flow_control_poll_uses_the_fixed_sdk_delay_schedule() {
    let mut io = FakeIo::new([]);

    let count = poll_command_flow_control(&mut io, Aic8800Product::Aic8800D80).unwrap();

    assert_eq!(count, 0);
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Read(1, 0x03)))
            .count(),
        51
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::DelayUs(200)))
            .count(),
        29
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::DelayMs(1)))
            .count(),
        10
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::DelayMs(10)))
            .count(),
        11
    );
}

#[test]
fn d80_command_uses_function_one_and_the_v3_write_fifo() {
    let mut io = FakeIo::new([1]);
    let mut transfer = [0_u8; 512];

    send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut transfer).unwrap();

    assert_eq!(
        io.events,
        vec![Event::Read(1, 0x03), Event::WriteFifo(1, 0x10, 512)]
    );
}

#[test]
fn classic_dc_command_bypasses_flow_control_and_uses_function_two() {
    let mut io = FakeIo::new([]);
    let mut transfer = [0_u8; 512];

    send_command_transfer(&mut io, Aic8800Product::Aic8800Dc, &mut transfer).unwrap();

    assert_eq!(io.events, vec![Event::WriteFifo(2, 0x07, 512)]);
}

#[test]
fn transfer_equal_to_reported_capacity_is_rejected_like_the_fixed_sdk() {
    let mut io = FakeIo::new([1]);
    let mut transfer = [0_u8; 1536];

    assert_eq!(
        send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut transfer),
        Err(AicCommandError::InsufficientFlowControl {
            buffer_count: 1,
            transfer_length: 1536,
        })
    );
    assert_eq!(io.events, vec![Event::Read(1, 0x03)]);
}

#[test]
fn insufficient_capacity_is_polled_initially_and_ten_more_times() {
    let mut io = FakeIo::new([1; 11]);
    let mut transfer = [0_u8; 2048];

    assert_eq!(
        send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut transfer),
        Err(AicCommandError::InsufficientFlowControl {
            buffer_count: 1,
            transfer_length: 2048,
        })
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Read(1, 0x03)))
            .count(),
        11
    );
    assert!(
        !io.events
            .iter()
            .any(|event| matches!(event, Event::WriteFifo(_, _, _)))
    );
}

#[test]
fn empty_or_non_block_aligned_command_transfer_is_rejected() {
    let mut io = FakeIo::new([]);
    let mut empty = [];
    let mut short = [0_u8; 511];

    assert_eq!(
        send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut empty),
        Err(AicCommandError::InvalidTransferLength { length: 0 })
    );
    assert_eq!(
        send_command_transfer(&mut io, Aic8800Product::Aic8800D80, &mut short),
        Err(AicCommandError::InvalidTransferLength { length: 511 })
    );
    assert!(io.events.is_empty());
}
