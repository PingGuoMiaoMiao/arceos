pub const SDIO_RECEIVE_HEADER_LENGTH: usize = 60;
pub const SDIO_CONFIG_TYPE_MASK: u8 = 0x10;
pub const SDIO_CONFIG_COMMAND_RESPONSE_TYPE: u8 = 0x11;
pub const EAPOL_ETHERTYPE: u16 = 0x888e;

const IEEE80211_MINIMUM_DATA_HEADER_LENGTH: usize = 24;
const IEEE80211_QOS_CONTROL_LENGTH: usize = 2;
const IEEE80211_HT_CONTROL_LENGTH: usize = 4;
const LLC_SNAP_HEADER: [u8; 6] = [0xaa, 0xaa, 0x03, 0, 0, 0];
const LLC_SNAP_LENGTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceivedEthernetFrame<'a> {
    pub destination: [u8; 6],
    pub source: [u8; 6],
    pub ether_type: u16,
    pub payload: &'a [u8],
    pub qos: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EapolPacket<'a> {
    pub protocol_version: u8,
    pub packet_type: u8,
    pub body: &'a [u8],
    pub trailing: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataDecodeError {
    Truncated { required: usize, available: usize },
    ConfigFrame { message_type: u8 },
    UnsupportedFrameControl { frame_control: u16 },
    UnsupportedDistributionSystemBits { value: u8 },
    AggregatedMsduUnsupported,
    InvalidLlcSnapHeader,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EapolDecodeError {
    Truncated { required: usize, available: usize },
}

pub fn decode_sdio_data_packet(
    packet: &[u8],
) -> Result<ReceivedEthernetFrame<'_>, DataDecodeError> {
    require_length(packet, SDIO_RECEIVE_HEADER_LENGTH)?;
    let message_type = packet[2] & 0x7f;
    if message_type & SDIO_CONFIG_TYPE_MASK == SDIO_CONFIG_TYPE_MASK {
        return Err(DataDecodeError::ConfigFrame { message_type });
    }

    let frame_length = usize::from(u16::from_le_bytes(packet[0..2].try_into().unwrap()));
    let required = SDIO_RECEIVE_HEADER_LENGTH + frame_length;
    require_length(packet, required)?;
    let frame = &packet[SDIO_RECEIVE_HEADER_LENGTH..required];
    require_length(frame, IEEE80211_MINIMUM_DATA_HEADER_LENGTH)?;

    let frame_control = u16::from_le_bytes(frame[0..2].try_into().unwrap());
    if frame[0] & 0x0f != 0x08 {
        return Err(DataDecodeError::UnsupportedFrameControl { frame_control });
    }
    let qos = frame[0] & 0x80 == 0x80;
    let mut header_length = IEEE80211_MINIMUM_DATA_HEADER_LENGTH;
    if qos {
        require_length(frame, header_length + IEEE80211_QOS_CONTROL_LENGTH)?;
        if frame[header_length] & 0x80 != 0 {
            return Err(DataDecodeError::AggregatedMsduUnsupported);
        }
        header_length += IEEE80211_QOS_CONTROL_LENGTH;
    }
    if frame[1] & 0x80 != 0 {
        header_length += IEEE80211_HT_CONTROL_LENGTH;
    }
    require_length(frame, header_length + LLC_SNAP_LENGTH)?;

    let distribution_system_bits = frame[1] & 0x03;
    if distribution_system_bits != 0x02 {
        return Err(DataDecodeError::UnsupportedDistributionSystemBits {
            value: distribution_system_bits,
        });
    }
    let destination = frame[4..10].try_into().unwrap();
    let source = frame[16..22].try_into().unwrap();

    if frame[header_length..header_length + LLC_SNAP_HEADER.len()] != LLC_SNAP_HEADER {
        return Err(DataDecodeError::InvalidLlcSnapHeader);
    }
    let ether_type = u16::from_be_bytes(
        frame[header_length + 6..header_length + 8]
            .try_into()
            .unwrap(),
    );
    Ok(ReceivedEthernetFrame {
        destination,
        source,
        ether_type,
        payload: &frame[header_length + LLC_SNAP_LENGTH..],
        qos,
    })
}

pub fn parse_eapol_packet(input: &[u8]) -> Result<EapolPacket<'_>, EapolDecodeError> {
    if input.len() < 4 {
        return Err(EapolDecodeError::Truncated {
            required: 4,
            available: input.len(),
        });
    }
    let body_length = usize::from(u16::from_be_bytes(input[2..4].try_into().unwrap()));
    let required = 4 + body_length;
    if input.len() < required {
        return Err(EapolDecodeError::Truncated {
            required,
            available: input.len(),
        });
    }
    Ok(EapolPacket {
        protocol_version: input[0],
        packet_type: input[1],
        body: &input[4..required],
        trailing: &input[required..],
    })
}

fn require_length(input: &[u8], required: usize) -> Result<(), DataDecodeError> {
    if input.len() < required {
        Err(DataDecodeError::Truncated {
            required,
            available: input.len(),
        })
    } else {
        Ok(())
    }
}
