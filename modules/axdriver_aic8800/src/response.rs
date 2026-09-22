use crate::data::{
    SDIO_CONFIG_COMMAND_RESPONSE_TYPE, SDIO_CONFIG_TYPE_MASK, SDIO_RECEIVE_HEADER_LENGTH,
};
use crate::device::{Aic8800Product, SDIO_BLOCK_SIZE};
use crate::protocol::{ConfigResponse, ProtocolError};

const FUNCTION_1: u8 = 1;
const FUNCTION_2: u8 = 2;
const D80_OTHER_INTERRUPT: u8 = 1 << 7;
const D80_FUNCTION_1_BYTE_MODE_STATUS: u8 = 120;
const D80_FUNCTION_2_BYTE_MODE_STATUS: u8 = 127;

pub trait AicResponseIo {
    type Error;

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error>;
    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error>;
    fn read_fifo(
        &mut self,
        function: u8,
        address: u32,
        output: &mut [u8],
    ) -> Result<(), Self::Error>;
    fn delay_ms(&mut self, milliseconds: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicResponseError<E> {
    Io(E),
    UnsupportedProduct(Aic8800Product),
    OutputTooShort { required: usize, available: usize },
    Protocol(ProtocolError),
    UnexpectedResponseId { expected: u16, actual: u16 },
    Timeout { milliseconds: u32 },
}

pub fn receive_pending_frame<I: AicResponseIo>(
    io: &mut I,
    product: Aic8800Product,
    output: &mut [u8],
) -> Result<Option<usize>, AicResponseError<I::Error>> {
    if !matches!(
        product,
        Aic8800Product::Aic8800D80 | Aic8800Product::Aic8800D80X2
    ) {
        return Err(AicResponseError::UnsupportedProduct(product));
    }

    let registers = product.registers();
    let status_register = registers
        .miscellaneous_interrupt_status
        .expect("D80 register map includes miscellaneous interrupt status");
    let status = io
        .read_register(FUNCTION_1, status_register as u32)
        .map_err(AicResponseError::Io)?;
    if status == 0 {
        return Ok(None);
    }

    if status & D80_OTHER_INTERRUPT != 0 {
        let pending = io
            .read_register(FUNCTION_1, registers.sleep_or_pending as u32)
            .map_err(AicResponseError::Io)?;
        io.write_register(
            FUNCTION_1,
            registers.sleep_or_pending as u32,
            pending & !0x01,
        )
        .map_err(AicResponseError::Io)?;
    }

    let function_two = status | (1 << 3) > D80_FUNCTION_1_BYTE_MODE_STATUS;
    let (function, length) = if function_two {
        if status == D80_FUNCTION_2_BYTE_MODE_STATUS {
            (
                FUNCTION_2,
                read_byte_mode_length(io, registers.byte_mode_length as u32)?,
            )
        } else {
            (FUNCTION_2, (status as usize & 0x07) * SDIO_BLOCK_SIZE)
        }
    } else if status == D80_FUNCTION_1_BYTE_MODE_STATUS {
        (
            FUNCTION_1,
            read_byte_mode_length(io, registers.byte_mode_length as u32)?,
        )
    } else {
        (FUNCTION_1, (status as usize & 0x7f) * SDIO_BLOCK_SIZE)
    };

    if length == 0 {
        return Ok(None);
    }
    if output.len() < length {
        return Err(AicResponseError::OutputTooShort {
            required: length,
            available: output.len(),
        });
    }
    io.read_fifo(function, registers.read_fifo as u32, &mut output[..length])
        .map_err(AicResponseError::Io)?;
    Ok(Some(length))
}

pub fn read_config_response<'a, I: AicResponseIo>(
    io: &mut I,
    product: Aic8800Product,
    expected_id: u16,
    output: &'a mut [u8],
    timeout_ms: u32,
) -> Result<ConfigResponse<'a>, AicResponseError<I::Error>> {
    let mut elapsed_ms = 0;
    loop {
        if let Some(length) = receive_pending_frame(io, product, output)? {
            let mut offset = 0;
            while let Some((start, end, next_offset, message_type)) =
                next_aggregate_frame_bounds(output, length, offset)?
            {
                offset = next_offset;
                if message_type != SDIO_CONFIG_COMMAND_RESPONSE_TYPE {
                    continue;
                }
                let response_id = ConfigResponse::parse(&output[start..end])
                    .map_err(AicResponseError::Protocol)?
                    .id;
                if response_id == expected_id {
                    return ConfigResponse::parse(&output[start..end])
                        .map_err(AicResponseError::Protocol);
                }
            }
            if elapsed_ms >= timeout_ms {
                break;
            }
            elapsed_ms += 1;
            continue;
        }
        if elapsed_ms >= timeout_ms {
            break;
        }
        io.delay_ms(1);
        elapsed_ms += 1;
    }
    Err(AicResponseError::Timeout {
        milliseconds: timeout_ms,
    })
}

fn next_aggregate_frame_bounds<E>(
    input: &[u8],
    aggregate_length: usize,
    offset: usize,
) -> Result<Option<(usize, usize, usize, u8)>, AicResponseError<E>> {
    if offset >= aggregate_length {
        return Ok(None);
    }
    if aggregate_length - offset < 4 {
        return Err(AicResponseError::Protocol(ProtocolError::TruncatedFrame {
            required: offset + 4,
            available: aggregate_length,
        }));
    }
    let packet_length = usize::from(u16::from_le_bytes([input[offset], input[offset + 1]]));
    if packet_length == 0 {
        return Ok(None);
    }
    let message_type = input[offset + 2] & 0x7f;
    let unaligned_frame_length = if message_type & SDIO_CONFIG_TYPE_MASK == SDIO_CONFIG_TYPE_MASK {
        4 + packet_length
    } else {
        SDIO_RECEIVE_HEADER_LENGTH + packet_length
    };
    let frame_end =
        offset
            .checked_add(unaligned_frame_length)
            .ok_or(AicResponseError::Protocol(ProtocolError::TruncatedFrame {
                required: usize::MAX,
                available: aggregate_length,
            }))?;
    if frame_end > aggregate_length {
        return Err(AicResponseError::Protocol(ProtocolError::TruncatedFrame {
            required: frame_end,
            available: aggregate_length,
        }));
    }
    let next_offset = offset
        .checked_add((unaligned_frame_length + 3) & !3)
        .ok_or(AicResponseError::Protocol(ProtocolError::TruncatedFrame {
            required: usize::MAX,
            available: aggregate_length,
        }))?;
    Ok(Some((offset, frame_end, next_offset, message_type)))
}

fn read_byte_mode_length<I: AicResponseIo>(
    io: &mut I,
    address: u32,
) -> Result<usize, AicResponseError<I::Error>> {
    let words = io
        .read_register(FUNCTION_1, address)
        .map_err(AicResponseError::Io)?;
    Ok(words as usize * 4)
}
