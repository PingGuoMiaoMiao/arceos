use axdriver_aic8800::association::{
    AicAssociationClient, AssociationEvent, CONTROL_PORT_HOST, CONTROL_PORT_PROTOCOL_EAPOL,
    ConnectParameters, SM_CONNECT_CONFIRM, SM_CONNECT_INDICATION, SM_CONNECT_REQUEST, SM_TASK_ID,
    WPA_WPA2_IN_USE, decode_connect_confirmation, decode_connect_indication,
    encode_wpa2_personal_connect_parameters,
};
use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::scan::MM_CHANNEL_SURVEY_INDICATION;
use axdriver_aic8800::sdio::AicCommandIo;

const TARGET_BSSID: [u8; 6] = [0x92, 0xf0, 0x52, 0x3f, 0xfa, 0x04];
const TARGET_RSN: [u8; 22] = [
    48, 20, 1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 2, 0, 0,
];

#[test]
fn encodes_the_fixed_sdk_wpa2_personal_connect_layout() {
    let mut output = [0xaa_u8; 320];
    let length = encode_wpa2_personal_connect_parameters(
        &mut output,
        ConnectParameters {
            ssid: b"MEIZU 20 Pro",
            bssid: TARGET_BSSID,
            frequency_mhz: 2437,
            band: 0,
            channel_flags: 0,
            transmit_power_dbm: 20,
            association_information_elements: &TARGET_RSN,
            interface_index: 0,
        },
    )
    .unwrap();

    assert_eq!(SM_TASK_ID, 6);
    assert_eq!(SM_CONNECT_REQUEST, 6144);
    assert_eq!(SM_CONNECT_CONFIRM, 6145);
    assert_eq!(SM_CONNECT_INDICATION, 6146);
    assert_eq!(length, 320);
    assert_eq!(output[0], 12);
    assert_eq!(&output[1..13], b"MEIZU 20 Pro");
    assert_eq!(&output[13..34], &[0; 21]);
    assert_eq!(&output[34..40], &TARGET_BSSID);
    assert_eq!(&output[40..42], &2437_u16.to_le_bytes());
    assert_eq!(output[42], 0);
    assert_eq!(output[43], 0);
    assert_eq!(output[44], 20);
    assert_eq!(&output[45..48], &[0; 3]);
    assert_eq!(
        &output[48..52],
        &(CONTROL_PORT_HOST | WPA_WPA2_IN_USE).to_le_bytes()
    );
    assert_eq!(&output[52..54], &CONTROL_PORT_PROTOCOL_EAPOL);
    assert_eq!(&output[54..56], &(TARGET_RSN.len() as u16).to_le_bytes());
    assert_eq!(&output[56..58], &[0, 0]);
    assert_eq!(output[58], 0);
    assert_eq!(output[59], 0);
    assert_eq!(output[60], 1);
    assert_eq!(output[61], 0);
    assert_eq!(&output[62..64], &[0, 0]);
    assert_eq!(&output[64..86], &TARGET_RSN);
    assert_eq!(&output[86..], &[0; 234]);
}

#[test]
fn decodes_connect_confirmation_and_the_fixed_connect_indication_layout() {
    let confirmation = decode_connect_confirmation(&[0]).unwrap();
    assert_eq!(confirmation.status, 0);

    let mut parameter = [0_u8; 852];
    parameter[0..2].copy_from_slice(&0_u16.to_le_bytes());
    parameter[2..8].copy_from_slice(&TARGET_BSSID);
    parameter[8] = 0;
    parameter[9] = 0;
    parameter[10] = 7;
    parameter[11] = 2;
    parameter[12] = 1;
    parameter[13] = 0x05;
    parameter[14..16].copy_from_slice(&4_u16.to_le_bytes());
    parameter[16..18].copy_from_slice(&3_u16.to_le_bytes());
    parameter[20..27].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
    parameter[820..822].copy_from_slice(&42_u16.to_le_bytes());
    parameter[822] = 0;
    parameter[824..826].copy_from_slice(&2437_u16.to_le_bytes());
    parameter[826] = 0;
    parameter[828..832].copy_from_slice(&2437_u32.to_le_bytes());
    parameter[832..836].copy_from_slice(&0_u32.to_le_bytes());
    parameter[836..840].copy_from_slice(&0x1122_3344_u32.to_le_bytes());
    parameter[840..844].copy_from_slice(&0x5566_7788_u32.to_le_bytes());
    parameter[844..848].copy_from_slice(&0x99aa_bbcc_u32.to_le_bytes());
    parameter[848..852].copy_from_slice(&0xddee_ff00_u32.to_le_bytes());

    let indication = decode_connect_indication(&parameter).unwrap();
    assert_eq!(indication.status_code, 0);
    assert_eq!(indication.bssid, TARGET_BSSID);
    assert!(!indication.roamed);
    assert_eq!(indication.interface_index, 0);
    assert_eq!(indication.access_point_index, 7);
    assert_eq!(indication.channel_index, 2);
    assert!(indication.qos);
    assert_eq!(indication.admission_control_mask, 0x05);
    assert_eq!(
        indication.association_request_information_elements,
        &[1, 2, 3, 4]
    );
    assert_eq!(
        indication.association_response_information_elements,
        &[5, 6, 7]
    );
    assert_eq!(indication.association_id, 42);
    assert_eq!(indication.band, 0);
    assert_eq!(indication.center_frequency_mhz, 2437);
    assert_eq!(indication.width, 0);
    assert_eq!(indication.center_frequency1_mhz, 2437);
    assert_eq!(indication.center_frequency2_mhz, 0);
    assert_eq!(
        indication.access_category_parameters,
        [0x1122_3344, 0x5566_7788, 0x99aa_bbcc, 0xddee_ff00]
    );
}

#[test]
fn rejects_values_that_do_not_fit_the_fixed_sdk_structures() {
    let mut output = [0_u8; 320];
    let long_ssid = [b'x'; 33];
    let long_ies = [0_u8; 257];

    assert!(
        encode_wpa2_personal_connect_parameters(
            &mut output,
            ConnectParameters {
                ssid: &long_ssid,
                bssid: TARGET_BSSID,
                frequency_mhz: 2437,
                band: 0,
                channel_flags: 0,
                transmit_power_dbm: 20,
                association_information_elements: &TARGET_RSN,
                interface_index: 0,
            },
        )
        .is_err()
    );
    assert!(
        encode_wpa2_personal_connect_parameters(
            &mut output,
            ConnectParameters {
                ssid: b"MEIZU 20 Pro",
                bssid: TARGET_BSSID,
                frequency_mhz: 2437,
                band: 0,
                channel_flags: 0,
                transmit_power_dbm: 20,
                association_information_elements: &long_ies,
                interface_index: 0,
            },
        )
        .is_err()
    );

    let mut indication = [0_u8; 852];
    indication[14..16].copy_from_slice(&799_u16.to_le_bytes());
    indication[16..18].copy_from_slice(&2_u16.to_le_bytes());
    assert!(decode_connect_indication(&indication).is_err());
}

#[derive(Debug, Eq, PartialEq)]
struct Command(u16, u16, u16, Vec<u8>);

struct FakeTransport {
    response: [u8; 1024],
    pending: bool,
    commands: Vec<Command>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0; 1024],
            pending: false,
            commands: Vec::new(),
        }
    }

    fn write_response(output: &mut [u8], offset: usize, id: u16, parameter: &[u8]) -> usize {
        let packet_length = 12 + parameter.len();
        output[offset..offset + 2].copy_from_slice(&(packet_length as u16).to_le_bytes());
        output[offset + 2] = 0x11;
        output[offset + 4..offset + 6].copy_from_slice(&id.to_le_bytes());
        output[offset + 10..offset + 12].copy_from_slice(&(parameter.len() as u16).to_le_bytes());
        output[offset + 16..offset + 16 + parameter.len()].copy_from_slice(parameter);
        offset + 4 + ((packet_length + 3) & !3)
    }
}

impl AicCommandIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(2)
    }

    fn write_fifo(
        &mut self,
        _function: u8,
        _address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        let id = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let destination = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let source = u16::from_le_bytes(data[12..14].try_into().unwrap());
        let parameter_length = usize::from(u16::from_le_bytes(data[14..16].try_into().unwrap()));
        self.commands.push(Command(
            id,
            destination,
            source,
            data[16..16 + parameter_length].to_vec(),
        ));

        self.response.fill(0);
        let next = Self::write_response(&mut self.response, 0, SM_CONNECT_CONFIRM, &[0]);
        let next = Self::write_response(
            &mut self.response,
            next,
            MM_CHANNEL_SURVEY_INDICATION,
            &[0x9e, 0x09, (-80_i8) as u8, 0, 50, 0, 0, 0, 8, 0, 0, 0],
        );
        let mut indication = [0_u8; 852];
        indication[2..8].copy_from_slice(&TARGET_BSSID);
        indication[9] = 0;
        indication[10] = 7;
        indication[824..826].copy_from_slice(&2437_u16.to_le_bytes());
        Self::write_response(&mut self.response, next, SM_CONNECT_INDICATION, &indication);
        self.pending = true;
        Ok(())
    }

    fn delay_us(&mut self, _microseconds: u32) {}
    fn delay_ms(&mut self, _milliseconds: u32) {}
}

impl AicResponseIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(if self.pending { 2 } else { 0 })
    }

    fn write_register(
        &mut self,
        _function: u8,
        _address: u32,
        _value: u8,
    ) -> Result<(), Self::Error> {
        unreachable!()
    }

    fn read_fifo(
        &mut self,
        _function: u8,
        _address: u32,
        output: &mut [u8],
    ) -> Result<(), Self::Error> {
        output.copy_from_slice(&self.response[..output.len()]);
        self.pending = false;
        Ok(())
    }

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

#[test]
fn sends_connect_request_and_preserves_an_aggregated_confirmation_and_indication() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 320];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 1024];
    let mut client = AicAssociationClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );

    client
        .start_wpa2_personal_connect(ConnectParameters {
            ssid: b"MEIZU 20 Pro",
            bssid: TARGET_BSSID,
            frequency_mhz: 2437,
            band: 0,
            channel_flags: 0,
            transmit_power_dbm: 20,
            association_information_elements: &TARGET_RSN,
            interface_index: 0,
        })
        .unwrap();

    match client.next_event().unwrap() {
        AssociationEvent::Confirmation(confirmation) => assert_eq!(confirmation.status, 0),
        _ => panic!("expected connect confirmation"),
    }
    assert_eq!(
        client.next_event().unwrap(),
        AssociationEvent::Unrelated {
            message_id: MM_CHANNEL_SURVEY_INDICATION,
        }
    );
    match client.next_event().unwrap() {
        AssociationEvent::Indication(indication) => {
            assert_eq!(indication.status_code, 0);
            assert_eq!(indication.bssid, TARGET_BSSID);
            assert_eq!(indication.access_point_index, 7);
            assert_eq!(indication.center_frequency_mhz, 2437);
        }
        _ => panic!("expected connect indication"),
    }

    drop(client);
    assert_eq!(io.commands.len(), 1);
    assert_eq!(io.commands[0].0, SM_CONNECT_REQUEST);
    assert_eq!(io.commands[0].1, SM_TASK_ID);
    assert_eq!(io.commands[0].2, 100);
    assert_eq!(io.commands[0].3.len(), 320);
}
