const SDHCI_CMD_CRC: u16 = 0x08;
const SDHCI_CMD_INDEX: u16 = 0x10;
const SDHCI_CMD_RESP_NONE: u16 = 0x00;
const SDHCI_CMD_RESP_SHORT: u16 = 0x02;
const SDHCI_CMD_DATA: u16 = 0x20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseKind {
    None,
    R1,
    R4,
    R5,
    R6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command {
    index: u8,
    response: ResponseKind,
    data: bool,
}

impl Command {
    pub const fn new(index: u8, response: ResponseKind) -> Self {
        Self {
            index,
            response,
            data: false,
        }
    }

    pub const fn with_data(mut self) -> Self {
        self.data = true;
        self
    }

    pub const fn register_word(self) -> u16 {
        let response_flags = match self.response {
            ResponseKind::None => SDHCI_CMD_RESP_NONE,
            ResponseKind::R4 => SDHCI_CMD_RESP_SHORT,
            ResponseKind::R1 | ResponseKind::R5 | ResponseKind::R6 => {
                SDHCI_CMD_RESP_SHORT | SDHCI_CMD_CRC | SDHCI_CMD_INDEX
            }
        };
        ((self.index as u16) << 8) | response_flags | if self.data { SDHCI_CMD_DATA } else { 0 }
    }

    pub const fn index(self) -> u8 {
        self.index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferCount {
    Bytes(u16),
    Blocks(u16),
}

pub const fn cmd53_argument(
    write: bool,
    function: u8,
    increment_address: bool,
    address: u32,
    count: TransferCount,
) -> Option<u32> {
    if function > 7 || address > 0x1ffff {
        return None;
    }

    let (block_mode, count_value) = match count {
        TransferCount::Bytes(value) | TransferCount::Blocks(value) if value == 0 || value > 512 => {
            return None;
        }
        TransferCount::Bytes(value) => (false, value),
        TransferCount::Blocks(value) => (true, value),
    };

    let mut argument = (function as u32) << 28 | address << 9;
    if write {
        argument |= 1 << 31;
    }
    if block_mode {
        argument |= 1 << 27;
    }
    if increment_address {
        argument |= 1 << 26;
    }
    argument |= (count_value as u32) & 0x1ff;
    Some(argument)
}

pub const fn cmd52_argument(
    write: bool,
    function: u8,
    read_after_write: bool,
    address: u32,
    value: u8,
) -> Option<u32> {
    if function > 7 || address > 0x1ffff {
        return None;
    }

    let mut argument = (function as u32) << 28 | address << 9 | value as u32;
    if write {
        argument |= 1 << 31;
        if read_after_write {
            argument |= 1 << 27;
        }
    }
    Some(argument)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R4 {
    pub ready: bool,
    pub function_count: u8,
    pub ocr: u32,
}

pub const fn parse_r4(raw: u32) -> R4 {
    R4 {
        ready: raw & (1 << 31) != 0,
        function_count: ((raw >> 28) & 0x7) as u8,
        ocr: raw & 0x00ff_ffff,
    }
}
