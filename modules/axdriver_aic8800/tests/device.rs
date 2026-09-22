use axdriver_aic8800::device::{Aic8800Product, CLASSIC_REGISTERS, SDIO_BLOCK_SIZE, V3_REGISTERS};
use axdriver_aic8800::protocol::CommandHeaderMode;

#[test]
fn recognizes_only_the_ids_listed_by_the_fixed_sdk_driver() {
    assert_eq!(
        Aic8800Product::from_sdio_id(0x5449, 0x0145),
        Some(Aic8800Product::Aic8801)
    );
    assert_eq!(
        Aic8800Product::from_sdio_id(0xc8a1, 0xc08d),
        Some(Aic8800Product::Aic8800Dc)
    );
    assert_eq!(
        Aic8800Product::from_sdio_id(0xc8a1, 0x0082),
        Some(Aic8800Product::Aic8800D80)
    );
    assert_eq!(
        Aic8800Product::from_sdio_id(0xc8a1, 0x2082),
        Some(Aic8800Product::Aic8800D80X2)
    );
    assert_eq!(Aic8800Product::from_sdio_id(0xc8a1, 0x0182), None);
    assert_eq!(Aic8800Product::from_sdio_id(0x0000, 0x0000), None);
}

#[test]
fn maps_products_to_the_driver_register_and_header_rules() {
    assert_eq!(Aic8800Product::Aic8801.registers(), CLASSIC_REGISTERS);
    assert_eq!(Aic8800Product::Aic8800Dc.registers(), CLASSIC_REGISTERS);
    assert_eq!(Aic8800Product::Aic8800D80.registers(), V3_REGISTERS);
    assert_eq!(Aic8800Product::Aic8800D80X2.registers(), V3_REGISTERS);
    assert_eq!(
        Aic8800Product::Aic8801.command_header_mode(),
        CommandHeaderMode::ReservedZero
    );
    assert_eq!(
        Aic8800Product::Aic8800D80.command_header_mode(),
        CommandHeaderMode::Crc8Polynomial0x107
    );
    assert_eq!(SDIO_BLOCK_SIZE, 512);
}

#[test]
fn register_maps_match_the_fixed_sdk_header() {
    assert_eq!(CLASSIC_REGISTERS.byte_mode_length, 0x02);
    assert_eq!(CLASSIC_REGISTERS.interrupt, 0x04);
    assert_eq!(CLASSIC_REGISTERS.flow_control, 0x0a);
    assert_eq!(CLASSIC_REGISTERS.register_block, Some(0x0b));
    assert_eq!(CLASSIC_REGISTERS.byte_mode_enable, 0x11);
    assert_eq!(CLASSIC_REGISTERS.read_fifo, 0x08);
    assert_eq!(CLASSIC_REGISTERS.write_fifo, 0x07);

    assert_eq!(V3_REGISTERS.byte_mode_length, 0x05);
    assert_eq!(V3_REGISTERS.interrupt, 0x00);
    assert_eq!(V3_REGISTERS.flow_control, 0x03);
    assert_eq!(V3_REGISTERS.register_block, None);
    assert_eq!(V3_REGISTERS.byte_mode_enable, 0x07);
    assert_eq!(V3_REGISTERS.read_fifo, 0x0f);
    assert_eq!(V3_REGISTERS.write_fifo, 0x10);
}
