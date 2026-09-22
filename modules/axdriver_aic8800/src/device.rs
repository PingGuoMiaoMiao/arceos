use crate::protocol::CommandHeaderMode;

pub const SDIO_BLOCK_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Aic8800Product {
    Aic8801,
    Aic8800Dc,
    Aic8800D80,
    Aic8800D80X2,
}

impl Aic8800Product {
    pub const fn from_sdio_id(vendor: u16, device: u16) -> Option<Self> {
        match (vendor, device) {
            (0x5449, 0x0145) => Some(Self::Aic8801),
            (0xc8a1, 0xc08d) => Some(Self::Aic8800Dc),
            (0xc8a1, 0x0082) => Some(Self::Aic8800D80),
            (0xc8a1, 0x2082) => Some(Self::Aic8800D80X2),
            _ => None,
        }
    }

    pub const fn registers(self) -> RegisterMap {
        match self {
            Self::Aic8801 | Self::Aic8800Dc => CLASSIC_REGISTERS,
            Self::Aic8800D80 | Self::Aic8800D80X2 => V3_REGISTERS,
        }
    }

    pub const fn command_header_mode(self) -> CommandHeaderMode {
        match self {
            Self::Aic8801 | Self::Aic8800Dc => CommandHeaderMode::ReservedZero,
            Self::Aic8800D80 | Self::Aic8800D80X2 => CommandHeaderMode::Crc8Polynomial0x107,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterMap {
    pub byte_mode_length: u8,
    pub interrupt: u8,
    pub sleep_or_pending: u8,
    pub wake_or_to_device: u8,
    pub flow_control: u8,
    pub register_block: Option<u8>,
    pub byte_mode_enable: u8,
    pub block_count: Option<u8>,
    pub miscellaneous_interrupt_status: Option<u8>,
    pub read_fifo: u8,
    pub write_fifo: u8,
}

pub const CLASSIC_REGISTERS: RegisterMap = RegisterMap {
    byte_mode_length: 0x02,
    interrupt: 0x04,
    sleep_or_pending: 0x05,
    wake_or_to_device: 0x09,
    flow_control: 0x0a,
    register_block: Some(0x0b),
    byte_mode_enable: 0x11,
    block_count: Some(0x12),
    miscellaneous_interrupt_status: None,
    read_fifo: 0x08,
    write_fifo: 0x07,
};

pub const V3_REGISTERS: RegisterMap = RegisterMap {
    byte_mode_length: 0x05,
    interrupt: 0x00,
    sleep_or_pending: 0x01,
    wake_or_to_device: 0x02,
    flow_control: 0x03,
    register_block: None,
    byte_mode_enable: 0x07,
    block_count: None,
    miscellaneous_interrupt_status: Some(0x04),
    read_fifo: 0x0f,
    write_fifo: 0x10,
};
