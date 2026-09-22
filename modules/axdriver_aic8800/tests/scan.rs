use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::scan::{
    AicScanClient, MM_CHANNEL_SURVEY_INDICATION, SCANU_RESULT_INDICATION,
    SCANU_START_ADDITIONAL_CONFIRM, SCANU_START_CONFIRM, SCANU_START_REQUEST, ScanEvent,
    decode_channel_survey, decode_scan_result, decode_scan_start_confirmation,
    encode_d80_full_scan_parameters, find_information_element, parse_bss_description,
};
use axdriver_aic8800::sdio::AicCommandIo;

#[test]
fn encodes_the_fixed_d80_full_scan_layout() {
    let mut output = [0xaa_u8; 376];
    let length = encode_d80_full_scan_parameters(&mut output, 3).unwrap();

    assert_eq!(length, 376);
    assert_eq!(&output[0..6], &[0x6c, 0x09, 0, 0, 20, 0]);
    assert_eq!(&output[13 * 6..14 * 6], &[0xb4, 0x09, 0, 0, 20, 0]);
    assert_eq!(&output[14 * 6..15 * 6], &[0x3c, 0x14, 1, 0, 20, 0]);
    assert_eq!(&output[38 * 6..39 * 6], &[0xc1, 0x16, 1, 0, 20, 0]);
    assert_eq!(&output[39 * 6..42 * 6], &[0; 18]);
    assert_eq!(&output[252..351], &[0; 99]);
    assert_eq!(output[351], 0);
    assert_eq!(&output[352..358], &[0xff; 6]);
    assert_eq!(&output[358..366], &[0; 8]);
    assert_eq!(output[366], 3);
    assert_eq!(output[367], 39);
    assert_eq!(output[368], 0);
    assert_eq!(output[369], 0);
    assert_eq!(&output[370..376], &[0; 6]);
}

#[test]
fn decodes_scan_result_and_final_confirmation() {
    let frame = [
        0x80, 0x00, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60,
    ];
    let mut parameter = [0_u8; 28];
    parameter[0..2].copy_from_slice(&(frame.len() as u16).to_le_bytes());
    parameter[2..4].copy_from_slice(&0x0080_u16.to_le_bytes());
    parameter[4..6].copy_from_slice(&2412_u16.to_le_bytes());
    parameter[6] = 0;
    parameter[7] = 0xff;
    parameter[8] = 3;
    parameter[9] = (-42_i8) as u8;
    parameter[12..].copy_from_slice(&frame);

    let result = decode_scan_result(&parameter).unwrap();
    assert_eq!(result.length, 16);
    assert_eq!(result.frame_control, 0x0080);
    assert_eq!(result.center_frequency_mhz, 2412);
    assert_eq!(result.band, 0);
    assert_eq!(result.station_index, 0xff);
    assert_eq!(result.interface_index, 3);
    assert_eq!(result.rssi_dbm, -42);
    assert_eq!(result.frame, frame);

    let done = decode_scan_start_confirmation(&[3, 0, 7]).unwrap();
    assert_eq!(done.interface_index, 3);
    assert_eq!(done.status, 0);
    assert_eq!(done.result_count, 7);
}

#[test]
fn parses_the_bssid_beacon_fields_and_preserves_all_information_elements() {
    let mut frame = [0_u8; 67];
    frame[0..2].copy_from_slice(&0x0080_u16.to_le_bytes());
    frame[16..22].copy_from_slice(&[0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
    frame[32..34].copy_from_slice(&100_u16.to_le_bytes());
    frame[34..36].copy_from_slice(&0x0431_u16.to_le_bytes());
    frame[36..45].copy_from_slice(&[0, 7, b'm', b'u', b's', b'h', b'r', b'o', b'o']);
    frame[45..67].copy_from_slice(&[
        48, 20, 1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 2, 0, 0,
    ]);

    let bss = parse_bss_description(&frame).unwrap();
    assert_eq!(bss.bssid, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
    assert_eq!(bss.beacon_interval, 100);
    assert_eq!(bss.capability, 0x0431);
    assert_eq!(bss.ssid, b"mushroo");
    assert_eq!(bss.information_elements, &frame[36..]);
    assert_eq!(
        find_information_element(bss.information_elements, 48).unwrap(),
        Some(&frame[45..67])
    );
}

#[test]
fn decodes_the_fixed_channel_survey_layout() {
    let parameter = [
        0x6c,
        0x09,
        (-96_i8) as u8,
        0,
        0x34,
        0x12,
        0,
        0,
        0x21,
        0,
        0,
        0,
    ];

    let survey = decode_channel_survey(&parameter).unwrap();
    assert_eq!(survey.frequency_mhz, 2412);
    assert_eq!(survey.noise_dbm, -96);
    assert_eq!(survey.channel_time_ms, 0x1234);
    assert_eq!(survey.channel_busy_time_ms, 0x21);
}

#[derive(Debug, Eq, PartialEq)]
struct Command(u16, u16, u16, Vec<u8>);

struct FakeTransport {
    response: [u8; 512],
    pending: bool,
    commands: Vec<Command>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0; 512],
            pending: false,
            commands: Vec::new(),
        }
    }

    fn prepare_response(&mut self, id: u16, parameter: &[u8]) {
        self.response.fill(0);
        Self::write_response(&mut self.response, 0, id, parameter);
        self.pending = true;
    }

    fn prepare_two_responses(
        &mut self,
        first_id: u16,
        first_parameter: &[u8],
        second_id: u16,
        second_parameter: &[u8],
    ) {
        self.response.fill(0);
        let second_offset = Self::write_response(&mut self.response, 0, first_id, first_parameter);
        Self::write_response(
            &mut self.response,
            second_offset,
            second_id,
            second_parameter,
        );
        self.pending = true;
    }

    fn write_response(output: &mut [u8], offset: usize, id: u16, parameter: &[u8]) -> usize {
        let packet_length = 12 + parameter.len();
        output[offset..offset + 2].copy_from_slice(&(packet_length as u16).to_le_bytes());
        output[offset + 2] = 0x11;
        output[offset + 4..offset + 6].copy_from_slice(&id.to_le_bytes());
        output[offset + 10..offset + 12].copy_from_slice(&(parameter.len() as u16).to_le_bytes());
        output[offset + 16..offset + 16 + parameter.len()].copy_from_slice(parameter);
        offset + ((packet_length + 3) & !3) + 4
    }
}

impl AicCommandIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(1)
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
        self.prepare_response(SCANU_START_ADDITIONAL_CONFIRM, &[]);
        Ok(())
    }

    fn delay_us(&mut self, _microseconds: u32) {}
    fn delay_ms(&mut self, _milliseconds: u32) {}
}

impl AicResponseIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(u8::from(self.pending))
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
fn sends_scan_request_then_receives_result_and_completion() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 376];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];

    {
        let mut client = AicScanClient::new(
            &mut io,
            Aic8800Product::Aic8800D80,
            &mut parameter,
            &mut transmit,
            &mut receive,
            6000,
        );
        client.start_d80_full_scan(3).unwrap();
    }

    assert_eq!(io.commands.len(), 1);
    assert_eq!(io.commands[0].0, SCANU_START_REQUEST);
    assert_eq!(io.commands[0].1, 4);
    assert_eq!(io.commands[0].2, 100);
    assert_eq!(io.commands[0].3.len(), 376);

    let mut result_parameter = [0_u8; 16];
    result_parameter[0..2].copy_from_slice(&4_u16.to_le_bytes());
    result_parameter[4..6].copy_from_slice(&2437_u16.to_le_bytes());
    result_parameter[8] = 3;
    result_parameter[9] = (-55_i8) as u8;
    result_parameter[12..16].copy_from_slice(&[1, 2, 3, 4]);
    io.prepare_response(SCANU_RESULT_INDICATION, &result_parameter);

    {
        let mut client = AicScanClient::new(
            &mut io,
            Aic8800Product::Aic8800D80,
            &mut parameter,
            &mut transmit,
            &mut receive,
            6000,
        );
        match client.next_event().unwrap() {
            ScanEvent::Result(result) => {
                assert_eq!(result.center_frequency_mhz, 2437);
                assert_eq!(result.rssi_dbm, -55);
                assert_eq!(result.frame, &[1, 2, 3, 4]);
            }
            _ => panic!("expected a scan result"),
        }
    }

    io.prepare_two_responses(
        MM_CHANNEL_SURVEY_INDICATION,
        &[0x85, 0x09, (-97_i8) as u8, 0, 10, 0, 0, 0, 3, 0, 0, 0],
        SCANU_START_CONFIRM,
        &[3, 0, 1],
    );
    let mut client = AicScanClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );
    match client.next_event().unwrap() {
        ScanEvent::ChannelSurvey(survey) => {
            assert_eq!(survey.frequency_mhz, 2437);
            assert_eq!(survey.channel_time_ms, 10);
            assert_eq!(survey.channel_busy_time_ms, 3);
        }
        _ => panic!("expected a channel survey"),
    }
    match client.next_event().unwrap() {
        ScanEvent::Complete(done) => assert_eq!(done.result_count, 1),
        _ => panic!("expected scan completion"),
    }
}
