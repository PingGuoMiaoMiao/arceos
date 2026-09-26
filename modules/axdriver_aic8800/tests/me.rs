use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::me::{
    AicMeClient, ME_CHANNEL_CONFIG_CONFIRM, ME_CHANNEL_CONFIG_REQUEST, ME_CONFIG_CONFIRM,
    ME_CONFIG_REQUEST, ME_SET_CONTROL_PORT_CONFIRM, ME_SET_CONTROL_PORT_REQUEST,
    encode_d80_board_channel_config, encode_d80_board_me_config,
    encode_set_control_port_parameters,
};
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;

const CHANNELS_2GHZ: [u16; 14] = [
    2412, 2417, 2422, 2427, 2432, 2437, 2442, 2447, 2452, 2457, 2462, 2467, 2472, 2484,
];
const CHANNELS_5GHZ: [u16; 25] = [
    5180, 5200, 5220, 5240, 5260, 5280, 5300, 5320, 5500, 5520, 5540, 5560, 5580, 5600, 5620, 5640,
    5660, 5680, 5700, 5720, 5745, 5765, 5785, 5805, 5825,
];

#[test]
fn encodes_the_observed_d80_me_capabilities_in_the_fixed_c_layout() {
    let mut output = [0xaa_u8; 112];
    let length = encode_d80_board_me_config(&mut output).unwrap();

    assert_eq!(length, 112);
    assert_eq!(&output[0..3], &[0x63, 0x09, 0x1f]);
    assert_eq!(
        &output[3..19],
        &[0xff, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0x96, 0, 1, 0, 0, 0]
    );
    assert_eq!(&output[19..32], &[0; 13]);
    assert_eq!(
        &output[32..44],
        &[
            0x31, 0x71, 0x98, 0x03, 0xfe, 0xff, 0x86, 0x01, 0xfe, 0xff, 0x86, 0x01
        ]
    );
    assert_eq!(&output[44..50], &[0x00, 0x00, 0x02, 0x00, 0x00, 0x00]);
    assert_eq!(
        &output[50..61],
        &[
            0x06, 0xe0, 0x2b, 0x58, 0x0d, 0xc0, 0xcf, 0x00, 0x02, 0x30, 0x00
        ]
    );
    assert_eq!(output[61], 0);
    assert_eq!(
        &output[62..74],
        &[
            0xfe, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
        ]
    );
    assert_eq!(&output[74..78], &[0x38, 0x1c, 0xc7, 0x01]);
    assert_eq!(&output[78..100], &[0; 22]);
    assert_eq!(
        &output[100..112],
        &[0xe8, 0x03, 2, 1, 1, 1, 0, 1, 1, 0, 0, 0]
    );
}

#[test]
fn encodes_all_observed_linux_channels_with_twenty_dbm_and_zero_flags() {
    let mut output = [0xaa_u8; 254];
    let length = encode_d80_board_channel_config(&mut output).unwrap();

    assert_eq!(length, 254);
    for (index, frequency) in CHANNELS_2GHZ.iter().enumerate() {
        let offset = index * 6;
        assert_eq!(&output[offset..offset + 2], &frequency.to_le_bytes());
        assert_eq!(&output[offset + 2..offset + 6], &[0, 0, 20, 0]);
    }
    for (index, frequency) in CHANNELS_5GHZ.iter().enumerate() {
        let offset = 14 * 6 + index * 6;
        assert_eq!(&output[offset..offset + 2], &frequency.to_le_bytes());
        assert_eq!(&output[offset + 2..offset + 6], &[1, 0, 20, 0]);
    }
    assert_eq!(&output[234..252], &[0; 18]);
    assert_eq!(&output[252..254], &[14, 25]);
}

#[test]
fn encodes_station_index_and_open_state_in_the_fixed_c_layout() {
    let mut output = [0xaa_u8; 2];
    let length = encode_set_control_port_parameters(&mut output, 7, true).unwrap();

    assert_eq!(length, 2);
    assert_eq!(output, [7, 1]);
}

#[derive(Debug, Eq, PartialEq)]
struct Event(u16, u16, u16, Vec<u8>);

struct FakeTransport {
    response: [u8; 512],
    events: Vec<Event>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0; 512],
            events: Vec::new(),
        }
    }

    fn prepare_empty_response(&mut self, id: u16) {
        self.response.fill(0);
        self.response[0..2].copy_from_slice(&12_u16.to_le_bytes());
        self.response[2] = 0x11;
        self.response[4..6].copy_from_slice(&id.to_le_bytes());
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
        self.events.push(Event(
            id,
            destination,
            source,
            data[16..16 + parameter_length].to_vec(),
        ));
        match id {
            ME_CONFIG_REQUEST => self.prepare_empty_response(ME_CONFIG_CONFIRM),
            ME_CHANNEL_CONFIG_REQUEST => self.prepare_empty_response(ME_CHANNEL_CONFIG_CONFIRM),
            ME_SET_CONTROL_PORT_REQUEST => self.prepare_empty_response(ME_SET_CONTROL_PORT_CONFIRM),
            value => panic!("unexpected request {value:#06x}"),
        }
        Ok(())
    }

    fn delay_us(&mut self, _microseconds: u32) {}
    fn delay_ms(&mut self, _milliseconds: u32) {}
}

impl AicResponseIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(1)
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
        Ok(())
    }

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

#[test]
fn sends_me_config_then_channel_config_with_fixed_task_and_message_ids() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 254];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicMeClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );

    client.configure_d80_board().unwrap();

    assert_eq!(io.events.len(), 2);
    assert_eq!(io.events[0].0, ME_CONFIG_REQUEST);
    assert_eq!(io.events[0].1, 5);
    assert_eq!(io.events[0].2, 100);
    assert_eq!(io.events[0].3.len(), 112);
    assert_eq!(io.events[1].0, ME_CHANNEL_CONFIG_REQUEST);
    assert_eq!(io.events[1].1, 5);
    assert_eq!(io.events[1].2, 100);
    assert_eq!(io.events[1].3.len(), 254);
}

#[test]
fn opens_the_control_port_for_the_associated_station() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 254];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicMeClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );

    client.set_control_port(7, true).unwrap();

    assert_eq!(io.events, vec![Event(0x1404, 5, 100, vec![7, 1])]);
}
