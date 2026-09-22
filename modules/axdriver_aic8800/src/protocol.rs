const SDIO_HEADER_SIZE: usize = 4;
const COMMAND_DUMMY_SIZE: usize = 4;
const LMAC_HEADER_SIZE: usize = 8;
const RESPONSE_HEADER_SIZE: usize = 12;
const TX_ALIGNMENT: usize = 4;
const LINK_TAIL_SIZE: usize = 4;
const SDIO_BLOCK_SIZE: usize = 512;

pub const CONFIG_COMMAND_RESPONSE_TYPE: u8 = 0x11;
pub const MAX_COMMAND_FRAME_SIZE: usize = 1536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandHeaderMode {
    ReservedZero,
    Crc8Polynomial0x107,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    ParameterTooLong { length: usize },
    FrameTooLong { length: usize },
    OutputTooShort { required: usize, available: usize },
    TruncatedFrame { required: usize, available: usize },
    UnexpectedFrameType { value: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigResponse<'a> {
    pub id: u16,
    pub parameter: &'a [u8],
}

impl<'a> ConfigResponse<'a> {
    pub fn parse(frame: &'a [u8]) -> Result<Self, ProtocolError> {
        if frame.len() < SDIO_HEADER_SIZE {
            return Err(ProtocolError::TruncatedFrame {
                required: SDIO_HEADER_SIZE,
                available: frame.len(),
            });
        }

        let frame_type = frame[2] & 0x7f;
        if frame_type != CONFIG_COMMAND_RESPONSE_TYPE {
            return Err(ProtocolError::UnexpectedFrameType { value: frame_type });
        }

        let fixed_size = SDIO_HEADER_SIZE + RESPONSE_HEADER_SIZE;
        if frame.len() < fixed_size {
            return Err(ProtocolError::TruncatedFrame {
                required: fixed_size,
                available: frame.len(),
            });
        }

        let id = u16::from_le_bytes([frame[4], frame[5]]);
        let parameter_length = u16::from_le_bytes([frame[10], frame[11]]) as usize;
        let required = fixed_size + parameter_length;
        if frame.len() < required {
            return Err(ProtocolError::TruncatedFrame {
                required,
                available: frame.len(),
            });
        }

        Ok(Self {
            id,
            parameter: &frame[fixed_size..required],
        })
    }
}

pub fn build_command_frame(
    output: &mut [u8],
    header_mode: CommandHeaderMode,
    id: u16,
    destination_id: u16,
    source_id: u16,
    parameter: &[u8],
) -> Result<usize, ProtocolError> {
    let parameter_length =
        u16::try_from(parameter.len()).map_err(|_| ProtocolError::ParameterTooLong {
            length: parameter.len(),
        })?;
    let required = SDIO_HEADER_SIZE + COMMAND_DUMMY_SIZE + LMAC_HEADER_SIZE + parameter.len();
    if required > MAX_COMMAND_FRAME_SIZE {
        return Err(ProtocolError::FrameTooLong { length: required });
    }
    if output.len() < required {
        return Err(ProtocolError::OutputTooShort {
            required,
            available: output.len(),
        });
    }

    output[..required].fill(0);
    let header_payload_length = LMAC_HEADER_SIZE + parameter.len() + COMMAND_DUMMY_SIZE;
    output[0] = header_payload_length as u8;
    output[1] = ((header_payload_length >> 8) & 0x0f) as u8;
    output[2] = CONFIG_COMMAND_RESPONSE_TYPE;
    output[3] = match header_mode {
        CommandHeaderMode::ReservedZero => 0,
        CommandHeaderMode::Crc8Polynomial0x107 => crc8_polynomial_0x107(&output[..3]),
    };

    let lmac = SDIO_HEADER_SIZE + COMMAND_DUMMY_SIZE;
    output[lmac..lmac + 2].copy_from_slice(&id.to_le_bytes());
    output[lmac + 2..lmac + 4].copy_from_slice(&destination_id.to_le_bytes());
    output[lmac + 4..lmac + 6].copy_from_slice(&source_id.to_le_bytes());
    output[lmac + 6..lmac + 8].copy_from_slice(&parameter_length.to_le_bytes());
    output[lmac + LMAC_HEADER_SIZE..required].copy_from_slice(parameter);

    Ok(required)
}

pub fn finalize_command_transfer(
    output: &mut [u8],
    frame_length: usize,
) -> Result<usize, ProtocolError> {
    if frame_length > output.len() {
        return Err(ProtocolError::OutputTooShort {
            required: frame_length,
            available: output.len(),
        });
    }

    let aligned_length = frame_length
        .checked_add(TX_ALIGNMENT - 1)
        .map(|length| length & !(TX_ALIGNMENT - 1))
        .ok_or(ProtocolError::FrameTooLong {
            length: frame_length,
        })?;
    let transfer_length = if aligned_length % SDIO_BLOCK_SIZE == 0 {
        aligned_length
    } else {
        let length_with_tail =
            aligned_length
                .checked_add(LINK_TAIL_SIZE)
                .ok_or(ProtocolError::FrameTooLong {
                    length: frame_length,
                })?;
        length_with_tail
            .checked_div(SDIO_BLOCK_SIZE)
            .and_then(|blocks| blocks.checked_add(1))
            .and_then(|blocks| blocks.checked_mul(SDIO_BLOCK_SIZE))
            .ok_or(ProtocolError::FrameTooLong {
                length: frame_length,
            })?
    };
    if transfer_length > output.len() {
        return Err(ProtocolError::OutputTooShort {
            required: transfer_length,
            available: output.len(),
        });
    }
    output[frame_length..transfer_length].fill(0);
    Ok(transfer_length)
}

pub fn crc8_polynomial_0x107(input: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in input {
        let mut bit = 0x80u8;
        while bit != 0 {
            if crc & 0x80 != 0 {
                crc = crc.wrapping_mul(2) ^ 0x07;
            } else {
                crc = crc.wrapping_mul(2);
            }
            if byte & bit != 0 {
                crc ^= 0x07;
            }
            bit /= 2;
        }
    }
    crc
}
