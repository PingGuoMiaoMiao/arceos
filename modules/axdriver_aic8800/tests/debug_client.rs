use axdriver_aic8800::debug::AicDebugClient;
use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::firmware::{
    DBG_MEM_READ_REQUEST, DBG_MEM_WRITE_REQUEST, DBG_START_APP_REQUEST,
};
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Read(u32),
    Write(u32, u32),
    Start(u32, u32),
}

struct FakeTransport {
    response: [u8; 512],
    events: Vec<Event>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0_u8; 512],
            events: Vec::new(),
        }
    }

    fn prepare_response(&mut self, request_id: u16, parameter: &[u8]) {
        self.response.fill(0);
        self.response[2] = 0x11;
        self.response[4..6].copy_from_slice(&(request_id + 1).to_le_bytes());
        let response_parameter_length = match request_id {
            DBG_MEM_READ_REQUEST => {
                let address = u32::from_le_bytes(parameter[..4].try_into().unwrap());
                self.events.push(Event::Read(address));
                self.response[16..20].copy_from_slice(&address.to_le_bytes());
                self.response[20..24].copy_from_slice(&0xa5a5_5a5a_u32.to_le_bytes());
                8
            }
            DBG_MEM_WRITE_REQUEST => {
                let address = u32::from_le_bytes(parameter[..4].try_into().unwrap());
                let value = u32::from_le_bytes(parameter[4..8].try_into().unwrap());
                self.events.push(Event::Write(address, value));
                0
            }
            DBG_START_APP_REQUEST => {
                let address = u32::from_le_bytes(parameter[..4].try_into().unwrap());
                let boot_type = u32::from_le_bytes(parameter[4..8].try_into().unwrap());
                self.events.push(Event::Start(address, boot_type));
                self.response[16..20].copy_from_slice(&0x1234_u32.to_le_bytes());
                4
            }
            value => panic!("unexpected request {value:#x}"),
        };
        let packet_length = 12 + response_parameter_length;
        self.response[0..2].copy_from_slice(&(packet_length as u16).to_le_bytes());
        self.response[10..12].copy_from_slice(&(response_parameter_length as u16).to_le_bytes());
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
        let request_id = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let length = u16::from_le_bytes(data[14..16].try_into().unwrap()) as usize;
        self.prepare_response(request_id, &data[16..16 + length]);
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
fn debug_client_reads_writes_and_starts_through_exact_sdk_messages() {
    let mut io = FakeTransport::new();
    let mut parameter = [0_u8; 1032];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicDebugClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    );

    assert_eq!(client.read_word(0x4050_0000).unwrap(), 0xa5a5_5a5a);
    client.write_word(0x4050_0150, 1).unwrap();
    assert_eq!(client.start_app(0x0012_0000, 1).unwrap(), 0x1234);

    assert_eq!(
        io.events,
        vec![
            Event::Read(0x4050_0000),
            Event::Write(0x4050_0150, 1),
            Event::Start(0x0012_0000, 1),
        ]
    );
}
