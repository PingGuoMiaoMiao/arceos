const PATCH_TABLE_TAG: &[u8; 13] = b"AICBT_PT_TAG\0";
const PATCH_TABLE_PREFIX_SIZE: usize = 16;
const SECTION_NAME_SIZE: usize = 16;
const SECTION_HEADER_SIZE: usize = SECTION_NAME_SIZE + 8;
const PAIR_SIZE: usize = 8;

pub const AICBT_PATCH_INFO_TYPE: u32 = 0;
pub const AICBT_PATCH_BTMODE_TYPE: u32 = 3;
pub const AICBT_PATCH_POWER_ON_TYPE: u32 = 4;
pub const AICBT_PATCH_VERSION_TYPE: u32 = 6;
const PATCH_INFORMATION_WRITE_PAIR_LIMIT: usize = 4;
const POWER_ON_DELAY_US: u32 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D80BtPatchSettings {
    pub hwinfo: i32,
    pub bt_mode: u32,
    pub bt_port: u32,
    pub uart_baud: u32,
    pub uart_flow_control: u32,
    pub low_power_mode: u32,
    pub transmit_power: u32,
}

pub const D80_SDIO_BT_DEFAULT_PATCH_SETTINGS: D80BtPatchSettings = D80BtPatchSettings {
    hwinfo: -1,
    bt_mode: 5,
    bt_port: 1,
    uart_baud: 1_500_000,
    uart_flow_control: 1,
    low_power_mode: 0,
    transmit_power: 0x0000_6f2f,
};

pub trait AicPatchMemory {
    type Error;

    fn write_word(&mut self, address: u32, value: u32) -> Result<(), Self::Error>;
    fn delay_us(&mut self, microseconds: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicPatchApplyError<E> {
    Write {
        section_type: u32,
        pair_index: usize,
        source: E,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicPatchTableError {
    TruncatedTag {
        required: usize,
        available: usize,
    },
    InvalidTag,
    TruncatedSectionHeader {
        offset: usize,
        available: usize,
    },
    TruncatedSectionData {
        offset: usize,
        required: usize,
        available: usize,
    },
    LengthOverflow {
        offset: usize,
    },
    MissingInformationSection,
    UnexpectedInformationType {
        actual: u32,
    },
    MissingInformationPairs {
        required: usize,
        available: usize,
    },
    MissingExtensionPairs {
        declared: u32,
        available: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AicPatchTable<'a> {
    bytes: &'a [u8],
}

impl<'a> AicPatchTable<'a> {
    pub fn sections(&self) -> AicPatchSections<'a> {
        AicPatchSections {
            remaining: &self.bytes[PATCH_TABLE_PREFIX_SIZE..],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AicPatchSection<'a> {
    name: &'a [u8],
    section_type: u32,
    pair_count: usize,
    pair_bytes: &'a [u8],
}

impl<'a> AicPatchSection<'a> {
    pub fn name(&self) -> &'a [u8] {
        self.name
    }

    pub const fn section_type(&self) -> u32 {
        self.section_type
    }

    pub const fn pair_count(&self) -> usize {
        self.pair_count
    }

    pub fn pairs(&self) -> AicPatchPairs<'a> {
        AicPatchPairs {
            remaining: self.pair_bytes,
        }
    }

    pub fn pair(&self, index: usize) -> Option<(u32, u32)> {
        let start = index.checked_mul(PAIR_SIZE)?;
        let pair = self.pair_bytes.get(start..start + PAIR_SIZE)?;
        Some((read_u32(pair, 0), read_u32(pair, 4)))
    }
}

pub struct AicPatchSections<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for AicPatchSections<'a> {
    type Item = AicPatchSection<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let section_type = read_u32(self.remaining, SECTION_NAME_SIZE);
        let declared_pair_count = read_u32(self.remaining, SECTION_NAME_SIZE + 4) as usize;
        let pair_count = if section_type >= 1000 {
            0
        } else {
            declared_pair_count
        };
        let pair_bytes_length = pair_count * PAIR_SIZE;
        let (header, rest) = self.remaining.split_at(SECTION_HEADER_SIZE);
        let (pair_bytes, following) = rest.split_at(pair_bytes_length);
        self.remaining = following;

        Some(AicPatchSection {
            name: &header[..SECTION_NAME_SIZE],
            section_type,
            pair_count,
            pair_bytes,
        })
    }
}

pub struct AicPatchPairs<'a> {
    remaining: &'a [u8],
}

impl Iterator for AicPatchPairs<'_> {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        let pair = self.remaining.get(..PAIR_SIZE)?;
        self.remaining = &self.remaining[PAIR_SIZE..];
        Some((read_u32(pair, 0), read_u32(pair, 4)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AicD80PatchInfo<'a> {
    pub adid_address_info: u32,
    pub adid_address: u32,
    pub patch_address_info: u32,
    pub patch_address: u32,
    pub reset_address: u32,
    pub reset_value: u32,
    pub adid_flag_address: u32,
    pub adid_flag_value: u32,
    pub ext_patch_count_address: u32,
    ext_patch_count: u32,
    ext_patch_bytes: &'a [u8],
}

impl<'a> AicD80PatchInfo<'a> {
    pub const fn ext_patch_count(&self) -> u32 {
        self.ext_patch_count
    }

    pub fn ext_patches(&self) -> AicPatchPairs<'a> {
        AicPatchPairs {
            remaining: self.ext_patch_bytes,
        }
    }
}

pub fn parse_patch_table(bytes: &[u8]) -> Result<AicPatchTable<'_>, AicPatchTableError> {
    if bytes.len() < PATCH_TABLE_TAG.len() {
        return Err(AicPatchTableError::TruncatedTag {
            required: PATCH_TABLE_TAG.len(),
            available: bytes.len(),
        });
    }
    if &bytes[..PATCH_TABLE_TAG.len()] != PATCH_TABLE_TAG {
        return Err(AicPatchTableError::InvalidTag);
    }
    if bytes.len() < PATCH_TABLE_PREFIX_SIZE {
        return Err(AicPatchTableError::TruncatedTag {
            required: PATCH_TABLE_PREFIX_SIZE,
            available: bytes.len(),
        });
    }

    let mut offset = PATCH_TABLE_PREFIX_SIZE;
    while offset < bytes.len() {
        let available = bytes.len() - offset;
        if available < SECTION_HEADER_SIZE {
            return Err(AicPatchTableError::TruncatedSectionHeader { offset, available });
        }

        let section_type = read_u32(bytes, offset + SECTION_NAME_SIZE);
        let declared_pair_count = read_u32(bytes, offset + SECTION_NAME_SIZE + 4) as usize;
        let pair_count = if section_type >= 1000 {
            0
        } else {
            declared_pair_count
        };
        let pair_bytes_length = pair_count
            .checked_mul(PAIR_SIZE)
            .ok_or(AicPatchTableError::LengthOverflow { offset })?;
        let required = SECTION_HEADER_SIZE
            .checked_add(pair_bytes_length)
            .ok_or(AicPatchTableError::LengthOverflow { offset })?;
        if available < required {
            return Err(AicPatchTableError::TruncatedSectionData {
                offset,
                required,
                available,
            });
        }
        offset += required;
    }

    Ok(AicPatchTable { bytes })
}

pub fn parse_d80_patch_info<'a>(
    table: &AicPatchTable<'a>,
) -> Result<AicD80PatchInfo<'a>, AicPatchTableError> {
    let section = table
        .sections()
        .next()
        .ok_or(AicPatchTableError::MissingInformationSection)?;
    if section.section_type != AICBT_PATCH_INFO_TYPE {
        return Err(AicPatchTableError::UnexpectedInformationType {
            actual: section.section_type,
        });
    }
    const REQUIRED_BASE_PAIRS: usize = 5;
    if section.pair_count < REQUIRED_BASE_PAIRS {
        return Err(AicPatchTableError::MissingInformationPairs {
            required: REQUIRED_BASE_PAIRS,
            available: section.pair_count,
        });
    }

    let (adid_address_info, adid_address) = section.pair(0).unwrap();
    let (patch_address_info, patch_address) = section.pair(1).unwrap();
    let (reset_address, reset_value) = section.pair(2).unwrap();
    let (adid_flag_address, adid_flag_value) = section.pair(3).unwrap();
    let (ext_patch_count_address, ext_patch_count) = section.pair(4).unwrap();
    let available_extensions = section.pair_count - REQUIRED_BASE_PAIRS;
    let required_extensions = usize::try_from(ext_patch_count).map_err(|_| {
        AicPatchTableError::MissingExtensionPairs {
            declared: ext_patch_count,
            available: available_extensions,
        }
    })?;
    if available_extensions < required_extensions {
        return Err(AicPatchTableError::MissingExtensionPairs {
            declared: ext_patch_count,
            available: available_extensions,
        });
    }
    let extension_bytes_start = REQUIRED_BASE_PAIRS * PAIR_SIZE;
    let extension_bytes_end = extension_bytes_start + required_extensions * PAIR_SIZE;

    Ok(AicD80PatchInfo {
        adid_address_info,
        adid_address,
        patch_address_info,
        patch_address,
        reset_address,
        reset_value,
        adid_flag_address,
        adid_flag_value,
        ext_patch_count_address,
        ext_patch_count,
        ext_patch_bytes: &section.pair_bytes[extension_bytes_start..extension_bytes_end],
    })
}

pub fn apply_d80_bt_patch_table<M: AicPatchMemory>(
    memory: &mut M,
    table: &AicPatchTable<'_>,
    settings: D80BtPatchSettings,
) -> Result<(), AicPatchApplyError<M::Error>> {
    for section in table.sections() {
        if section.section_type == AICBT_PATCH_VERSION_TYPE {
            continue;
        }
        let pair_count = if section.section_type == AICBT_PATCH_INFO_TYPE {
            section.pair_count.min(PATCH_INFORMATION_WRITE_PAIR_LIMIT)
        } else {
            section.pair_count
        };
        for pair_index in 0..pair_count {
            let (address, original_value) = section.pair(pair_index).unwrap();
            let value = if section.section_type == AICBT_PATCH_BTMODE_TYPE {
                d80_bt_mode_value(pair_index, original_value, settings)
            } else {
                original_value
            };
            memory
                .write_word(address, value)
                .map_err(|source| AicPatchApplyError::Write {
                    section_type: section.section_type,
                    pair_index,
                    source,
                })?;
        }
        if section.section_type == AICBT_PATCH_POWER_ON_TYPE {
            memory.delay_us(POWER_ON_DELAY_US);
        }
    }
    Ok(())
}

fn d80_bt_mode_value(pair_index: usize, original_value: u32, settings: D80BtPatchSettings) -> u32 {
    match pair_index {
        0 => u32::from(settings.hwinfo < 0),
        1 => settings.hwinfo as u32,
        2 => 0,
        3 => settings.bt_mode,
        4 => settings.bt_port,
        5 => settings.uart_baud,
        6 => settings.uart_flow_control,
        7 => settings.low_power_mode,
        8 => settings.transmit_power,
        _ => original_value,
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
