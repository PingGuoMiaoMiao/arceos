use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::protocol::{ConfigResponse, ProtocolError};
use crate::response::{AicResponseError, AicResponseIo, receive_pending_frame};
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const SCANU_TASK_ID: u16 = 4;
pub const SCANU_START_REQUEST: u16 = SCANU_TASK_ID << 10;
pub const SCANU_START_CONFIRM: u16 = SCANU_START_REQUEST + 1;
pub const SCANU_RESULT_INDICATION: u16 = SCANU_START_REQUEST + 4;
pub const SCANU_START_ADDITIONAL_CONFIRM: u16 = SCANU_START_REQUEST + 9;
pub const MM_CHANNEL_SURVEY_INDICATION: u16 = 79;

const SCAN_PARAMETER_SIZE: usize = 376;
const MAC_CHANNEL_DEFINITION_SIZE: usize = 6;
const BSSID_OFFSET: usize = 352;
const INTERFACE_INDEX_OFFSET: usize = 366;
const CHANNEL_COUNT_OFFSET: usize = 367;
const SSID_COUNT_OFFSET: usize = 368;
const NO_CCK_OFFSET: usize = 369;
const DURATION_OFFSET: usize = 372;
const SCAN_RESULT_HEADER_SIZE: usize = 12;
const SCAN_COMPLETION_SIZE: usize = 3;
const CHANNEL_SURVEY_SIZE: usize = 12;
const MANAGEMENT_FIXED_FIELDS_SIZE: usize = 36;
const MANAGEMENT_TYPE_SUBTYPE_MASK: u16 = 0x00fc;
const PROBE_RESPONSE_TYPE_SUBTYPE: u16 = 0x0050;
const BEACON_TYPE_SUBTYPE: u16 = 0x0080;
const SSID_INFORMATION_ELEMENT_ID: u8 = 0;
const BOARD_CHANNEL_POWER_DBM: u8 = 20;

const CHANNELS_2GHZ: [u16; 14] = [
    2412, 2417, 2422, 2427, 2432, 2437, 2442, 2447, 2452, 2457, 2462, 2467, 2472, 2484,
];
const CHANNELS_5GHZ: [u16; 25] = [
    5180, 5200, 5220, 5240, 5260, 5280, 5300, 5320, 5500, 5520, 5540, 5560, 5580, 5600, 5620, 5640,
    5660, 5680, 5700, 5720, 5745, 5765, 5785, 5805, 5825,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanEncodeError {
    pub required: usize,
    pub available: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanDecodeError {
    TruncatedResult { required: usize, available: usize },
    TruncatedCompletion { required: usize, available: usize },
    TruncatedChannelSurvey { required: usize, available: usize },
    TruncatedAggregateFrame { required: usize, available: usize },
    TruncatedManagementFrame { required: usize, available: usize },
    UnsupportedManagementFrame { frame_control: u16 },
    TruncatedInformationElement { required: usize, available: usize },
    MissingSsidInformationElement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanResult<'a> {
    pub length: u16,
    pub frame_control: u16,
    pub center_frequency_mhz: u16,
    pub band: u8,
    pub station_index: u8,
    pub interface_index: u8,
    pub rssi_dbm: i8,
    pub frame: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanCompletion {
    pub interface_index: u8,
    pub status: u8,
    pub result_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelSurvey {
    pub frequency_mhz: u16,
    pub noise_dbm: i8,
    pub channel_time_ms: u32,
    pub channel_busy_time_ms: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BssDescription<'a> {
    pub bssid: [u8; 6],
    pub beacon_interval: u16,
    pub capability: u16,
    pub ssid: &'a [u8],
    pub information_elements: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanEvent<'a> {
    Result(ScanResult<'a>),
    ChannelSurvey(ChannelSurvey),
    Complete(ScanCompletion),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicScanError<E> {
    Encode(ScanEncodeError),
    Decode(ScanDecodeError),
    Transaction(AicTransactionError<E>),
    Receive(AicResponseError<E>),
    Protocol(ProtocolError),
    UnexpectedMessageId { actual: u16 },
}

fn encode_channel(output: &mut [u8], index: usize, frequency: u16, band: u8) {
    let offset = index * MAC_CHANNEL_DEFINITION_SIZE;
    output[offset..offset + 2].copy_from_slice(&frequency.to_le_bytes());
    output[offset + 2] = band;
    output[offset + 3] = 0;
    output[offset + 4] = BOARD_CHANNEL_POWER_DBM;
    output[offset + 5] = 0;
}

pub fn encode_d80_full_scan_parameters(
    output: &mut [u8],
    interface_index: u8,
) -> Result<usize, ScanEncodeError> {
    if output.len() < SCAN_PARAMETER_SIZE {
        return Err(ScanEncodeError {
            required: SCAN_PARAMETER_SIZE,
            available: output.len(),
        });
    }

    let output = &mut output[..SCAN_PARAMETER_SIZE];
    output.fill(0);
    for (index, frequency) in CHANNELS_2GHZ.iter().copied().enumerate() {
        encode_channel(output, index, frequency, 0);
    }
    for (index, frequency) in CHANNELS_5GHZ.iter().copied().enumerate() {
        encode_channel(output, CHANNELS_2GHZ.len() + index, frequency, 1);
    }

    output[BSSID_OFFSET..BSSID_OFFSET + 6].fill(0xff);
    output[INTERFACE_INDEX_OFFSET] = interface_index;
    output[CHANNEL_COUNT_OFFSET] = (CHANNELS_2GHZ.len() + CHANNELS_5GHZ.len()) as u8;
    output[SSID_COUNT_OFFSET] = 0;
    output[NO_CCK_OFFSET] = 0;
    output[DURATION_OFFSET..DURATION_OFFSET + 4].copy_from_slice(&0_u32.to_le_bytes());
    Ok(SCAN_PARAMETER_SIZE)
}

pub fn decode_scan_result(parameter: &[u8]) -> Result<ScanResult<'_>, ScanDecodeError> {
    if parameter.len() < SCAN_RESULT_HEADER_SIZE {
        return Err(ScanDecodeError::TruncatedResult {
            required: SCAN_RESULT_HEADER_SIZE,
            available: parameter.len(),
        });
    }
    let length = u16::from_le_bytes(parameter[0..2].try_into().unwrap());
    let required = SCAN_RESULT_HEADER_SIZE + usize::from(length);
    if parameter.len() < required {
        return Err(ScanDecodeError::TruncatedResult {
            required,
            available: parameter.len(),
        });
    }

    Ok(ScanResult {
        length,
        frame_control: u16::from_le_bytes(parameter[2..4].try_into().unwrap()),
        center_frequency_mhz: u16::from_le_bytes(parameter[4..6].try_into().unwrap()),
        band: parameter[6],
        station_index: parameter[7],
        interface_index: parameter[8],
        rssi_dbm: parameter[9] as i8,
        frame: &parameter[SCAN_RESULT_HEADER_SIZE..required],
    })
}

pub fn decode_scan_start_confirmation(parameter: &[u8]) -> Result<ScanCompletion, ScanDecodeError> {
    if parameter.len() < SCAN_COMPLETION_SIZE {
        return Err(ScanDecodeError::TruncatedCompletion {
            required: SCAN_COMPLETION_SIZE,
            available: parameter.len(),
        });
    }
    Ok(ScanCompletion {
        interface_index: parameter[0],
        status: parameter[1],
        result_count: parameter[2],
    })
}

pub fn decode_channel_survey(parameter: &[u8]) -> Result<ChannelSurvey, ScanDecodeError> {
    if parameter.len() < CHANNEL_SURVEY_SIZE {
        return Err(ScanDecodeError::TruncatedChannelSurvey {
            required: CHANNEL_SURVEY_SIZE,
            available: parameter.len(),
        });
    }
    Ok(ChannelSurvey {
        frequency_mhz: u16::from_le_bytes(parameter[0..2].try_into().unwrap()),
        noise_dbm: parameter[2] as i8,
        channel_time_ms: u32::from_le_bytes(parameter[4..8].try_into().unwrap()),
        channel_busy_time_ms: u32::from_le_bytes(parameter[8..12].try_into().unwrap()),
    })
}

pub fn find_information_element(
    information_elements: &[u8],
    requested_id: u8,
) -> Result<Option<&[u8]>, ScanDecodeError> {
    let mut offset = 0;
    while offset < information_elements.len() {
        if information_elements.len() - offset < 2 {
            return Err(ScanDecodeError::TruncatedInformationElement {
                required: offset + 2,
                available: information_elements.len(),
            });
        }
        let element_id = information_elements[offset];
        let element_length = usize::from(information_elements[offset + 1]);
        let required = offset + 2 + element_length;
        if information_elements.len() < required {
            return Err(ScanDecodeError::TruncatedInformationElement {
                required,
                available: information_elements.len(),
            });
        }
        if element_id == requested_id {
            return Ok(Some(&information_elements[offset..required]));
        }
        offset = required;
    }
    Ok(None)
}

pub fn parse_bss_description(frame: &[u8]) -> Result<BssDescription<'_>, ScanDecodeError> {
    if frame.len() < MANAGEMENT_FIXED_FIELDS_SIZE {
        return Err(ScanDecodeError::TruncatedManagementFrame {
            required: MANAGEMENT_FIXED_FIELDS_SIZE,
            available: frame.len(),
        });
    }
    let frame_control = u16::from_le_bytes(frame[0..2].try_into().unwrap());
    let type_subtype = frame_control & MANAGEMENT_TYPE_SUBTYPE_MASK;
    if type_subtype != PROBE_RESPONSE_TYPE_SUBTYPE && type_subtype != BEACON_TYPE_SUBTYPE {
        return Err(ScanDecodeError::UnsupportedManagementFrame { frame_control });
    }

    let information_elements = &frame[MANAGEMENT_FIXED_FIELDS_SIZE..];
    let ssid_element = find_information_element(information_elements, SSID_INFORMATION_ELEMENT_ID)?
        .ok_or(ScanDecodeError::MissingSsidInformationElement)?;
    Ok(BssDescription {
        bssid: frame[16..22].try_into().unwrap(),
        beacon_interval: u16::from_le_bytes(frame[32..34].try_into().unwrap()),
        capability: u16::from_le_bytes(frame[34..36].try_into().unwrap()),
        ssid: &ssid_element[2..],
        information_elements,
    })
}

pub struct AicScanClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    parameter_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
    aggregate_length: usize,
    aggregate_offset: usize,
}

impl<'a, I> AicScanClient<'a, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    pub fn new(
        io: &'a mut I,
        product: Aic8800Product,
        parameter_storage: &'a mut [u8],
        transmit_storage: &'a mut [u8],
        receive_storage: &'a mut [u8],
        response_timeout_ms: u32,
    ) -> Self {
        Self {
            io,
            product,
            parameter_storage,
            transmit_storage,
            receive_storage,
            response_timeout_ms,
            aggregate_length: 0,
            aggregate_offset: 0,
        }
    }

    pub fn start_d80_full_scan(
        &mut self,
        interface_index: u8,
    ) -> Result<(), AicScanError<<I as AicCommandIo>::Error>> {
        let length = encode_d80_full_scan_parameters(self.parameter_storage, interface_index)
            .map_err(AicScanError::Encode)?;
        execute_config_command(
            self.io,
            self.product,
            SCANU_START_REQUEST,
            SCANU_START_ADDITIONAL_CONFIRM,
            SCANU_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicScanError::Transaction)?;
        Ok(())
    }

    pub fn next_event(
        &mut self,
    ) -> Result<ScanEvent<'_>, AicScanError<<I as AicCommandIo>::Error>> {
        for elapsed_ms in 0..=self.response_timeout_ms {
            if self.aggregate_offset < self.aggregate_length {
                let (start, end, next_offset) = match next_aggregate_frame_bounds(
                    self.receive_storage,
                    self.aggregate_length,
                    self.aggregate_offset,
                )
                .map_err(AicScanError::Decode)?
                {
                    Some(bounds) => bounds,
                    None => {
                        self.aggregate_offset = self.aggregate_length;
                        continue;
                    }
                };
                self.aggregate_offset = next_offset;
                let response = ConfigResponse::parse(&self.receive_storage[start..end])
                    .map_err(AicScanError::Protocol)?;
                return decode_scan_event(response);
            }

            if let Some(length) = receive_pending_frame(self.io, self.product, self.receive_storage)
                .map_err(AicScanError::Receive)?
            {
                self.aggregate_length = length;
                self.aggregate_offset = 0;
                continue;
            }
            if elapsed_ms < self.response_timeout_ms {
                AicResponseIo::delay_ms(self.io, 1);
            }
        }
        Err(AicScanError::Receive(AicResponseError::Timeout {
            milliseconds: self.response_timeout_ms,
        }))
    }
}

fn next_aggregate_frame_bounds(
    input: &[u8],
    aggregate_length: usize,
    offset: usize,
) -> Result<Option<(usize, usize, usize)>, ScanDecodeError> {
    if offset >= aggregate_length {
        return Ok(None);
    }
    if aggregate_length - offset < 2 {
        return Err(ScanDecodeError::TruncatedAggregateFrame {
            required: offset + 2,
            available: aggregate_length,
        });
    }
    let packet_length = usize::from(u16::from_le_bytes(
        input[offset..offset + 2].try_into().unwrap(),
    ));
    if packet_length == 0 {
        return Ok(None);
    }
    let frame_end = offset + 4 + packet_length;
    if frame_end > aggregate_length {
        return Err(ScanDecodeError::TruncatedAggregateFrame {
            required: frame_end,
            available: aggregate_length,
        });
    }
    let next_offset = offset + 4 + ((packet_length + 3) & !3);
    Ok(Some((offset, frame_end, next_offset)))
}

fn decode_scan_event<E>(response: ConfigResponse<'_>) -> Result<ScanEvent<'_>, AicScanError<E>> {
    match response.id {
        SCANU_RESULT_INDICATION => decode_scan_result(response.parameter)
            .map(ScanEvent::Result)
            .map_err(AicScanError::Decode),
        SCANU_START_CONFIRM => decode_scan_start_confirmation(response.parameter)
            .map(ScanEvent::Complete)
            .map_err(AicScanError::Decode),
        MM_CHANNEL_SURVEY_INDICATION => decode_channel_survey(response.parameter)
            .map(ScanEvent::ChannelSurvey)
            .map_err(AicScanError::Decode),
        actual => Err(AicScanError::UnexpectedMessageId { actual }),
    }
}
