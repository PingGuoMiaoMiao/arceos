use std::collections::VecDeque;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::{
    AicResponseError, AicResponseIo, D80_MAXIMUM_RECEIVE_TRANSFER_LENGTH, read_config_response,
    receive_pending_frame,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    ReadRegister(u8, u32),
    WriteRegister(u8, u32, u8),
    ReadFifo(u8, u32, usize),
    DelayMs(u32),
}

struct FakeIo {
    interrupt_statuses: VecDeque<u8>,
    byte_mode_length: u8,
    pending: u8,
    frame: Vec<u8>,
    frames: VecDeque<Vec<u8>>,
    events: Vec<Event>,
}

impl FakeIo {
    fn new(interrupt_statuses: impl IntoIterator<Item = u8>) -> Self {
        Self {
            interrupt_statuses: interrupt_statuses.into_iter().collect(),
            byte_mode_length: 0,
            pending: 0,
            frame: Vec::new(),
            frames: VecDeque::new(),
            events: Vec::new(),
        }
    }
}

impl AicResponseIo for FakeIo {
    type Error = ();

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        self.events.push(Event::ReadRegister(function, address));
        Ok(match (function, address) {
            (1, 0x04) => self.interrupt_statuses.pop_front().unwrap_or(0),
            (1, 0x05) => self.byte_mode_length,
            (1, 0x01) => self.pending,
            value => panic!("unexpected register read {value:?}"),
        })
    }

    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error> {
        self.events
            .push(Event::WriteRegister(function, address, value));
        Ok(())
    }

    fn read_fifo(
        &mut self,
        function: u8,
        address: u32,
        output: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.events
            .push(Event::ReadFifo(function, address, output.len()));
        let frame = self
            .frames
            .pop_front()
            .unwrap_or_else(|| self.frame.clone());
        let copy_length = frame.len().min(output.len());
        output[..copy_length].copy_from_slice(&frame[..copy_length]);
        Ok(())
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.events.push(Event::DelayMs(milliseconds));
    }
}

#[test]
fn d80_block_status_reads_function_one_fifo_in_512_byte_units() {
    let mut io = FakeIo::new([2]);
    let mut output = [0_u8; 1024];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(length, Some(1024));
    assert_eq!(
        io.events,
        vec![Event::ReadRegister(1, 0x04), Event::ReadFifo(1, 0x0f, 1024),]
    );
}

#[test]
fn d80_maximum_receive_transfer_accepts_seven_sdio_blocks() {
    let mut io = FakeIo::new([7]);
    let mut output = [0_u8; D80_MAXIMUM_RECEIVE_TRANSFER_LENGTH];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(D80_MAXIMUM_RECEIVE_TRANSFER_LENGTH, 3584);
    assert_eq!(length, Some(3584));
    assert_eq!(
        io.events,
        vec![Event::ReadRegister(1, 0x04), Event::ReadFifo(1, 0x0f, 3584),]
    );
}

#[test]
fn d80_status_121_reads_one_block_from_function_two() {
    let mut io = FakeIo::new([121]);
    let mut output = [0_u8; 512];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(length, Some(512));
    assert_eq!(
        io.events,
        vec![Event::ReadRegister(1, 0x04), Event::ReadFifo(2, 0x0f, 512),]
    );
}

#[test]
fn d80_status_120_uses_function_one_byte_mode_length_times_four() {
    let mut io = FakeIo::new([120]);
    io.byte_mode_length = 5;
    let mut output = [0_u8; 20];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(length, Some(20));
    assert_eq!(
        io.events,
        vec![
            Event::ReadRegister(1, 0x04),
            Event::ReadRegister(1, 0x05),
            Event::ReadFifo(1, 0x0f, 20),
        ]
    );
}

#[test]
fn d80_status_127_uses_function_two_with_the_shared_byte_length_register() {
    let mut io = FakeIo::new([127]);
    io.byte_mode_length = 4;
    let mut output = [0_u8; 16];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(length, Some(16));
    assert_eq!(
        io.events,
        vec![
            Event::ReadRegister(1, 0x04),
            Event::ReadRegister(1, 0x05),
            Event::ReadFifo(2, 0x0f, 16),
        ]
    );
}

#[test]
fn d80_other_interrupt_clears_device_to_host_pending_without_reading_fifo() {
    let mut io = FakeIo::new([0x80]);
    io.pending = 0x13;
    let mut output = [0_u8; 512];

    let length = receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output).unwrap();

    assert_eq!(length, None);
    assert_eq!(
        io.events,
        vec![
            Event::ReadRegister(1, 0x04),
            Event::ReadRegister(1, 0x01),
            Event::WriteRegister(1, 0x01, 0x12),
        ]
    );
}

#[test]
fn frame_larger_than_output_is_rejected_before_fifo_access() {
    let mut io = FakeIo::new([2]);
    let mut output = [0_u8; 512];

    assert_eq!(
        receive_pending_frame(&mut io, Aic8800Product::Aic8800D80, &mut output),
        Err(AicResponseError::OutputTooShort {
            required: 1024,
            available: 512,
        })
    );
    assert_eq!(io.events, vec![Event::ReadRegister(1, 0x04)]);
}

#[test]
fn config_response_waits_one_millisecond_and_validates_expected_id() {
    let mut io = FakeIo::new([0, 1]);
    io.frame = config_response_frame(0x0401, &[0x00, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3]);
    let mut output = [0_u8; 512];

    let response = read_config_response(
        &mut io,
        Aic8800Product::Aic8800D80,
        0x0401,
        &mut output,
        6000,
    )
    .unwrap();

    assert_eq!(response.id, 0x0401);
    assert_eq!(
        response.parameter,
        &[0x00, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3]
    );
    assert!(io.events.contains(&Event::DelayMs(1)));
}

#[test]
fn config_response_timeout_uses_the_requested_millisecond_bound() {
    let mut io = FakeIo::new([]);
    let mut output = [0_u8; 512];

    assert_eq!(
        read_config_response(&mut io, Aic8800Product::Aic8800D80, 0x0401, &mut output, 2,),
        Err(AicResponseError::Timeout { milliseconds: 2 })
    );
    assert_eq!(
        io.events,
        vec![
            Event::ReadRegister(1, 0x04),
            Event::DelayMs(1),
            Event::ReadRegister(1, 0x04),
            Event::DelayMs(1),
            Event::ReadRegister(1, 0x04),
        ]
    );
}

#[test]
fn config_response_skips_data_confirmation_from_an_earlier_read() {
    let mut io = FakeIo::new([1, 1]);
    io.frames.push_back(transport_frame(0x12, &[1, 0, 0, 0]));
    io.frames
        .push_back(config_response_frame(0x0025, &[0, 7, 0, 0]));
    let mut output = [0_u8; 512];

    let response =
        read_config_response(&mut io, Aic8800Product::Aic8800D80, 0x0025, &mut output, 2).unwrap();

    assert_eq!(response.id, 0x0025);
    assert_eq!(response.parameter, &[0, 7, 0, 0]);
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::ReadFifo(..)))
            .count(),
        2
    );
}

#[test]
fn config_response_finds_expected_response_after_data_confirmation_in_aggregate() {
    let mut io = FakeIo::new([1]);
    let confirmation = transport_frame_bytes(0x12, &[1, 0, 0, 0]);
    let response = config_response_frame_bytes(0x0025, &[0, 7, 0, 0]);
    let mut aggregate = Vec::with_capacity(512);
    aggregate.extend_from_slice(&confirmation);
    aggregate.extend_from_slice(&response);
    aggregate.resize(512, 0);
    io.frames.push_back(aggregate);
    let mut output = [0_u8; 512];

    let response =
        read_config_response(&mut io, Aic8800Product::Aic8800D80, 0x0025, &mut output, 2).unwrap();

    assert_eq!(response.id, 0x0025);
    assert_eq!(response.parameter, &[0, 7, 0, 0]);
}

fn config_response_frame(id: u16, parameter: &[u8]) -> Vec<u8> {
    let mut frame = config_response_frame_bytes(id, parameter);
    frame.resize(512, 0);
    frame
}

fn config_response_frame_bytes(id: u16, parameter: &[u8]) -> Vec<u8> {
    let packet_length = 12 + parameter.len();
    let mut frame = vec![0_u8; 4 + packet_length];
    frame[0..2].copy_from_slice(&(packet_length as u16).to_le_bytes());
    frame[2] = 0x11;
    frame[4..6].copy_from_slice(&id.to_le_bytes());
    frame[10..12].copy_from_slice(&(parameter.len() as u16).to_le_bytes());
    frame[16..16 + parameter.len()].copy_from_slice(parameter);
    frame
}

fn transport_frame(message_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = transport_frame_bytes(message_type, payload);
    frame.resize(512, 0);
    frame
}

fn transport_frame_bytes(message_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0_u8; 4 + payload.len()];
    frame[0..2].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    frame[2] = message_type;
    frame[4..].copy_from_slice(payload);
    frame
}
