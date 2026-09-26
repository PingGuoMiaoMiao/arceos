use crate::protocol::crc8_polynomial_0x107;
use crate::{device::Aic8800Product, sdio::AicCommandIo};

const SDIO_HEADER_SIZE: usize = 4;
const FULLMAC_HOST_DESCRIPTOR_SIZE: usize = 28;
const TX_ALIGNMENT: usize = 4;
const LINK_TAIL_SIZE: usize = 4;
const SDIO_BLOCK_SIZE: usize = 512;
const SDIO_DATA_FRAME_TYPE: u8 = 0x01;
const EAPOL_ETHERTYPE: u16 = 0x888e;
const RWNX_HWQ_BE: u8 = 1;
const EAPOL_TID: u8 = 0;
const CONFIRMATION_REQUESTED: u32 = 1 << 31;
const FUNCTION_1: u8 = 1;
const DATA_FLOW_CONTROL_THRESHOLD: u8 = 2;
const FLOW_CONTROL_RETRY_COUNT: u32 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EapolTransmitParameters {
    pub destination_address: [u8; 6],
    pub source_address: [u8; 6],
    pub interface_index: u8,
    pub station_index: u8,
    pub confirmation_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EthernetTransmitParameters {
    pub destination_address: [u8; 6],
    pub source_address: [u8; 6],
    pub ether_type: u16,
    pub interface_index: u8,
    pub station_index: u8,
    pub confirmation_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataTransferLengths {
    pub frame_length: usize,
    pub transfer_length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTransmitError {
    PayloadTooLong { maximum: usize, actual: usize },
    OutputTooShort { required: usize, available: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataSendError<E> {
    Io(E),
    InvalidTransferLength { length: usize },
    InsufficientFlowControl { buffer_count: u8 },
}

pub fn build_d80_eapol_data_transfer(
    output: &mut [u8],
    parameters: EapolTransmitParameters,
    payload: &[u8],
) -> Result<DataTransferLengths, DataTransmitError> {
    build_d80_ethernet_data_transfer(
        output,
        EthernetTransmitParameters {
            destination_address: parameters.destination_address,
            source_address: parameters.source_address,
            ether_type: EAPOL_ETHERTYPE,
            interface_index: parameters.interface_index,
            station_index: parameters.station_index,
            confirmation_index: parameters.confirmation_index,
        },
        payload,
    )
}

pub fn build_d80_ethernet_data_transfer(
    output: &mut [u8],
    parameters: EthernetTransmitParameters,
    payload: &[u8],
) -> Result<DataTransferLengths, DataTransmitError> {
    let maximum_payload_length = 0x0fff - FULLMAC_HOST_DESCRIPTOR_SIZE;
    if payload.len() > maximum_payload_length {
        return Err(DataTransmitError::PayloadTooLong {
            maximum: maximum_payload_length,
            actual: payload.len(),
        });
    }

    let header_payload_length = FULLMAC_HOST_DESCRIPTOR_SIZE + payload.len();
    let frame_length = SDIO_HEADER_SIZE + header_payload_length;
    let aligned_length = (frame_length + TX_ALIGNMENT - 1) & !(TX_ALIGNMENT - 1);
    let transfer_length = if aligned_length % SDIO_BLOCK_SIZE == 0 {
        aligned_length
    } else {
        let length_with_tail = aligned_length + LINK_TAIL_SIZE;
        (length_with_tail + SDIO_BLOCK_SIZE - 1) & !(SDIO_BLOCK_SIZE - 1)
    };
    if output.len() < transfer_length {
        return Err(DataTransmitError::OutputTooShort {
            required: transfer_length,
            available: output.len(),
        });
    }

    output[..transfer_length].fill(0);
    output[0] = header_payload_length as u8;
    output[1] = ((header_payload_length >> 8) & 0x0f) as u8;
    output[2] = SDIO_DATA_FRAME_TYPE;
    output[3] = crc8_polynomial_0x107(&output[..3]);

    let descriptor = SDIO_HEADER_SIZE;
    output[descriptor..descriptor + 2].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    output[descriptor + 4..descriptor + 8]
        .copy_from_slice(&(CONFIRMATION_REQUESTED | parameters.confirmation_index).to_le_bytes());
    output[descriptor + 8..descriptor + 14].copy_from_slice(&parameters.destination_address);
    output[descriptor + 14..descriptor + 20].copy_from_slice(&parameters.source_address);
    output[descriptor + 20..descriptor + 22].copy_from_slice(&parameters.ether_type.to_be_bytes());
    output[descriptor + 22] = RWNX_HWQ_BE;
    output[descriptor + 23] = EAPOL_TID;
    output[descriptor + 24] = parameters.interface_index;
    output[descriptor + 25] = parameters.station_index;

    let payload_start = SDIO_HEADER_SIZE + FULLMAC_HOST_DESCRIPTOR_SIZE;
    output[payload_start..payload_start + payload.len()].copy_from_slice(payload);

    Ok(DataTransferLengths {
        frame_length,
        transfer_length,
    })
}

pub fn send_d80_data_transfer<I: AicCommandIo>(
    io: &mut I,
    transfer: &mut [u8],
) -> Result<(), DataSendError<I::Error>> {
    if transfer.is_empty() || transfer.len() % SDIO_BLOCK_SIZE != 0 {
        return Err(DataSendError::InvalidTransferLength {
            length: transfer.len(),
        });
    }

    let product = Aic8800Product::Aic8800D80;
    let flow_control_register = product.registers().flow_control as u32;
    let mut count = 0;
    let buffer_count = loop {
        let value = io
            .read_register(FUNCTION_1, flow_control_register)
            .map_err(DataSendError::Io)?;
        if value > DATA_FLOW_CONTROL_THRESHOLD {
            break value;
        }
        if count >= FLOW_CONTROL_RETRY_COUNT {
            return Err(DataSendError::InsufficientFlowControl {
                buffer_count: value,
            });
        }

        count += 1;
        if count < 30 {
            io.delay_us(200);
        } else if count < 40 {
            io.delay_ms(2);
        } else {
            io.delay_ms(10);
        }
    };
    debug_assert!(buffer_count > DATA_FLOW_CONTROL_THRESHOLD);

    io.write_fifo(FUNCTION_1, product.registers().write_fifo as u32, transfer)
        .map_err(DataSendError::Io)
}
