pub use axmodel_mushroom_yolov5::SourceImageMeta;
use axmodel_mushroom_yolov5::SourceImageMetaError;
pub use axmodel_mushroom_yolov5::crc32_ieee;

pub const MODEL_WIDTH: u32 = axmodel_mushroom_yolov5::MODEL_WIDTH as u32;
pub const MODEL_HEIGHT: u32 = axmodel_mushroom_yolov5::MODEL_HEIGHT as u32;
pub const CHANNEL_COUNT: usize = 3;
pub const HEADER_LENGTH: usize = 40;
pub const PAYLOAD_LENGTH: usize = MODEL_WIDTH as usize * MODEL_HEIGHT as usize * CHANNEL_COUNT;
pub const TOTAL_LENGTH: usize = HEADER_LENGTH + PAYLOAD_LENGTH;
pub const VERSION: u16 = 1;
pub const MAGIC: [u8; 4] = *b"ARIM";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeError {
    TotalLength {
        actual: usize,
    },
    Magic,
    Version {
        actual: u16,
    },
    HeaderLength {
        actual: u16,
    },
    SourceDimensions {
        width: u32,
        height: u32,
    },
    UnrepresentableLetterbox {
        width: u32,
        height: u32,
    },
    Letterbox {
        expected_width: u32,
        expected_height: u32,
        expected_pad_x: u32,
        expected_pad_y: u32,
    },
    PayloadLength {
        actual: u32,
    },
    PayloadCrc32 {
        expected: u32,
        actual: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhoneImageEnvelopeV1<'a> {
    pub meta: SourceImageMeta,
    pub payload: &'a [u8],
}

impl<'a> PhoneImageEnvelopeV1<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, EnvelopeError> {
        if bytes.len() != TOTAL_LENGTH {
            return Err(EnvelopeError::TotalLength {
                actual: bytes.len(),
            });
        }
        if bytes[..4] != MAGIC {
            return Err(EnvelopeError::Magic);
        }

        let version = read_u16(bytes, 4);
        if version != VERSION {
            return Err(EnvelopeError::Version { actual: version });
        }
        let header_length = read_u16(bytes, 6);
        if header_length as usize != HEADER_LENGTH {
            return Err(EnvelopeError::HeaderLength {
                actual: header_length,
            });
        }

        let meta = SourceImageMeta {
            source_width: read_u32(bytes, 8),
            source_height: read_u32(bytes, 12),
            resized_width: read_u32(bytes, 16),
            resized_height: read_u32(bytes, 20),
            pad_x: read_u32(bytes, 24),
            pad_y: read_u32(bytes, 28),
        };
        let expected_meta =
            SourceImageMeta::centered_letterbox(meta.source_width, meta.source_height).map_err(
                |error| match error {
                    SourceImageMetaError::ZeroDimension { width, height } => {
                        EnvelopeError::SourceDimensions { width, height }
                    }
                    SourceImageMetaError::UnrepresentableLetterbox { width, height } => {
                        EnvelopeError::UnrepresentableLetterbox { width, height }
                    }
                },
            )?;
        if meta != expected_meta {
            return Err(EnvelopeError::Letterbox {
                expected_width: expected_meta.resized_width,
                expected_height: expected_meta.resized_height,
                expected_pad_x: expected_meta.pad_x,
                expected_pad_y: expected_meta.pad_y,
            });
        }

        let payload_length = read_u32(bytes, 32);
        if payload_length as usize != PAYLOAD_LENGTH {
            return Err(EnvelopeError::PayloadLength {
                actual: payload_length,
            });
        }
        let payload = &bytes[HEADER_LENGTH..];
        let expected_crc32 = read_u32(bytes, 36);
        let actual_crc32 = crc32_ieee(payload);
        if actual_crc32 != expected_crc32 {
            return Err(EnvelopeError::PayloadCrc32 {
                expected: expected_crc32,
                actual: actual_crc32,
            });
        }

        Ok(Self { meta, payload })
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
