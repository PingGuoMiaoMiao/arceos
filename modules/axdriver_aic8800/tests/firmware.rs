use axdriver_aic8800::firmware::{
    DBG_MEM_BLOCK_WRITE_CONFIRM, DBG_MEM_BLOCK_WRITE_REQUEST, DBG_MEM_READ_CONFIRM,
    DBG_MEM_READ_REQUEST, DBG_MEM_WRITE_CONFIRM, DBG_MEM_WRITE_REQUEST, DBG_START_APP_CONFIRM,
    DBG_START_APP_REQUEST, DebugMemoryReadError, DebugStartAppError, FirmwareBlockError,
    MAX_FIRMWARE_BLOCK_SIZE, decode_debug_memory_read_confirmation,
    decode_debug_start_app_confirmation, encode_debug_memory_read_parameters,
    encode_debug_memory_write_parameters, encode_debug_start_app_parameters,
    encode_firmware_block_write_parameters,
};

#[test]
fn debug_message_ids_follow_the_fixed_sdk_enum_order() {
    assert_eq!(DBG_MEM_BLOCK_WRITE_REQUEST, 0x040b);
    assert_eq!(DBG_MEM_BLOCK_WRITE_CONFIRM, 0x040c);
    assert_eq!(DBG_MEM_READ_REQUEST, 0x0400);
    assert_eq!(DBG_MEM_READ_CONFIRM, 0x0401);
    assert_eq!(DBG_MEM_WRITE_REQUEST, 0x0402);
    assert_eq!(DBG_MEM_WRITE_CONFIRM, 0x0403);
    assert_eq!(DBG_START_APP_REQUEST, 0x040d);
    assert_eq!(DBG_START_APP_CONFIRM, 0x040e);
}

#[test]
fn debug_memory_read_request_and_confirmation_use_little_endian_fields() {
    let mut request = [0_u8; 4];

    encode_debug_memory_read_parameters(&mut request, 0x4050_0000).unwrap();
    let value = decode_debug_memory_read_confirmation(
        &[0x00, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3],
        0x4050_0000,
    )
    .unwrap();

    assert_eq!(request, [0x00, 0x00, 0x50, 0x40]);
    assert_eq!(value, 0xf3c7_8820);
}

#[test]
fn debug_memory_read_confirmation_rejects_a_different_address() {
    assert_eq!(
        decode_debug_memory_read_confirmation(
            &[0x04, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3],
            0x4050_0000,
        ),
        Err(DebugMemoryReadError::AddressMismatch {
            expected: 0x4050_0000,
            actual: 0x4050_0004,
        })
    );
}

#[test]
fn encodes_the_firmware_block_write_parameter_in_little_endian() {
    let mut output = [0u8; 32];
    let length = encode_firmware_block_write_parameters(
        &mut output,
        0x0012_0000,
        &[0x01, 0x02, 0x03, 0x04, 0x05],
    )
    .unwrap();

    assert_eq!(length, 13);
    assert_eq!(
        &output[..length],
        &[
            0x00, 0x00, 0x12, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        ]
    );
}

#[test]
fn rejects_a_firmware_block_larger_than_the_driver_structure() {
    let input = [0u8; MAX_FIRMWARE_BLOCK_SIZE + 1];
    let mut output = [0u8; MAX_FIRMWARE_BLOCK_SIZE + 8];
    assert_eq!(
        encode_firmware_block_write_parameters(&mut output, 0, &input),
        Err(FirmwareBlockError::BlockTooLong {
            length: MAX_FIRMWARE_BLOCK_SIZE + 1,
        })
    );
}

#[test]
fn debug_memory_write_request_uses_two_little_endian_words() {
    let mut output = [0_u8; 8];

    let length = encode_debug_memory_write_parameters(&mut output, 0x4050_0150, 1).unwrap();

    assert_eq!(length, 8);
    assert_eq!(output, [0x50, 0x01, 0x50, 0x40, 1, 0, 0, 0]);
}

#[test]
fn debug_start_app_request_and_confirmation_use_the_sdk_layout() {
    let mut output = [0_u8; 8];

    let length = encode_debug_start_app_parameters(&mut output, 0x0012_0000, 1).unwrap();
    let boot_status = decode_debug_start_app_confirmation(&[0x34, 0x12, 0, 0]).unwrap();

    assert_eq!(length, 8);
    assert_eq!(output, [0, 0, 0x12, 0, 1, 0, 0, 0]);
    assert_eq!(boot_status, 0x1234);
}

#[test]
fn debug_start_app_confirmation_rejects_a_truncated_status() {
    assert_eq!(
        decode_debug_start_app_confirmation(&[0, 0, 0]),
        Err(DebugStartAppError::TruncatedConfirmation {
            required: 4,
            available: 3,
        })
    );
}
