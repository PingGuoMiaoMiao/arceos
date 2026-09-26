use std::collections::VecDeque;

use axdriver_aic8800::sdio::AicCommandIo;
use axdriver_aic8800::tx::{
    DataSendError, DataTransmitError, EapolTransmitParameters, build_d80_eapol_data_transfer,
    send_d80_data_transfer,
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
fn builds_the_fixed_sdk_d80_fullmac_non_adma_eapol_transfer() {
    let mut output = [0xa5; 512];
    let payload = [0x02, 0x03, 0x00, 0x00];

    let lengths = build_d80_eapol_data_transfer(
        &mut output,
        EapolTransmitParameters {
            destination_address: [0x92, 0xf0, 0x52, 0x3f, 0xfa, 0x04],
            source_address: [0x38, 0x7a, 0xcc, 0x98, 0xe6, 0x46],
            interface_index: 0,
            station_index: 0,
            confirmation_index: 0,
        },
        &payload,
    )
    .unwrap();

    assert_eq!(lengths.frame_length, 36);
    assert_eq!(lengths.transfer_length, 512);
    assert_eq!(&output[..4], &[0x20, 0x00, 0x01, 0x44]);
    assert_eq!(
        &output[4..32],
        &[
            0x04, 0x00, // packet_len
            0x00, 0x00, // flags_ext
            0x00, 0x00, 0x00, 0x80, // hostid: confirmation requested, index 0
            0x92, 0xf0, 0x52, 0x3f, 0xfa, 0x04, // destination
            0x38, 0x7a, 0xcc, 0x98, 0xe6, 0x46, // source
            0x88, 0x8e, // EAPOL EtherType bytes
            0x01, // RWNX_HWQ_BE
            0x00, // TID 0
            0x00, // VIF index
            0x00, // AP station index
            0x00, 0x00, // TX flags
        ]
    );
    assert_eq!(&output[32..36], &payload);
    assert!(output[36..].iter().all(|byte| *byte == 0));
}

#[test]
fn rejects_storage_that_cannot_hold_the_complete_512_byte_sdio_transfer() {
    let mut output = [0; 511];
    let error = build_d80_eapol_data_transfer(
        &mut output,
        EapolTransmitParameters {
            destination_address: [0; 6],
            source_address: [0; 6],
            interface_index: 0,
            station_index: 0,
            confirmation_index: 0,
        },
        &[0x02, 0x03, 0x00, 0x00],
    )
    .unwrap_err();

    assert_eq!(
        error,
        DataTransmitError::OutputTooShort {
            required: 512,
            available: 511,
        }
    );
}

#[test]
fn d80_data_send_waits_until_more_than_two_firmware_buffers_are_free() {
    let mut io = FakeIo::new([2, 3]);
    let mut transfer = [0; 512];

    send_d80_data_transfer(&mut io, &mut transfer).unwrap();

    assert_eq!(
        io.events,
        vec![
            Event::Read(1, 0x03),
            Event::DelayUs(200),
            Event::Read(1, 0x03),
            Event::WriteFifo(1, 0x10, 512),
        ]
    );
}

#[test]
fn d80_data_send_rejects_a_non_block_aligned_transfer_before_bus_access() {
    let mut io = FakeIo::new([]);
    let mut transfer = [0; 511];

    assert_eq!(
        send_d80_data_transfer(&mut io, &mut transfer),
        Err(DataSendError::InvalidTransferLength { length: 511 })
    );
    assert!(io.events.is_empty());
}
