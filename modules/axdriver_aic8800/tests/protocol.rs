use axdriver_aic8800::protocol::{
    CommandHeaderMode, ConfigResponse, ProtocolError, build_command_frame, crc8_polynomial_0x107,
    finalize_command_transfer,
};

#[test]
fn crc8_matches_the_d80_command_header_rule() {
    assert_eq!(crc8_polynomial_0x107(&[0x0c, 0x00, 0x11]), 0x8d);
}

#[test]
fn builds_a_zero_checksum_command_frame() {
    let mut output = [0xa5; 64];
    let length = build_command_frame(
        &mut output,
        CommandHeaderMode::ReservedZero,
        0x040b,
        1,
        100,
        &[0x44, 0x33, 0x22, 0x11],
    )
    .unwrap();

    assert_eq!(length, 20);
    assert_eq!(
        &output[..length],
        &[
            0x10, 0x00, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0x04, 0x01, 0x00, 0x64, 0x00,
            0x04, 0x00, 0x44, 0x33, 0x22, 0x11,
        ]
    );
}

#[test]
fn builds_a_crc8_command_frame() {
    let mut output = [0; 64];
    let length = build_command_frame(
        &mut output,
        CommandHeaderMode::Crc8Polynomial0x107,
        0x040b,
        1,
        100,
        &[],
    )
    .unwrap();

    assert_eq!(length, 16);
    assert_eq!(&output[..4], &[0x0c, 0x00, 0x11, 0x8d]);
}

#[test]
fn rejects_a_frame_larger_than_the_fixed_sdk_command_buffer() {
    let params = [0u8; 1521];
    let mut output = [0u8; 1600];
    assert_eq!(
        build_command_frame(
            &mut output,
            CommandHeaderMode::ReservedZero,
            0x0400,
            1,
            100,
            &params,
        ),
        Err(ProtocolError::FrameTooLong { length: 1537 })
    );
}

#[test]
fn rejects_an_output_buffer_that_is_too_short() {
    let mut output = [0; 15];
    assert_eq!(
        build_command_frame(
            &mut output,
            CommandHeaderMode::ReservedZero,
            0x0400,
            1,
            100,
            &[],
        ),
        Err(ProtocolError::OutputTooShort {
            required: 16,
            available: 15,
        })
    );
}

#[test]
fn rejects_a_parameter_length_that_does_not_fit_the_lmac_field() {
    let params = [0u8; u16::MAX as usize + 1];
    let mut output = [0u8; 32];
    assert_eq!(
        build_command_frame(
            &mut output,
            CommandHeaderMode::ReservedZero,
            0x0400,
            1,
            100,
            &params,
        ),
        Err(ProtocolError::ParameterTooLong {
            length: u16::MAX as usize + 1,
        })
    );
}

#[test]
fn parses_a_config_command_response() {
    let frame = [
        0x10, 0x00, 0x11, 0x00, 0x0c, 0x04, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x50, 0x4c, 0x41,
        0x43, 0x78, 0x56, 0x34, 0x12,
    ];

    assert_eq!(
        ConfigResponse::parse(&frame).unwrap(),
        ConfigResponse {
            id: 0x040c,
            parameter: &[0x78, 0x56, 0x34, 0x12],
        }
    );
}

#[test]
fn rejects_non_command_response_frames() {
    let frame = [0x00, 0x00, 0x12, 0x00];
    assert_eq!(
        ConfigResponse::parse(&frame),
        Err(ProtocolError::UnexpectedFrameType { value: 0x12 })
    );
}

#[test]
fn rejects_a_truncated_response_parameter() {
    let frame = [
        0x10, 0x00, 0x11, 0x00, 0x0c, 0x04, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x50, 0x4c, 0x41,
        0x43, 0x78, 0x56, 0x34, 0x12,
    ];
    assert_eq!(
        ConfigResponse::parse(&frame),
        Err(ProtocolError::TruncatedFrame {
            required: 21,
            available: 20,
        })
    );
}

#[test]
fn command_transfer_adds_the_sdk_link_tail_and_rounds_to_512_bytes() {
    let mut output = [0xa5; 1536];
    let frame_length = build_command_frame(
        &mut output,
        CommandHeaderMode::ReservedZero,
        0x040b,
        1,
        100,
        &[],
    )
    .unwrap();

    let transfer_length = finalize_command_transfer(&mut output, frame_length).unwrap();

    assert_eq!(frame_length, 16);
    assert_eq!(transfer_length, 512);
    assert!(
        output[frame_length..transfer_length]
            .iter()
            .all(|byte| *byte == 0)
    );
}

#[test]
fn command_transfer_does_not_append_a_tail_to_an_existing_512_byte_frame() {
    let mut output = [0xa5; 1536];
    output[..512].fill(0x3c);

    let transfer_length = finalize_command_transfer(&mut output, 512).unwrap();

    assert_eq!(transfer_length, 512);
    assert!(output[..512].iter().all(|byte| *byte == 0x3c));
}

#[test]
fn sdk_tail_rule_uses_the_next_block_when_tail_reaches_a_block_boundary() {
    let mut output = [0xa5; 1536];

    let transfer_length = finalize_command_transfer(&mut output, 508).unwrap();

    assert_eq!(transfer_length, 1024);
    assert!(output[508..1024].iter().all(|byte| *byte == 0));
}

#[test]
fn command_transfer_aligns_the_frame_to_four_bytes_before_the_tail() {
    let mut output = [0xa5; 1536];

    let transfer_length = finalize_command_transfer(&mut output, 513).unwrap();

    assert_eq!(transfer_length, 1024);
    assert!(output[513..1024].iter().all(|byte| *byte == 0));
}

#[test]
fn command_transfer_rejects_short_output_storage() {
    let mut output = [0_u8; 511];

    assert_eq!(
        finalize_command_transfer(&mut output, 16),
        Err(ProtocolError::OutputTooShort {
            required: 512,
            available: 511,
        })
    );
}
