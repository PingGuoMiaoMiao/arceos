use core::cell::Cell;

use axdriver_aic8800::association::{AicAssociationClient, AicAssociationError, AssociationEvent};
use axdriver_aic8800::data::{DataDecodeError, SDIO_RECEIVE_HEADER_LENGTH};
use axdriver_aic8800::response::{AicResponseError, AicResponseIo};
use axdriver_aic8800::sdio::AicCommandIo;
use smoltcp::iface::SocketSet;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

const ETHERNET_HEADER_LENGTH: usize = 14;
const MAXIMUM_ETHERNET_FRAME_LENGTH: usize = 1514;

pub trait AicNetworkDevice: Device {
    fn transport_failed(&self) -> bool;
}

pub struct AicEthernetDevice<'a, I> {
    client: AicAssociationClient<'a, I>,
    receive_frame: [u8; MAXIMUM_ETHERNET_FRAME_LENGTH],
    interface_index: u8,
    station_index: u8,
    confirmation_index: u32,
    transport_failed: Cell<bool>,
    transmitted_frames: usize,
    received_frames: usize,
    transport_events: usize,
    events_since_summary: usize,
    undecoded_management: usize,
    undecoded_llc: usize,
    undecoded_other: usize,
    unrelated_events: usize,
    confirmation_events: usize,
    llc_dumps: usize,
    last_transmit_ether_type: u16,
    last_transmit_length: usize,
    last_receive_ether_type: u16,
    last_receive_length: usize,
}

impl<'a, I> AicEthernetDevice<'a, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    pub fn new(
        client: AicAssociationClient<'a, I>,
        interface_index: u8,
        station_index: u8,
    ) -> Self {
        Self {
            client,
            receive_frame: [0; MAXIMUM_ETHERNET_FRAME_LENGTH],
            interface_index,
            station_index,
            confirmation_index: 2,
            transport_failed: Cell::new(false),
            transmitted_frames: 0,
            received_frames: 0,
            transport_events: 0,
            events_since_summary: 0,
            undecoded_management: 0,
            undecoded_llc: 0,
            undecoded_other: 0,
            unrelated_events: 0,
            confirmation_events: 0,
            llc_dumps: 0,
            last_transmit_ether_type: 0,
            last_transmit_length: 0,
            last_receive_ether_type: 0,
            last_receive_length: 0,
        }
    }

    pub fn transport_failed(&self) -> bool {
        self.transport_failed.get()
    }

    /// Count one non-data transport event and periodically report the split.
    ///
    /// Only the first eight events are printed in detail, which hides how the
    /// remaining ones are distributed. Counting them by kind is what tells apart a
    /// benign beacon flood from data frames that arrive but fail to decode.
    fn note_transport_event(&mut self) {
        self.transport_events += 1;
        self.events_since_summary += 1;
        if self.events_since_summary >= 256 {
            self.events_since_summary = 0;
            axstd::println!(
                "AIC8800_RX_SUMMARY events={} data={} mgmt_undecoded={} llc_invalid={} other_undecoded={} unrelated={} confirmation={}",
                self.transport_events,
                self.received_frames,
                self.undecoded_management,
                self.undecoded_llc,
                self.undecoded_other,
                self.unrelated_events,
                self.confirmation_events,
            );
        }
    }

    pub fn diagnostics(&self) -> (usize, usize, usize, u16, usize, u16, usize) {
        (
            self.transmitted_frames,
            self.received_frames,
            self.transport_events,
            self.last_transmit_ether_type,
            self.last_transmit_length,
            self.last_receive_ether_type,
            self.last_receive_length,
        )
    }
}

pub struct AicReceiveToken<'a> {
    frame: &'a [u8],
}

impl RxToken for AicReceiveToken<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.frame)
    }

    fn preprocess(&self, _sockets: &mut SocketSet<'_>) {}
}

pub struct AicTransmitToken<'device, 'io, I> {
    client: &'device mut AicAssociationClient<'io, I>,
    interface_index: u8,
    station_index: u8,
    confirmation_index: &'device mut u32,
    transport_failed: &'device Cell<bool>,
    transmitted_frames: &'device mut usize,
    last_transmit_ether_type: &'device mut u16,
    last_transmit_length: &'device mut usize,
}

impl<I> TxToken for AicTransmitToken<'_, '_, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    fn consume<R, F>(self, length: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        assert!(length <= MAXIMUM_ETHERNET_FRAME_LENGTH);
        let mut frame = [0_u8; MAXIMUM_ETHERNET_FRAME_LENGTH];
        let result = f(&mut frame[..length]);
        *self.last_transmit_ether_type = u16::from_be_bytes(frame[12..14].try_into().unwrap());
        *self.last_transmit_length = length;
        if self
            .client
            .send_ethernet_frame(
                &frame[..length],
                self.interface_index,
                self.station_index,
                *self.confirmation_index,
            )
            .is_err()
        {
            self.transport_failed.set(true);
        } else {
            *self.transmitted_frames += 1;
        }
        *self.confirmation_index = self.confirmation_index.wrapping_add(1);
        result
    }
}

impl<'io, I> Device for AicEthernetDevice<'io, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    type RxToken<'device>
        = AicReceiveToken<'device>
    where
        Self: 'device;
    type TxToken<'device>
        = AicTransmitToken<'device, 'io, I>
    where
        Self: 'device;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let received_length = match self.client.next_event() {
            Ok(AssociationEvent::Data(frame)) => {
                let length = ETHERNET_HEADER_LENGTH + frame.payload.len();
                if length > self.receive_frame.len() {
                    self.transport_failed.set(true);
                    return None;
                }
                self.receive_frame[0..6].copy_from_slice(&frame.destination);
                self.receive_frame[6..12].copy_from_slice(&frame.source);
                self.receive_frame[12..14].copy_from_slice(&frame.ether_type.to_be_bytes());
                self.receive_frame[14..length].copy_from_slice(frame.payload);
                self.received_frames += 1;
                self.last_receive_ether_type = frame.ether_type;
                self.last_receive_length = length;
                length
            }
            Err(AicAssociationError::Receive(AicResponseError::Timeout { .. })) => return None,
            Err(_) => {
                self.transport_failed.set(true);
                return None;
            }
            Ok(AssociationEvent::Transport { message_type }) => {
                if self.transport_events < 8 {
                    axstd::println!(
                        "AIC8800_NETWORK_EVENT transport message-type={:#04x}",
                        message_type
                    );
                }
                self.note_transport_event();
                return None;
            }
            Ok(AssociationEvent::Unrelated { message_id }) => {
                if self.transport_events < 8 {
                    axstd::println!(
                        "AIC8800_NETWORK_EVENT unrelated message-id={:#06x}",
                        message_id
                    );
                }
                self.unrelated_events += 1;
                self.note_transport_event();
                return None;
            }
            Ok(AssociationEvent::UndecodedData { packet, error }) => {
                match error {
                    DataDecodeError::UnsupportedFrameControl { .. } => {
                        self.undecoded_management += 1
                    }
                    DataDecodeError::InvalidLlcSnapHeader => self.undecoded_llc += 1,
                    _ => self.undecoded_other += 1,
                }
                if self.transport_events < 8 {
                    axstd::println!("AIC8800_NETWORK_EVENT undecoded-data error={error:?}");
                }
                if error == DataDecodeError::InvalidLlcSnapHeader && self.llc_dumps < 3 {
                    self.llc_dumps += 1;
                    let limit = packet.len().min(SDIO_RECEIVE_HEADER_LENGTH + 64);
                    axstd::print!(
                        "AIC8800_LLC_DUMP sdio_header={} packet_len={} bytes=",
                        SDIO_RECEIVE_HEADER_LENGTH,
                        packet.len()
                    );
                    for byte in &packet[..limit] {
                        axstd::print!("{:02x}", byte);
                    }
                    axstd::println!("");
                }
                self.note_transport_event();
                return None;
            }
            Ok(AssociationEvent::Confirmation(_)) | Ok(AssociationEvent::Indication(_)) => {
                if self.transport_events < 8 {
                    axstd::println!("AIC8800_NETWORK_EVENT association-control");
                }
                self.confirmation_events += 1;
                self.note_transport_event();
                return None;
            }
        };

        Some((
            AicReceiveToken {
                frame: &self.receive_frame[..received_length],
            },
            AicTransmitToken {
                client: &mut self.client,
                interface_index: self.interface_index,
                station_index: self.station_index,
                confirmation_index: &mut self.confirmation_index,
                transport_failed: &self.transport_failed,
                transmitted_frames: &mut self.transmitted_frames,
                last_transmit_ether_type: &mut self.last_transmit_ether_type,
                last_transmit_length: &mut self.last_transmit_length,
            },
        ))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(AicTransmitToken {
            client: &mut self.client,
            interface_index: self.interface_index,
            station_index: self.station_index,
            confirmation_index: &mut self.confirmation_index,
            transport_failed: &self.transport_failed,
            transmitted_frames: &mut self.transmitted_frames,
            last_transmit_ether_type: &mut self.last_transmit_ether_type,
            last_transmit_length: &mut self.last_transmit_length,
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = MAXIMUM_ETHERNET_FRAME_LENGTH;
        capabilities.max_burst_size = Some(1);
        capabilities
    }
}

impl<I> AicNetworkDevice for AicEthernetDevice<'_, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    fn transport_failed(&self) -> bool {
        AicEthernetDevice::transport_failed(self)
    }
}
