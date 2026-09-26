const STATION_NONCE_LENGTH: usize = 32;
const CHECKSUM_LENGTH: usize = 4;
const MINIMUM_PASSPHRASE_LENGTH: usize = 8;
const MAXIMUM_PASSPHRASE_LENGTH: usize = 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WifiCredentials<'a> {
    pub passphrase: &'a [u8],
    pub station_nonce: [u8; STATION_NONCE_LENGTH],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiCredentialError {
    MissingPassphraseLength,
    InvalidPassphraseLength { length: usize },
    TruncatedFrame { required: usize, available: usize },
    ChecksumMismatch { expected: u32, actual: u32 },
}

pub fn wifi_credential_wire_length(passphrase_length: u8) -> Result<usize, WifiCredentialError> {
    let passphrase_length = usize::from(passphrase_length);
    if !(MINIMUM_PASSPHRASE_LENGTH..=MAXIMUM_PASSPHRASE_LENGTH).contains(&passphrase_length) {
        return Err(WifiCredentialError::InvalidPassphraseLength {
            length: passphrase_length,
        });
    }
    Ok(1 + passphrase_length + STATION_NONCE_LENGTH + CHECKSUM_LENGTH)
}

pub fn parse_wifi_credentials(frame: &[u8]) -> Result<WifiCredentials<'_>, WifiCredentialError> {
    let passphrase_length = *frame
        .first()
        .ok_or(WifiCredentialError::MissingPassphraseLength)?;
    let required = wifi_credential_wire_length(passphrase_length)?;
    if frame.len() < required {
        return Err(WifiCredentialError::TruncatedFrame {
            required,
            available: frame.len(),
        });
    }

    let passphrase_end = 1 + usize::from(passphrase_length);
    let nonce_end = passphrase_end + STATION_NONCE_LENGTH;
    let expected = u32::from_le_bytes(frame[nonce_end..nonce_end + 4].try_into().unwrap());
    let actual = crc32_ieee(&frame[..nonce_end]);
    if actual != expected {
        return Err(WifiCredentialError::ChecksumMismatch { expected, actual });
    }

    Ok(WifiCredentials {
        passphrase: &frame[1..passphrase_end],
        station_nonce: frame[passphrase_end..nonce_end].try_into().unwrap(),
    })
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
