#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CisPointer(u32);

impl CisPointer {
    pub const fn from_bytes(bytes: [u8; 3]) -> Self {
        Self(bytes[0] as u32 | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16)
    }

    pub const fn address(self) -> u32 {
        self.0
    }
}

pub const fn parse_manufacturer_id(data: &[u8]) -> Option<(u16, u16)> {
    if data.len() < 4 {
        return None;
    }
    let vendor = data[0] as u16 | (data[1] as u16) << 8;
    let device = data[2] as u16 | (data[3] as u16) << 8;
    Some((vendor, device))
}

pub const fn parse_function_extension(data: &[u8]) -> Option<u16> {
    if data.len() < 14 || data[0] != 0x01 {
        return None;
    }
    Some(data[12] as u16 | (data[13] as u16) << 8)
}
