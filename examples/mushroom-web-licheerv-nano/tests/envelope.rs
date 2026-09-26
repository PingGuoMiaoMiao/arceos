use arceos_mushroom_web_licheerv_nano::envelope::{
    EnvelopeError, HEADER_LENGTH, MODEL_HEIGHT, MODEL_WIDTH, PAYLOAD_LENGTH, PhoneImageEnvelopeV1,
    SourceImageMeta, TOTAL_LENGTH, crc32_ieee,
};

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap())
}

fn valid_envelope() -> std::vec::Vec<u8> {
    let mut bytes = std::vec![0u8; TOTAL_LENGTH];
    bytes[..4].copy_from_slice(b"ARIM");
    write_u16(&mut bytes, 4, 1);
    write_u16(&mut bytes, 6, HEADER_LENGTH as u16);
    write_u32(&mut bytes, 8, 1280);
    write_u32(&mut bytes, 12, 720);
    write_u32(&mut bytes, 16, 640);
    write_u32(&mut bytes, 20, 360);
    write_u32(&mut bytes, 24, 0);
    write_u32(&mut bytes, 28, 140);
    write_u32(&mut bytes, 32, PAYLOAD_LENGTH as u32);
    let checksum = crc32_ieee(&bytes[HEADER_LENGTH..]);
    write_u32(&mut bytes, 36, checksum);
    bytes
}

#[test]
fn fixed_sizes_match_the_design() {
    assert_eq!(MODEL_WIDTH, 640);
    assert_eq!(MODEL_HEIGHT, 640);
    assert_eq!(HEADER_LENGTH, 40);
    assert_eq!(PAYLOAD_LENGTH, 1_228_800);
    assert_eq!(TOTAL_LENGTH, 1_228_840);
}

#[test]
fn ieee_crc32_matches_the_standard_check_value() {
    assert_eq!(crc32_ieee(b"123456789"), 0xcbf4_3926);
}

#[test]
fn parses_a_complete_envelope_without_copying_the_payload() {
    let bytes = valid_envelope();
    let envelope = PhoneImageEnvelopeV1::parse(&bytes).unwrap();
    assert_eq!(
        envelope.meta,
        SourceImageMeta {
            source_width: 1280,
            source_height: 720,
            resized_width: 640,
            resized_height: 360,
            pad_x: 0,
            pad_y: 140,
        }
    );
    assert_eq!(envelope.payload.len(), PAYLOAD_LENGTH);
    assert_eq!(envelope.payload.as_ptr(), bytes[HEADER_LENGTH..].as_ptr());
    assert_eq!(envelope.payload_crc32, read_u32(&bytes, 36));
}

#[test]
fn rejects_every_fixed_header_field_when_it_is_wrong() {
    let mut bytes = valid_envelope();
    bytes[0] = b'X';
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::Magic)
    );

    let mut bytes = valid_envelope();
    write_u16(&mut bytes, 4, 2);
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::Version { actual: 2 })
    );

    let mut bytes = valid_envelope();
    write_u16(&mut bytes, 6, 39);
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::HeaderLength { actual: 39 })
    );

    let mut bytes = valid_envelope();
    write_u32(&mut bytes, 32, 1);
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::PayloadLength { actual: 1 })
    );
}

#[test]
fn rejects_wrong_total_length_before_reading_the_payload() {
    let bytes = std::vec![0u8; HEADER_LENGTH - 1];
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::TotalLength {
            actual: HEADER_LENGTH - 1,
        })
    );
}

#[test]
fn rejects_zero_source_dimensions() {
    let mut bytes = valid_envelope();
    write_u32(&mut bytes, 8, 0);
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::SourceDimensions {
            width: 0,
            height: 720,
        })
    );
}

#[test]
fn rejects_resize_and_padding_that_do_not_match_the_preprocessing_rule() {
    let mut bytes = valid_envelope();
    write_u32(&mut bytes, 20, 359);
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::Letterbox {
            expected_width: 640,
            expected_height: 360,
            expected_pad_x: 0,
            expected_pad_y: 140,
        })
    );
}

#[test]
fn calculates_portrait_letterbox_dimensions_with_integer_flooring() {
    assert_eq!(
        SourceImageMeta::centered_letterbox(720, 1280).unwrap(),
        SourceImageMeta {
            source_width: 720,
            source_height: 1280,
            resized_width: 360,
            resized_height: 640,
            pad_x: 140,
            pad_y: 0,
        }
    );
}

#[test]
fn rejects_a_corrupted_payload() {
    let mut bytes = valid_envelope();
    let expected = read_u32(&bytes, 36);
    bytes[HEADER_LENGTH + 17] = 1;
    assert_eq!(
        PhoneImageEnvelopeV1::parse(&bytes),
        Err(EnvelopeError::PayloadCrc32 {
            expected,
            actual: crc32_ieee(&bytes[HEADER_LENGTH..]),
        })
    );
}
