use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::management::{
    AicManagementClient, MM_GET_MAC_ADDRESS_CONFIRM, MM_GET_MAC_ADDRESS_REQUEST, MM_RESET_CONFIRM,
    MM_RESET_REQUEST, MM_VERSION_CONFIRM, MM_VERSION_REQUEST, decode_mm_version_confirmation,
};
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;

const VERSION_PARAMETER: [u8; 28] = [
    0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0xcc, 0xbb, 0xaa, 0x99, 0x04, 0x03, 0x02, 0x01,
    0x14, 0x13, 0x12, 0x11, 0x24, 0x23, 0x22, 0x21, 0x34, 0x12, 0x08, 0x03,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Request(u16, u16, u16, u16),
}

struct FakeTransport {
    response: [u8; 512],
    events: Vec<Event>,
    parameters: Vec<Vec<u8>>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0_u8; 512],
            events: Vec::new(),
            parameters: Vec::new(),
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
        let request = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let destination = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let source = u16::from_le_bytes(data[12..14].try_into().unwrap());
        let parameter_length = u16::from_le_bytes(data[14..16].try_into().unwrap());
        self.events.push(Event::Request(
            request,
            destination,
            source,
            parameter_length,
        ));
        self.parameters
            .push(data[16..16 + usize::from(parameter_length)].to_vec());
        match request {
            MM_RESET_REQUEST => self.prepare_response(MM_RESET_CONFIRM, &[]),
            MM_VERSION_REQUEST => self.prepare_response(MM_VERSION_CONFIRM, &VERSION_PARAMETER),
            MM_GET_MAC_ADDRESS_REQUEST => self.prepare_response(
                MM_GET_MAC_ADDRESS_CONFIRM,
                &[0x48, 0xda, 0x35, 0x6f, 0x95, 0xf0],
            ),
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
fn decodes_the_fixed_mm_version_confirmation_layout() {
    let version = decode_mm_version_confirmation(&VERSION_PARAMETER).unwrap();

    assert_eq!(version.version_lmac, 0x1122_3344);
    assert_eq!(version.version_machw_1, 0x5566_7788);
    assert_eq!(version.version_machw_2, 0x99aa_bbcc);
    assert_eq!(version.version_phy_1, 0x0102_0304);
    assert_eq!(version.version_phy_2, 0x1112_1314);
    assert_eq!(version.features, 0x2122_2324);
    assert_eq!(version.max_sta_nb, 0x1234);
    assert_eq!(version.max_vif_nb, 8);
}

#[test]
fn rejects_a_truncated_mm_version_confirmation() {
    let error = decode_mm_version_confirmation(&VERSION_PARAMETER[..26]).unwrap_err();
    assert_eq!(
        error,
        axdriver_aic8800::management::ManagementDecodeError::TruncatedVersion {
            required: 27,
            available: 26,
        }
    );
}

#[test]
fn sends_reset_then_version_with_the_fixed_message_ids() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicManagementClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut transmit,
        &mut receive,
        6000,
    );

    client.reset().unwrap();
    let version = client.read_version().unwrap();

    assert_eq!(version.version_lmac, 0x1122_3344);
    assert_eq!(
        io.events,
        vec![
            Event::Request(MM_RESET_REQUEST, 0, 100, 0),
            Event::Request(MM_VERSION_REQUEST, 0, 100, 0),
        ]
    );
}

#[test]
fn sends_get_mac_with_the_fixed_message_ids_and_get_flag() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicManagementClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut transmit,
        &mut receive,
        6000,
    );

    let address = client.read_mac_address().unwrap();

    assert_eq!(address, [0x48, 0xda, 0x35, 0x6f, 0x95, 0xf0]);
    assert_eq!(
        io.events,
        vec![Event::Request(MM_GET_MAC_ADDRESS_REQUEST, 0, 100, 4,)]
    );
    assert_eq!(io.parameters, vec![vec![1, 0, 0, 0]]);
}
