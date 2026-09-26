use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::runtime::{
    AicRuntimeClient, MM_ADD_INTERFACE_CONFIRM, MM_ADD_INTERFACE_REQUEST, MM_START_CONFIRM,
    MM_START_REQUEST, decode_mm_add_interface_confirmation, encode_d80_start_parameters,
    encode_station_interface_parameters,
};
use axdriver_aic8800::sdio::AicCommandIo;

const BOARD_MAC: [u8; 6] = [0x38, 0x7a, 0xcc, 0x98, 0xe6, 0x46];

#[test]
fn encodes_the_fixed_d80_start_parameters() {
    let mut output = [0xaa_u8; 72];
    let length = encode_d80_start_parameters(&mut output).unwrap();

    assert_eq!(length, 72);
    assert_eq!(&output[..64], &[0; 64]);
    assert_eq!(&output[64..68], &300_u32.to_le_bytes());
    assert_eq!(&output[68..70], &20_u16.to_le_bytes());
    assert_eq!(&output[70..72], &[0, 0]);
}

#[test]
fn encodes_and_decodes_the_station_interface_layout() {
    let mut output = [0xaa_u8; 10];
    let length = encode_station_interface_parameters(&mut output, BOARD_MAC).unwrap();
    let confirmation = decode_mm_add_interface_confirmation(&[0, 3]).unwrap();

    assert_eq!(length, 10);
    assert_eq!(output, [0, 0, 0x38, 0x7a, 0xcc, 0x98, 0xe6, 0x46, 0, 0]);
    assert_eq!(confirmation.status, 0);
    assert_eq!(confirmation.interface_index, 3);
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

    fn prepare_response(&mut self, id: u16, parameter: &[u8]) {
        self.response.fill(0);
        self.response[0..2].copy_from_slice(&(12_u16 + parameter.len() as u16).to_le_bytes());
        self.response[2] = 0x11;
        self.response[4..6].copy_from_slice(&id.to_le_bytes());
        self.response[10..12].copy_from_slice(&(parameter.len() as u16).to_le_bytes());
        self.response[16..16 + parameter.len()].copy_from_slice(parameter);
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
            MM_START_REQUEST => self.prepare_response(MM_START_CONFIRM, &[]),
            MM_ADD_INTERFACE_REQUEST => self.prepare_response(MM_ADD_INTERFACE_CONFIRM, &[0, 3]),
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
fn starts_firmware_then_adds_the_station_interface() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 72];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicRuntimeClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );

    client.start().unwrap();
    let interface = client.add_station_interface(BOARD_MAC).unwrap();

    assert_eq!(interface.status, 0);
    assert_eq!(interface.interface_index, 3);
    assert_eq!(io.events.len(), 2);
    assert_eq!(io.events[0].0, MM_START_REQUEST);
    assert_eq!(io.events[0].1, 0);
    assert_eq!(io.events[0].2, 100);
    assert_eq!(io.events[0].3.len(), 72);
    assert_eq!(&io.events[0].3[..64], &[0; 64]);
    assert_eq!(io.events[1].0, MM_ADD_INTERFACE_REQUEST);
    assert_eq!(io.events[1].1, 0);
    assert_eq!(io.events[1].2, 100);
    assert_eq!(
        io.events[1].3,
        vec![0, 0, 0x38, 0x7a, 0xcc, 0x98, 0xe6, 0x46, 0, 0]
    );
    assert_eq!(&io.events[0].3[64..68], &300_u32.to_le_bytes());
    assert_eq!(&io.events[0].3[68..70], &20_u16.to_le_bytes());
}
