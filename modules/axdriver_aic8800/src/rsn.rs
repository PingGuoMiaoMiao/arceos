pub const RSN_INFORMATION_ELEMENT_ID: u8 = 48;
pub const RSN_VERSION: u16 = 1;
pub const RSN_CIPHER_CCMP: [u8; 4] = [0x00, 0x0f, 0xac, 0x04];
pub const RSN_AKM_PSK: [u8; 4] = [0x00, 0x0f, 0xac, 0x02];

const STATION_INFORMATION_ELEMENT_LENGTH: usize = 22;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RsnInformation<'a> {
    pub group_cipher: [u8; 4],
    pub pairwise_cipher_suites: &'a [u8],
    pub authentication_key_management_suites: &'a [u8],
    pub capabilities: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RsnParseError {
    Truncated { required: usize, available: usize },
    UnexpectedElementId { expected: u8, actual: u8 },
    LengthMismatch { declared: usize, actual: usize },
    UnsupportedVersion { expected: u16, actual: u16 },
    EmptyPairwiseCipherList,
    EmptyAuthenticationKeyManagementList,
    LengthOverflow,
    UnexpectedTrailingBytes { count: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RsnBuildError {
    OutputTooShort { required: usize, available: usize },
    UnsupportedGroupCipher { actual: [u8; 4] },
    PairwiseCcmpMissing,
    PskAuthenticationMissing,
}

pub fn parse_rsn_information_element(input: &[u8]) -> Result<RsnInformation<'_>, RsnParseError> {
    require_length(input, 2)?;
    if input[0] != RSN_INFORMATION_ELEMENT_ID {
        return Err(RsnParseError::UnexpectedElementId {
            expected: RSN_INFORMATION_ELEMENT_ID,
            actual: input[0],
        });
    }
    let declared_length = usize::from(input[1]);
    let actual_length = input.len() - 2;
    if declared_length != actual_length {
        return Err(RsnParseError::LengthMismatch {
            declared: declared_length,
            actual: actual_length,
        });
    }

    let mut offset = 2;
    require_length(input, offset + 2)?;
    let version = u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap());
    if version != RSN_VERSION {
        return Err(RsnParseError::UnsupportedVersion {
            expected: RSN_VERSION,
            actual: version,
        });
    }
    offset += 2;

    require_length(input, offset + 4)?;
    let group_cipher = input[offset..offset + 4].try_into().unwrap();
    offset += 4;

    let pairwise_cipher_suites = parse_suite_list(input, &mut offset, true)?;
    let authentication_key_management_suites = parse_suite_list(input, &mut offset, false)?;

    require_length(input, offset + 2)?;
    let capabilities = u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap());
    offset += 2;
    if offset != input.len() {
        return Err(RsnParseError::UnexpectedTrailingBytes {
            count: input.len() - offset,
        });
    }

    Ok(RsnInformation {
        group_cipher,
        pairwise_cipher_suites,
        authentication_key_management_suites,
        capabilities,
    })
}

pub fn build_wpa2_psk_ccmp_station_information_element(
    output: &mut [u8],
    access_point: &RsnInformation<'_>,
) -> Result<usize, RsnBuildError> {
    if output.len() < STATION_INFORMATION_ELEMENT_LENGTH {
        return Err(RsnBuildError::OutputTooShort {
            required: STATION_INFORMATION_ELEMENT_LENGTH,
            available: output.len(),
        });
    }
    if access_point.group_cipher != RSN_CIPHER_CCMP {
        return Err(RsnBuildError::UnsupportedGroupCipher {
            actual: access_point.group_cipher,
        });
    }
    if !contains_suite(access_point.pairwise_cipher_suites, RSN_CIPHER_CCMP) {
        return Err(RsnBuildError::PairwiseCcmpMissing);
    }
    if !contains_suite(
        access_point.authentication_key_management_suites,
        RSN_AKM_PSK,
    ) {
        return Err(RsnBuildError::PskAuthenticationMissing);
    }

    output[..STATION_INFORMATION_ELEMENT_LENGTH].copy_from_slice(&[
        RSN_INFORMATION_ELEMENT_ID,
        20,
        1,
        0,
        RSN_CIPHER_CCMP[0],
        RSN_CIPHER_CCMP[1],
        RSN_CIPHER_CCMP[2],
        RSN_CIPHER_CCMP[3],
        1,
        0,
        RSN_CIPHER_CCMP[0],
        RSN_CIPHER_CCMP[1],
        RSN_CIPHER_CCMP[2],
        RSN_CIPHER_CCMP[3],
        1,
        0,
        RSN_AKM_PSK[0],
        RSN_AKM_PSK[1],
        RSN_AKM_PSK[2],
        RSN_AKM_PSK[3],
        0,
        0,
    ]);
    Ok(STATION_INFORMATION_ELEMENT_LENGTH)
}

fn parse_suite_list<'a>(
    input: &'a [u8],
    offset: &mut usize,
    pairwise: bool,
) -> Result<&'a [u8], RsnParseError> {
    require_length(input, *offset + 2)?;
    let count = usize::from(u16::from_le_bytes(
        input[*offset..*offset + 2].try_into().unwrap(),
    ));
    *offset += 2;
    if count == 0 {
        return if pairwise {
            Err(RsnParseError::EmptyPairwiseCipherList)
        } else {
            Err(RsnParseError::EmptyAuthenticationKeyManagementList)
        };
    }
    let byte_length = count.checked_mul(4).ok_or(RsnParseError::LengthOverflow)?;
    let end = offset
        .checked_add(byte_length)
        .ok_or(RsnParseError::LengthOverflow)?;
    require_length(input, end)?;
    let suites = &input[*offset..end];
    *offset = end;
    Ok(suites)
}

fn contains_suite(suites: &[u8], requested: [u8; 4]) -> bool {
    suites
        .chunks_exact(4)
        .any(|suite| suite == requested.as_slice())
}

fn require_length(input: &[u8], required: usize) -> Result<(), RsnParseError> {
    if input.len() < required {
        Err(RsnParseError::Truncated {
            required,
            available: input.len(),
        })
    } else {
        Ok(())
    }
}
