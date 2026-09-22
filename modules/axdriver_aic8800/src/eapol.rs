use crate::data::{EapolDecodeError, parse_eapol_packet};
use crate::wpa_crypto::Wpa2CcmpPairwiseTransientKey;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

pub const EAPOL_PROTOCOL_VERSION_2: u8 = 2;
pub const EAPOL_KEY_PACKET_TYPE: u8 = 3;
pub const RSN_KEY_DESCRIPTOR_TYPE: u8 = 2;
pub const WPA2_PSK_CCMP_DESCRIPTOR_VERSION: u16 = 2;
pub const WPA2_PSK_CCMP_MESSAGE_1_KEY_INFORMATION: u16 = 0x008a;
pub const WPA2_PSK_CCMP_MESSAGE_2_KEY_INFORMATION: u16 = 0x010a;

const WPA2_PSK_MIC_LENGTH: usize = 16;
const WPA_KEY_FIXED_LENGTH_WITHOUT_MIC: usize = 77;
const WPA_KEY_DATA_LENGTH_FIELD_SIZE: usize = 2;
const WPA2_PSK_KEY_HEADER_LENGTH: usize =
    WPA_KEY_FIXED_LENGTH_WITHOUT_MIC + WPA2_PSK_MIC_LENGTH + WPA_KEY_DATA_LENGTH_FIELD_SIZE;
const WPA_KEY_INFORMATION_OFFSET: usize = 1;
const WPA_KEY_LENGTH_OFFSET: usize = 3;
const WPA_REPLAY_COUNTER_OFFSET: usize = 5;
const WPA_NONCE_OFFSET: usize = 13;
const WPA_KEY_IV_OFFSET: usize = 45;
const WPA_KEY_RSC_OFFSET: usize = 61;
const WPA_KEY_ID_OFFSET: usize = 69;
const WPA_KEY_MIC_OFFSET: usize = 77;
const WPA_KEY_DATA_LENGTH_OFFSET: usize = WPA_KEY_MIC_OFFSET + WPA2_PSK_MIC_LENGTH;
const WPA_KEY_DATA_OFFSET: usize = WPA_KEY_DATA_LENGTH_OFFSET + WPA_KEY_DATA_LENGTH_FIELD_SIZE;
const WPA_KEY_INFORMATION_TYPE_MASK: u16 = 0x0007;
const WPA_KEY_INFORMATION_KEY_TYPE: u16 = 1 << 3;
const WPA_KEY_INFORMATION_KEY_INDEX_MASK: u16 = 0x0030;
const WPA_KEY_INFORMATION_INSTALL: u16 = 1 << 6;
const WPA_KEY_INFORMATION_ACK: u16 = 1 << 7;
const WPA_KEY_INFORMATION_MIC: u16 = 1 << 8;
const WPA_KEY_INFORMATION_SECURE: u16 = 1 << 9;
const WPA_KEY_INFORMATION_REQUEST: u16 = 1 << 11;
const WPA_KEY_INFORMATION_ENCRYPTED_KEY_DATA: u16 = 1 << 12;
const WPA_KEY_INFORMATION_SMK_MESSAGE: u16 = 1 << 13;
const WPA2_PSK_CCMP_MESSAGE_3_REQUIRED_INFORMATION: u16 =
    WPA_KEY_INFORMATION_KEY_TYPE | WPA_KEY_INFORMATION_ACK | WPA_KEY_INFORMATION_MIC;
const WPA2_PSK_CCMP_MESSAGE_3_PROHIBITED_INFORMATION: u16 = WPA_KEY_INFORMATION_KEY_INDEX_MASK
    | WPA_KEY_INFORMATION_REQUEST
    | WPA_KEY_INFORMATION_SMK_MESSAGE;
const EAPOL_HEADER_LENGTH: usize = 4;

type HmacSha1 = Hmac<Sha1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wpa2PskCcmpMessage1<'a> {
    pub protocol_version: u8,
    pub key_information: u16,
    pub descriptor_version: u16,
    pub key_length: u16,
    pub replay_counter: u64,
    pub authenticator_nonce: [u8; 32],
    pub key_iv: [u8; 16],
    pub key_rsc: [u8; 8],
    pub key_id: [u8; 8],
    pub key_mic: [u8; 16],
    pub key_data: &'a [u8],
    pub trailing: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wpa2PskCcmpMessage3<'a> {
    pub protocol_version: u8,
    pub key_information: u16,
    pub descriptor_version: u16,
    pub key_length: u16,
    pub replay_counter: u64,
    pub authenticator_nonce: [u8; 32],
    pub key_iv: [u8; 16],
    pub key_rsc: [u8; 8],
    pub key_id: [u8; 8],
    pub key_mic: [u8; 16],
    pub key_data: &'a [u8],
    pub trailing: &'a [u8],
    pub encrypted_key_data: bool,
    pub install: bool,
    pub secure: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wpa2PskCcmpKeyData {
    pub group_key_index: u8,
    pub group_key_transmit: bool,
    pub group_temporal_key: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EapolKeyDataDecodeError {
    ElementOverflow {
        offset: usize,
        declared: usize,
        available: usize,
    },
    RsnInformationElementMismatch,
    InvalidGroupKeyLength {
        actual: usize,
    },
    MissingRsnInformationElement,
    MissingGroupTemporalKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EapolKeyDecodeError {
    Eapol(EapolDecodeError),
    UnexpectedProtocolVersion { actual: u8 },
    UnexpectedPacketType { actual: u8 },
    TruncatedKeyBody { required: usize, available: usize },
    UnexpectedDescriptorType { actual: u8 },
    UnexpectedDescriptorVersion { actual: u16 },
    UnexpectedMessage1KeyInformation { actual: u16 },
    MissingMessage3KeyInformation { required: u16, actual: u16 },
    ProhibitedMessage3KeyInformation { prohibited: u16, actual: u16 },
    Message3ReplayCounterNotIncreased { previous: u64, actual: u64 },
    Message3AuthenticatorNonceMismatch,
    UnexpectedMessage3KeyLength { expected: u16, actual: u16 },
    Message3MicMismatch,
    KeyDataOverflow { declared: usize, available: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EapolKeyBuildError {
    OutputTooShort { required: usize, available: usize },
    KeyDataTooLong { maximum: usize, actual: usize },
}

pub fn parse_wpa2_psk_ccmp_message_1(
    input: &[u8],
) -> Result<Wpa2PskCcmpMessage1<'_>, EapolKeyDecodeError> {
    let packet = parse_eapol_packet(input).map_err(EapolKeyDecodeError::Eapol)?;
    if packet.protocol_version != EAPOL_PROTOCOL_VERSION_2 {
        return Err(EapolKeyDecodeError::UnexpectedProtocolVersion {
            actual: packet.protocol_version,
        });
    }
    if packet.packet_type != EAPOL_KEY_PACKET_TYPE {
        return Err(EapolKeyDecodeError::UnexpectedPacketType {
            actual: packet.packet_type,
        });
    }
    if packet.body.len() < WPA2_PSK_KEY_HEADER_LENGTH {
        return Err(EapolKeyDecodeError::TruncatedKeyBody {
            required: WPA2_PSK_KEY_HEADER_LENGTH,
            available: packet.body.len(),
        });
    }
    if packet.body[0] != RSN_KEY_DESCRIPTOR_TYPE {
        return Err(EapolKeyDecodeError::UnexpectedDescriptorType {
            actual: packet.body[0],
        });
    }

    let key_information = u16::from_be_bytes(
        packet.body[WPA_KEY_INFORMATION_OFFSET..WPA_KEY_INFORMATION_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    let descriptor_version = key_information & WPA_KEY_INFORMATION_TYPE_MASK;
    if descriptor_version != WPA2_PSK_CCMP_DESCRIPTOR_VERSION {
        return Err(EapolKeyDecodeError::UnexpectedDescriptorVersion {
            actual: descriptor_version,
        });
    }
    if key_information != WPA2_PSK_CCMP_MESSAGE_1_KEY_INFORMATION {
        return Err(EapolKeyDecodeError::UnexpectedMessage1KeyInformation {
            actual: key_information,
        });
    }

    let key_data_length = usize::from(u16::from_be_bytes(
        packet.body[WPA_KEY_DATA_LENGTH_OFFSET..WPA_KEY_DATA_OFFSET]
            .try_into()
            .unwrap(),
    ));
    let available_key_data = packet.body.len() - WPA_KEY_DATA_OFFSET;
    if key_data_length > available_key_data {
        return Err(EapolKeyDecodeError::KeyDataOverflow {
            declared: key_data_length,
            available: available_key_data,
        });
    }

    Ok(Wpa2PskCcmpMessage1 {
        protocol_version: packet.protocol_version,
        key_information,
        descriptor_version,
        key_length: u16::from_be_bytes(
            packet.body[WPA_KEY_LENGTH_OFFSET..WPA_KEY_LENGTH_OFFSET + 2]
                .try_into()
                .unwrap(),
        ),
        replay_counter: u64::from_be_bytes(
            packet.body[WPA_REPLAY_COUNTER_OFFSET..WPA_REPLAY_COUNTER_OFFSET + 8]
                .try_into()
                .unwrap(),
        ),
        authenticator_nonce: packet.body[WPA_NONCE_OFFSET..WPA_NONCE_OFFSET + 32]
            .try_into()
            .unwrap(),
        key_iv: packet.body[WPA_KEY_IV_OFFSET..WPA_KEY_IV_OFFSET + 16]
            .try_into()
            .unwrap(),
        key_rsc: packet.body[WPA_KEY_RSC_OFFSET..WPA_KEY_RSC_OFFSET + 8]
            .try_into()
            .unwrap(),
        key_id: packet.body[WPA_KEY_ID_OFFSET..WPA_KEY_ID_OFFSET + 8]
            .try_into()
            .unwrap(),
        key_mic: packet.body[WPA_KEY_MIC_OFFSET..WPA_KEY_MIC_OFFSET + WPA2_PSK_MIC_LENGTH]
            .try_into()
            .unwrap(),
        key_data: &packet.body[WPA_KEY_DATA_OFFSET..WPA_KEY_DATA_OFFSET + key_data_length],
        trailing: packet.trailing,
    })
}

pub fn parse_wpa2_psk_ccmp_message_3<'a>(
    input: &'a [u8],
    message_1: &Wpa2PskCcmpMessage1<'_>,
    pairwise_transient_key: &Wpa2CcmpPairwiseTransientKey,
) -> Result<Wpa2PskCcmpMessage3<'a>, EapolKeyDecodeError> {
    let packet = parse_eapol_packet(input).map_err(EapolKeyDecodeError::Eapol)?;
    if packet.protocol_version != EAPOL_PROTOCOL_VERSION_2 {
        return Err(EapolKeyDecodeError::UnexpectedProtocolVersion {
            actual: packet.protocol_version,
        });
    }
    if packet.packet_type != EAPOL_KEY_PACKET_TYPE {
        return Err(EapolKeyDecodeError::UnexpectedPacketType {
            actual: packet.packet_type,
        });
    }
    if packet.body.len() < WPA2_PSK_KEY_HEADER_LENGTH {
        return Err(EapolKeyDecodeError::TruncatedKeyBody {
            required: WPA2_PSK_KEY_HEADER_LENGTH,
            available: packet.body.len(),
        });
    }
    if packet.body[0] != RSN_KEY_DESCRIPTOR_TYPE {
        return Err(EapolKeyDecodeError::UnexpectedDescriptorType {
            actual: packet.body[0],
        });
    }

    let key_information = u16::from_be_bytes(
        packet.body[WPA_KEY_INFORMATION_OFFSET..WPA_KEY_INFORMATION_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    let descriptor_version = key_information & WPA_KEY_INFORMATION_TYPE_MASK;
    if descriptor_version != WPA2_PSK_CCMP_DESCRIPTOR_VERSION {
        return Err(EapolKeyDecodeError::UnexpectedDescriptorVersion {
            actual: descriptor_version,
        });
    }
    if key_information & WPA2_PSK_CCMP_MESSAGE_3_REQUIRED_INFORMATION
        != WPA2_PSK_CCMP_MESSAGE_3_REQUIRED_INFORMATION
    {
        return Err(EapolKeyDecodeError::MissingMessage3KeyInformation {
            required: WPA2_PSK_CCMP_MESSAGE_3_REQUIRED_INFORMATION,
            actual: key_information,
        });
    }
    if key_information & WPA2_PSK_CCMP_MESSAGE_3_PROHIBITED_INFORMATION != 0 {
        return Err(EapolKeyDecodeError::ProhibitedMessage3KeyInformation {
            prohibited: WPA2_PSK_CCMP_MESSAGE_3_PROHIBITED_INFORMATION,
            actual: key_information,
        });
    }

    let key_length = u16::from_be_bytes(
        packet.body[WPA_KEY_LENGTH_OFFSET..WPA_KEY_LENGTH_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    if key_length != 16 {
        return Err(EapolKeyDecodeError::UnexpectedMessage3KeyLength {
            expected: 16,
            actual: key_length,
        });
    }
    let replay_counter = u64::from_be_bytes(
        packet.body[WPA_REPLAY_COUNTER_OFFSET..WPA_REPLAY_COUNTER_OFFSET + 8]
            .try_into()
            .unwrap(),
    );
    if replay_counter <= message_1.replay_counter {
        return Err(EapolKeyDecodeError::Message3ReplayCounterNotIncreased {
            previous: message_1.replay_counter,
            actual: replay_counter,
        });
    }
    let authenticator_nonce: [u8; 32] = packet.body[WPA_NONCE_OFFSET..WPA_NONCE_OFFSET + 32]
        .try_into()
        .unwrap();
    if authenticator_nonce != message_1.authenticator_nonce {
        return Err(EapolKeyDecodeError::Message3AuthenticatorNonceMismatch);
    }

    let key_data_length = usize::from(u16::from_be_bytes(
        packet.body[WPA_KEY_DATA_LENGTH_OFFSET..WPA_KEY_DATA_OFFSET]
            .try_into()
            .unwrap(),
    ));
    let available_key_data = packet.body.len() - WPA_KEY_DATA_OFFSET;
    if key_data_length > available_key_data {
        return Err(EapolKeyDecodeError::KeyDataOverflow {
            declared: key_data_length,
            available: available_key_data,
        });
    }

    let key_mic: [u8; WPA2_PSK_MIC_LENGTH] = packet.body
        [WPA_KEY_MIC_OFFSET..WPA_KEY_MIC_OFFSET + WPA2_PSK_MIC_LENGTH]
        .try_into()
        .unwrap();
    let mic_offset = EAPOL_HEADER_LENGTH + WPA_KEY_MIC_OFFSET;
    let authenticated_length = EAPOL_HEADER_LENGTH + packet.body.len();
    let mut mac =
        <HmacSha1 as KeyInit>::new_from_slice(pairwise_transient_key.key_confirmation_key())
            .expect("HMAC-SHA1 accepts a 16-byte KCK");
    mac.update(&input[..mic_offset]);
    mac.update(&[0; WPA2_PSK_MIC_LENGTH]);
    mac.update(&input[mic_offset + WPA2_PSK_MIC_LENGTH..authenticated_length]);
    let digest = mac.finalize().into_bytes();
    let difference = key_mic
        .iter()
        .zip(&digest[..WPA2_PSK_MIC_LENGTH])
        .fold(0_u8, |difference, (actual, expected)| {
            difference | (actual ^ expected)
        });
    if difference != 0 {
        return Err(EapolKeyDecodeError::Message3MicMismatch);
    }

    Ok(Wpa2PskCcmpMessage3 {
        protocol_version: packet.protocol_version,
        key_information,
        descriptor_version,
        key_length,
        replay_counter,
        authenticator_nonce,
        key_iv: packet.body[WPA_KEY_IV_OFFSET..WPA_KEY_IV_OFFSET + 16]
            .try_into()
            .unwrap(),
        key_rsc: packet.body[WPA_KEY_RSC_OFFSET..WPA_KEY_RSC_OFFSET + 8]
            .try_into()
            .unwrap(),
        key_id: packet.body[WPA_KEY_ID_OFFSET..WPA_KEY_ID_OFFSET + 8]
            .try_into()
            .unwrap(),
        key_mic,
        key_data: &packet.body[WPA_KEY_DATA_OFFSET..WPA_KEY_DATA_OFFSET + key_data_length],
        trailing: packet.trailing,
        encrypted_key_data: key_information & WPA_KEY_INFORMATION_ENCRYPTED_KEY_DATA != 0,
        install: key_information & WPA_KEY_INFORMATION_INSTALL != 0,
        secure: key_information & WPA_KEY_INFORMATION_SECURE != 0,
    })
}

pub fn parse_wpa2_psk_ccmp_key_data(
    key_data: &[u8],
    expected_access_point_rsn: &[u8],
) -> Result<Wpa2PskCcmpKeyData, EapolKeyDataDecodeError> {
    const RSN_INFORMATION_ELEMENT_ID: u8 = 0x30;
    const VENDOR_SPECIFIC_INFORMATION_ELEMENT_ID: u8 = 0xdd;
    const GTK_SELECTOR: [u8; 4] = [0x00, 0x0f, 0xac, 0x01];
    const GTK_BODY_LENGTH: usize = 2 + 16;

    let mut offset = 0_usize;
    let mut rsn_found = false;
    let mut group_key = None;
    while key_data.len().saturating_sub(offset) > 1 {
        let element_id = key_data[offset];
        let element_length = usize::from(key_data[offset + 1]);
        if element_id == VENDOR_SPECIFIC_INFORMATION_ELEMENT_ID && element_length == 0 {
            break;
        }
        let total_length = 2 + element_length;
        let available = key_data.len() - offset;
        if total_length > available {
            return Err(EapolKeyDataDecodeError::ElementOverflow {
                offset,
                declared: total_length,
                available,
            });
        }
        let element = &key_data[offset..offset + total_length];
        let body = &element[2..];
        if element_id == RSN_INFORMATION_ELEMENT_ID {
            if element != expected_access_point_rsn {
                return Err(EapolKeyDataDecodeError::RsnInformationElementMismatch);
            }
            rsn_found = true;
        } else if element_id == VENDOR_SPECIFIC_INFORMATION_ELEMENT_ID
            && body.len() >= GTK_SELECTOR.len()
            && body[..GTK_SELECTOR.len()] == GTK_SELECTOR
        {
            let gtk = &body[GTK_SELECTOR.len()..];
            if gtk.len() != GTK_BODY_LENGTH {
                return Err(EapolKeyDataDecodeError::InvalidGroupKeyLength { actual: gtk.len() });
            }
            group_key = Some(Wpa2PskCcmpKeyData {
                group_key_index: gtk[0] & 0x03,
                group_key_transmit: gtk[0] & (1 << 2) != 0,
                group_temporal_key: gtk[2..].try_into().unwrap(),
            });
        }
        offset += total_length;
    }

    if !rsn_found {
        return Err(EapolKeyDataDecodeError::MissingRsnInformationElement);
    }
    group_key.ok_or(EapolKeyDataDecodeError::MissingGroupTemporalKey)
}

pub fn build_wpa2_psk_ccmp_message_2(
    output: &mut [u8],
    message_1: &Wpa2PskCcmpMessage1<'_>,
    station_nonce: [u8; 32],
    station_rsn_information_element: &[u8],
    pairwise_transient_key: &Wpa2CcmpPairwiseTransientKey,
) -> Result<usize, EapolKeyBuildError> {
    if station_rsn_information_element.len() > usize::from(u16::MAX) {
        return Err(EapolKeyBuildError::KeyDataTooLong {
            maximum: usize::from(u16::MAX),
            actual: station_rsn_information_element.len(),
        });
    }
    let body_length = WPA2_PSK_KEY_HEADER_LENGTH + station_rsn_information_element.len();
    let required = EAPOL_HEADER_LENGTH + body_length;
    if output.len() < required {
        return Err(EapolKeyBuildError::OutputTooShort {
            required,
            available: output.len(),
        });
    }

    let frame = &mut output[..required];
    frame.fill(0);
    frame[0] = EAPOL_PROTOCOL_VERSION_2;
    frame[1] = EAPOL_KEY_PACKET_TYPE;
    frame[2..4].copy_from_slice(&(body_length as u16).to_be_bytes());
    let body = &mut frame[EAPOL_HEADER_LENGTH..];
    body[0] = RSN_KEY_DESCRIPTOR_TYPE;
    body[WPA_KEY_INFORMATION_OFFSET..WPA_KEY_INFORMATION_OFFSET + 2]
        .copy_from_slice(&WPA2_PSK_CCMP_MESSAGE_2_KEY_INFORMATION.to_be_bytes());
    body[WPA_KEY_LENGTH_OFFSET..WPA_KEY_LENGTH_OFFSET + 2].copy_from_slice(&0_u16.to_be_bytes());
    body[WPA_REPLAY_COUNTER_OFFSET..WPA_REPLAY_COUNTER_OFFSET + 8]
        .copy_from_slice(&message_1.replay_counter.to_be_bytes());
    body[WPA_NONCE_OFFSET..WPA_NONCE_OFFSET + 32].copy_from_slice(&station_nonce);
    body[WPA_KEY_DATA_LENGTH_OFFSET..WPA_KEY_DATA_OFFSET]
        .copy_from_slice(&(station_rsn_information_element.len() as u16).to_be_bytes());
    body[WPA_KEY_DATA_OFFSET..].copy_from_slice(station_rsn_information_element);

    let mut mac =
        <HmacSha1 as KeyInit>::new_from_slice(pairwise_transient_key.key_confirmation_key())
            .expect("HMAC-SHA1 accepts a 16-byte KCK");
    mac.update(frame);
    let digest = mac.finalize().into_bytes();
    frame[EAPOL_HEADER_LENGTH + WPA_KEY_MIC_OFFSET
        ..EAPOL_HEADER_LENGTH + WPA_KEY_MIC_OFFSET + WPA2_PSK_MIC_LENGTH]
        .copy_from_slice(&digest[..WPA2_PSK_MIC_LENGTH]);
    Ok(required)
}

pub fn build_wpa2_psk_ccmp_message_4(
    output: &mut [u8],
    message_3: &Wpa2PskCcmpMessage3<'_>,
    pairwise_transient_key: &Wpa2CcmpPairwiseTransientKey,
) -> Result<usize, EapolKeyBuildError> {
    let body_length = WPA2_PSK_KEY_HEADER_LENGTH;
    let required = EAPOL_HEADER_LENGTH + body_length;
    if output.len() < required {
        return Err(EapolKeyBuildError::OutputTooShort {
            required,
            available: output.len(),
        });
    }

    let frame = &mut output[..required];
    frame.fill(0);
    frame[0] = message_3.protocol_version;
    frame[1] = EAPOL_KEY_PACKET_TYPE;
    frame[2..4].copy_from_slice(&(body_length as u16).to_be_bytes());
    let body = &mut frame[EAPOL_HEADER_LENGTH..];
    body[0] = RSN_KEY_DESCRIPTOR_TYPE;
    let key_information = (message_3.key_information & WPA_KEY_INFORMATION_SECURE)
        | message_3.descriptor_version
        | WPA_KEY_INFORMATION_KEY_TYPE
        | WPA_KEY_INFORMATION_MIC;
    body[WPA_KEY_INFORMATION_OFFSET..WPA_KEY_INFORMATION_OFFSET + 2]
        .copy_from_slice(&key_information.to_be_bytes());
    body[WPA_KEY_LENGTH_OFFSET..WPA_KEY_LENGTH_OFFSET + 2].copy_from_slice(&0_u16.to_be_bytes());
    body[WPA_REPLAY_COUNTER_OFFSET..WPA_REPLAY_COUNTER_OFFSET + 8]
        .copy_from_slice(&message_3.replay_counter.to_be_bytes());
    body[WPA_KEY_DATA_LENGTH_OFFSET..WPA_KEY_DATA_OFFSET].copy_from_slice(&0_u16.to_be_bytes());

    let mut mac =
        <HmacSha1 as KeyInit>::new_from_slice(pairwise_transient_key.key_confirmation_key())
            .expect("HMAC-SHA1 accepts a 16-byte KCK");
    mac.update(frame);
    let digest = mac.finalize().into_bytes();
    frame[EAPOL_HEADER_LENGTH + WPA_KEY_MIC_OFFSET
        ..EAPOL_HEADER_LENGTH + WPA_KEY_MIC_OFFSET + WPA2_PSK_MIC_LENGTH]
        .copy_from_slice(&digest[..WPA2_PSK_MIC_LENGTH]);
    Ok(required)
}
