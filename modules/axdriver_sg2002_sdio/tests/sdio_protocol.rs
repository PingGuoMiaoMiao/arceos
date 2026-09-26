#[path = "../src/cis.rs"]
mod cis;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/registers.rs"]
mod registers;

use cis::{CisPointer, parse_function_extension, parse_manufacturer_id};
use protocol::{Command, ResponseKind, TransferCount, cmd52_argument, cmd53_argument, parse_r4};
use registers::clock_divider;

#[test]
fn clock_divider_never_exceeds_requested_frequency() {
    let initial = clock_divider(375_000_000, 400_000).unwrap();
    assert_eq!(initial.register_bits, 0xd540);
    assert_eq!(initial.actual_hz, 399_786);
    assert!(initial.actual_hz <= 400_000);

    let run = clock_divider(375_000_000, 25_000_000).unwrap();
    assert_eq!(run.register_bits, 0x0800);
    assert_eq!(run.actual_hz, 23_437_500);
    assert!(run.actual_hz <= 25_000_000);
}

#[test]
fn sdhci_command_words_match_linux_response_flags() {
    assert_eq!(Command::new(0, ResponseKind::None).register_word(), 0x0000);
    assert_eq!(Command::new(5, ResponseKind::R4).register_word(), 0x0502);
    assert_eq!(Command::new(3, ResponseKind::R6).register_word(), 0x031a);
    assert_eq!(Command::new(7, ResponseKind::R1).register_word(), 0x071a);
    assert_eq!(Command::new(52, ResponseKind::R5).register_word(), 0x341a);
    assert_eq!(
        Command::new(53, ResponseKind::R5)
            .with_data()
            .register_word(),
        0x353a
    );
}

#[test]
fn cmd52_argument_encodes_read_and_write_fields_exactly() {
    assert_eq!(
        cmd52_argument(false, 1, false, 0x1234, 0x00),
        Some(0x1024_6800)
    );
    assert_eq!(
        cmd52_argument(true, 2, true, 0x1ffff, 0xa5),
        Some(0xabff_fea5)
    );
    assert_eq!(cmd52_argument(false, 8, false, 0, 0), None);
    assert_eq!(cmd52_argument(false, 0, false, 0x20000, 0), None);
}

#[test]
fn cmd53_argument_encodes_byte_and_block_modes_exactly() {
    assert_eq!(
        cmd53_argument(true, 1, false, 0x10, TransferCount::Blocks(2)),
        Some(0x9800_2002)
    );
    assert_eq!(
        cmd53_argument(false, 2, true, 0x0f, TransferCount::Bytes(512)),
        Some(0x2400_1e00)
    );
    assert_eq!(
        cmd53_argument(false, 8, false, 0, TransferCount::Bytes(1)),
        None
    );
    assert_eq!(
        cmd53_argument(false, 1, false, 0x20000, TransferCount::Bytes(1)),
        None
    );
    assert_eq!(
        cmd53_argument(false, 1, false, 0, TransferCount::Bytes(0)),
        None
    );
}

#[test]
fn r4_reports_ready_function_count_and_ocr() {
    let r4 = parse_r4(0xb0ff_8000);
    assert!(r4.ready);
    assert_eq!(r4.function_count, 3);
    assert_eq!(r4.ocr, 0x00ff_8000);
}

#[test]
fn cis_fields_are_little_endian_and_function_block_size_is_exact() {
    assert_eq!(
        CisPointer::from_bytes([0x56, 0x34, 0x01]).address(),
        0x013456
    );
    assert_eq!(
        parse_manufacturer_id(&[0x34, 0x12, 0x78, 0x56]),
        Some((0x1234, 0x5678))
    );

    let mut function_extension = [0_u8; 14];
    function_extension[0] = 0x01;
    function_extension[12] = 0x00;
    function_extension[13] = 0x02;
    assert_eq!(parse_function_extension(&function_extension), Some(512));
    assert_eq!(parse_function_extension(&function_extension[..13]), None);
}
