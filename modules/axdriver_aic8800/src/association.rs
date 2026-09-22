pub const SM_TASK_ID: u16 = 6;
pub const SM_CONNECT_REQUEST: u16 = SM_TASK_ID << 10;
pub const SM_CONNECT_CONFIRM: u16 = SM_CONNECT_REQUEST + 1;
pub const SM_CONNECT_INDICATION: u16 = SM_CONNECT_REQUEST + 2;

pub const CONTROL_PORT_HOST: u32 = 1 << 0;
pub const WPA_WPA2_IN_USE: u32 = 1 << 3;
pub const CONTROL_PORT_PROTOCOL_EAPOL: [u8; 2] = [0x88, 0x8e];

const CONNECT_PARAMETER_SIZE: usize = 320;
const SSID_MAXIMUM_LENGTH: usize = 32;
const ASSOCIATION_INFORMATION_ELEMENTS_MAXIMUM_LENGTH: usize = 256;
const CONNECT_INDICATION_SIZE: usize = 852;
const CONNECT_INDICATION_INFORMATION_ELEMENTS_OFFSET: usize = 20;
const CONNECT_INDICATION_INFORMATION_ELEMENTS_LENGTH: usize = 800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectParameters<'a> {
    pub ssid: &'a [u8],
    pub bssid: [u8; 6],
    pub frequency_mhz: u16,
    pub band: u8,
    pub channel_flags: u8,
    pub transmit_power_dbm: i8,
    pub association_information_elements: &'a [u8],
    pub interface_index: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectEncodeError {
    OutputTooShort { required: usize, available: usize },
    SsidTooLong { maximum: usize, actual: usize },
    InformationElementsTooLong { maximum: usize, actual: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectDecodeError {
    TruncatedConfirmation { required: usize, available: usize },
    TruncatedIndication { required: usize, available: usize },
    InformationElementsTooLong { maximum: usize, actual: usize },
    TruncatedAggregateFrame { required: usize, available: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectConfirmation {
    pub status: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectIndication<'a> {
    pub status_code: u16,
    pub bssid: [u8; 6],
    pub roamed: bool,
    pub interface_index: u8,
    pub access_point_index: u8,
    pub channel_index: u8,
    pub qos: bool,
    pub admission_control_mask: u8,
    pub association_request_information_elements: &'a [u8],
    pub association_response_information_elements: &'a [u8],
    pub association_id: u16,
    pub band: u8,
    pub center_frequency_mhz: u16,
    pub width: u8,
    pub center_frequency1_mhz: u32,
    pub center_frequency2_mhz: u32,
    pub access_category_parameters: [u32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssociationEvent<'a> {
    Confirmation(ConnectConfirmation),
    Indication(ConnectIndication<'a>),
    Data(ReceivedEthernetFrame<'a>),
    UndecodedData {
        packet: &'a [u8],
        error: DataDecodeError,
    },
    Transport {
        message_type: u8,
    },
    Unrelated {
        message_id: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicAssociationError<E> {
    Encode(ConnectEncodeError),
    Decode(ConnectDecodeError),
    Protocol(ProtocolError),
    Send(AicCommandError<E>),
    Receive(AicResponseError<E>),
    UnexpectedMessageId { actual: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EthernetSendError<E> {
    TruncatedFrame { required: usize, available: usize },
    Encode(DataTransmitError),
    Send(DataSendError<E>),
}

pub fn encode_wpa2_personal_connect_parameters(
    output: &mut [u8],
    parameters: ConnectParameters<'_>,
) -> Result<usize, ConnectEncodeError> {
    if output.len() < CONNECT_PARAMETER_SIZE {
        return Err(ConnectEncodeError::OutputTooShort {
            required: CONNECT_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    if parameters.ssid.len() > SSID_MAXIMUM_LENGTH {
        return Err(ConnectEncodeError::SsidTooLong {
            maximum: SSID_MAXIMUM_LENGTH,
            actual: parameters.ssid.len(),
        });
    }
    if parameters.association_information_elements.len()
        > ASSOCIATION_INFORMATION_ELEMENTS_MAXIMUM_LENGTH
    {
        return Err(ConnectEncodeError::InformationElementsTooLong {
            maximum: ASSOCIATION_INFORMATION_ELEMENTS_MAXIMUM_LENGTH,
            actual: parameters.association_information_elements.len(),
        });
    }

    let output = &mut output[..CONNECT_PARAMETER_SIZE];
    output.fill(0);
    output[0] = parameters.ssid.len() as u8;
    output[1..1 + parameters.ssid.len()].copy_from_slice(parameters.ssid);
    output[34..40].copy_from_slice(&parameters.bssid);
    output[40..42].copy_from_slice(&parameters.frequency_mhz.to_le_bytes());
    output[42] = parameters.band;
    output[43] = parameters.channel_flags;
    output[44] = parameters.transmit_power_dbm as u8;
    output[48..52].copy_from_slice(&(CONTROL_PORT_HOST | WPA_WPA2_IN_USE).to_le_bytes());
    output[52..54].copy_from_slice(&CONTROL_PORT_PROTOCOL_EAPOL);
    output[54..56]
        .copy_from_slice(&(parameters.association_information_elements.len() as u16).to_le_bytes());
    output[56..58].copy_from_slice(&0_u16.to_le_bytes());
    output[58] = 0;
    output[59] = 0;
    output[60] = 1;
    output[61] = parameters.interface_index;
    output[64..64 + parameters.association_information_elements.len()]
        .copy_from_slice(parameters.association_information_elements);
    Ok(CONNECT_PARAMETER_SIZE)
}

pub fn decode_connect_confirmation(
    parameter: &[u8],
) -> Result<ConnectConfirmation, ConnectDecodeError> {
    if parameter.is_empty() {
        return Err(ConnectDecodeError::TruncatedConfirmation {
            required: 1,
            available: parameter.len(),
        });
    }
    Ok(ConnectConfirmation {
        status: parameter[0],
    })
}

pub fn decode_connect_indication(
    parameter: &[u8],
) -> Result<ConnectIndication<'_>, ConnectDecodeError> {
    if parameter.len() < CONNECT_INDICATION_SIZE {
        return Err(ConnectDecodeError::TruncatedIndication {
            required: CONNECT_INDICATION_SIZE,
            available: parameter.len(),
        });
    }
    let request_length = usize::from(u16::from_le_bytes(parameter[14..16].try_into().unwrap()));
    let response_length = usize::from(u16::from_le_bytes(parameter[16..18].try_into().unwrap()));
    let information_elements_length = request_length + response_length;
    if information_elements_length > CONNECT_INDICATION_INFORMATION_ELEMENTS_LENGTH {
        return Err(ConnectDecodeError::InformationElementsTooLong {
            maximum: CONNECT_INDICATION_INFORMATION_ELEMENTS_LENGTH,
            actual: information_elements_length,
        });
    }
    let request_start = CONNECT_INDICATION_INFORMATION_ELEMENTS_OFFSET;
    let response_start = request_start + request_length;
    let response_end = response_start + response_length;

    Ok(ConnectIndication {
        status_code: u16::from_le_bytes(parameter[0..2].try_into().unwrap()),
        bssid: parameter[2..8].try_into().unwrap(),
        roamed: parameter[8] != 0,
        interface_index: parameter[9],
        access_point_index: parameter[10],
        channel_index: parameter[11],
        qos: parameter[12] != 0,
        admission_control_mask: parameter[13],
        association_request_information_elements: &parameter[request_start..response_start],
        association_response_information_elements: &parameter[response_start..response_end],
        association_id: u16::from_le_bytes(parameter[820..822].try_into().unwrap()),
        band: parameter[822],
        center_frequency_mhz: u16::from_le_bytes(parameter[824..826].try_into().unwrap()),
        width: parameter[826],
        center_frequency1_mhz: u32::from_le_bytes(parameter[828..832].try_into().unwrap()),
        center_frequency2_mhz: u32::from_le_bytes(parameter[832..836].try_into().unwrap()),
        access_category_parameters: [
            u32::from_le_bytes(parameter[836..840].try_into().unwrap()),
            u32::from_le_bytes(parameter[840..844].try_into().unwrap()),
            u32::from_le_bytes(parameter[844..848].try_into().unwrap()),
            u32::from_le_bytes(parameter[848..852].try_into().unwrap()),
        ],
    })
}

pub struct AicAssociationClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    parameter_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
    aggregate_length: usize,
    aggregate_offset: usize,
}

impl<'a, I> AicAssociationClient<'a, I>
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

    pub fn start_wpa2_personal_connect(
        &mut self,
        parameters: ConnectParameters<'_>,
    ) -> Result<(), AicAssociationError<<I as AicCommandIo>::Error>> {
        let parameter_length =
            encode_wpa2_personal_connect_parameters(self.parameter_storage, parameters)
                .map_err(AicAssociationError::Encode)?;
        let frame_length = build_command_frame(
            self.transmit_storage,
            self.product.command_header_mode(),
            SM_CONNECT_REQUEST,
            SM_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..parameter_length],
        )
        .map_err(AicAssociationError::Protocol)?;
        let transfer_length = finalize_command_transfer(self.transmit_storage, frame_length)
            .map_err(AicAssociationError::Protocol)?;
        send_command_transfer(
            self.io,
            self.product,
            &mut self.transmit_storage[..transfer_length],
        )
        .map_err(AicAssociationError::Send)
    }

    pub fn send_ethernet_frame(
        &mut self,
        frame: &[u8],
        interface_index: u8,
        station_index: u8,
        confirmation_index: u32,
    ) -> Result<DataTransferLengths, EthernetSendError<<I as AicCommandIo>::Error>> {
        const ETHERNET_HEADER_LENGTH: usize = 14;
        if frame.len() < ETHERNET_HEADER_LENGTH {
            return Err(EthernetSendError::TruncatedFrame {
                required: ETHERNET_HEADER_LENGTH,
                available: frame.len(),
            });
        }

        let lengths = build_d80_ethernet_data_transfer(
            self.transmit_storage,
            EthernetTransmitParameters {
                destination_address: frame[0..6].try_into().unwrap(),
                source_address: frame[6..12].try_into().unwrap(),
                ether_type: u16::from_be_bytes(frame[12..14].try_into().unwrap()),
                interface_index,
                station_index,
                confirmation_index,
            },
            &frame[ETHERNET_HEADER_LENGTH..],
        )
        .map_err(EthernetSendError::Encode)?;
        send_d80_data_transfer(
            self.io,
            &mut self.transmit_storage[..lengths.transfer_length],
        )
        .map_err(EthernetSendError::Send)?;
        Ok(lengths)
    }

    pub fn next_event(
        &mut self,
    ) -> Result<AssociationEvent<'_>, AicAssociationError<<I as AicCommandIo>::Error>> {
        for elapsed_ms in 0..=self.response_timeout_ms {
            if self.aggregate_offset < self.aggregate_length {
                let (start, end, next_offset, kind) = match next_aggregate_frame_bounds(
                    self.receive_storage,
                    self.aggregate_length,
                    self.aggregate_offset,
                )
                .map_err(AicAssociationError::Decode)?
                {
                    Some(bounds) => bounds,
                    None => {
                        self.aggregate_offset = self.aggregate_length;
                        continue;
                    }
                };
                self.aggregate_offset = next_offset;
                if kind == AggregateFrameKind::Data {
                    let packet = &self.receive_storage[start..end];
                    return Ok(match decode_sdio_data_packet(packet) {
                        Ok(frame) => AssociationEvent::Data(frame),
                        Err(error) => AssociationEvent::UndecodedData { packet, error },
                    });
                }
                let message_type = self.receive_storage[start + 2] & 0x7f;
                if message_type != SDIO_CONFIG_COMMAND_RESPONSE_TYPE {
                    return Ok(AssociationEvent::Transport { message_type });
                }
                let response = ConfigResponse::parse(&self.receive_storage[start..end])
                    .map_err(AicAssociationError::Protocol)?;
                return decode_association_event(response);
            }

            if let Some(length) = receive_pending_frame(self.io, self.product, self.receive_storage)
                .map_err(AicAssociationError::Receive)?
            {
                self.aggregate_length = length;
                self.aggregate_offset = 0;
                continue;
            }
            if elapsed_ms < self.response_timeout_ms {
                AicResponseIo::delay_ms(self.io, 1);
            }
        }
        Err(AicAssociationError::Receive(AicResponseError::Timeout {
            milliseconds: self.response_timeout_ms,
        }))
    }
}

fn next_aggregate_frame_bounds(
    input: &[u8],
    aggregate_length: usize,
    offset: usize,
) -> Result<Option<(usize, usize, usize, AggregateFrameKind)>, ConnectDecodeError> {
    if offset >= aggregate_length {
        return Ok(None);
    }
    if aggregate_length - offset < 4 {
        return Err(ConnectDecodeError::TruncatedAggregateFrame {
            required: offset + 4,
            available: aggregate_length,
        });
    }
    let packet_length = usize::from(u16::from_le_bytes(
        input[offset..offset + 2].try_into().unwrap(),
    ));
    if packet_length == 0 {
        return Ok(None);
    }
    let message_type = input[offset + 2] & 0x7f;
    let kind = if message_type & SDIO_CONFIG_TYPE_MASK == SDIO_CONFIG_TYPE_MASK {
        AggregateFrameKind::Config
    } else {
        AggregateFrameKind::Data
    };
    let unaligned_frame_length = match kind {
        AggregateFrameKind::Config => 4 + packet_length,
        AggregateFrameKind::Data => SDIO_RECEIVE_HEADER_LENGTH + packet_length,
    };
    let frame_end = offset + unaligned_frame_length;
    if frame_end > aggregate_length {
        return Err(ConnectDecodeError::TruncatedAggregateFrame {
            required: frame_end,
            available: aggregate_length,
        });
    }
    let next_offset = offset + ((unaligned_frame_length + 3) & !3);
    Ok(Some((offset, frame_end, next_offset, kind)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AggregateFrameKind {
    Config,
    Data,
}

fn decode_association_event<E>(
    response: ConfigResponse<'_>,
) -> Result<AssociationEvent<'_>, AicAssociationError<E>> {
    match response.id {
        SM_CONNECT_CONFIRM => decode_connect_confirmation(response.parameter)
            .map(AssociationEvent::Confirmation)
            .map_err(AicAssociationError::Decode),
        SM_CONNECT_INDICATION => decode_connect_indication(response.parameter)
            .map(AssociationEvent::Indication)
            .map_err(AicAssociationError::Decode),
        message_id => Ok(AssociationEvent::Unrelated { message_id }),
    }
}
use crate::data::{
    DataDecodeError, ReceivedEthernetFrame, SDIO_CONFIG_COMMAND_RESPONSE_TYPE,
    SDIO_CONFIG_TYPE_MASK, SDIO_RECEIVE_HEADER_LENGTH, decode_sdio_data_packet,
};
use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::protocol::{
    ConfigResponse, ProtocolError, build_command_frame, finalize_command_transfer,
};
use crate::response::{AicResponseError, AicResponseIo, receive_pending_frame};
use crate::sdio::{AicCommandError, AicCommandIo, send_command_transfer};
use crate::tx::{
    DataSendError, DataTransferLengths, DataTransmitError, EthernetTransmitParameters,
    build_d80_ethernet_data_transfer, send_d80_data_transfer,
};
