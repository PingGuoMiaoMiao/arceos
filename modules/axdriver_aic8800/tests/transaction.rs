use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;
use axdriver_aic8800::transaction::execute_config_command;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    ReadRegister(u8, u32),
    WriteFifo(u8, u32, usize),
    ReadFifo(u8, u32, usize),
}

struct FakeTransport {
    response: [u8; 512],
    events: Vec<Event>,
}

impl FakeTransport {
    fn new() -> Self {
        let mut response = [0_u8; 512];
        response[0..2].copy_from_slice(&20_u16.to_le_bytes());
        response[2] = 0x11;
        response[4..6].copy_from_slice(&0x0401_u16.to_le_bytes());
        response[10..12].copy_from_slice(&8_u16.to_le_bytes());
        response[16..24].copy_from_slice(&[0x00, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3]);
        Self {
            response,
            events: Vec::new(),
        }
    }
}

impl AicCommandIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        self.events.push(Event::ReadRegister(function, address));
        Ok(1)
    }

    fn write_fifo(
        &mut self,
        function: u8,
        address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.events
            .push(Event::WriteFifo(function, address, data.len()));
        assert_eq!(
            &data[8..16],
            &[0x00, 0x04, 0x01, 0x00, 0x64, 0x00, 0x04, 0x00]
        );
        assert_eq!(&data[16..20], &[0x00, 0x00, 0x50, 0x40]);
        Ok(())
    }

    fn delay_us(&mut self, _microseconds: u32) {}

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

impl AicResponseIo for FakeTransport {
    type Error = ();

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        self.events.push(Event::ReadRegister(function, address));
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
        function: u8,
        address: u32,
        output: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.events
            .push(Event::ReadFifo(function, address, output.len()));
        output.copy_from_slice(&self.response[..output.len()]);
        Ok(())
    }

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

#[test]
fn config_transaction_builds_sends_receives_and_validates_one_response() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];

    let response = execute_config_command(
        &mut io,
        Aic8800Product::Aic8800D80,
        0x0400,
        0x0401,
        1,
        100,
        &[0x00, 0x00, 0x50, 0x40],
        &mut transmit,
        &mut receive,
        6000,
    )
    .unwrap();

    assert_eq!(response.id, 0x0401);
    assert_eq!(
        response.parameter,
        &[0x00, 0x00, 0x50, 0x40, 0x20, 0x88, 0xc7, 0xf3]
    );
    assert_eq!(
        io.events,
        vec![
            Event::ReadRegister(1, 0x03),
            Event::WriteFifo(1, 0x10, 512),
            Event::ReadRegister(1, 0x04),
            Event::ReadFifo(1, 0x0f, 512),
        ]
    );
}
