use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::rf::{
    AicRfClient, D80_BOARD_TX_POWER_LEVEL_V3, MM_SET_RF_CALIB_CONFIRM, MM_SET_RF_CALIB_REQUEST,
    MM_SET_TX_POWER_INDEX_LEVEL_CONFIRM, MM_SET_TX_POWER_INDEX_LEVEL_REQUEST,
    encode_d80_rf_calibration_request, encode_d80_tx_power_level_request,
};
use axdriver_aic8800::sdio::AicCommandIo;

#[derive(Debug, Eq, PartialEq)]
struct Request {
    id: u16,
    destination: u16,
    source: u16,
    parameter: Vec<u8>,
}

struct FakeTransport {
    response: [u8; 512],
    requests: Vec<Request>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0_u8; 512],
            requests: Vec::new(),
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
        let parameter_length = u16::from_le_bytes(data[14..16].try_into().unwrap());
        self.requests.push(Request {
            id,
            destination,
            source,
            parameter: data[16..16 + usize::from(parameter_length)].to_vec(),
        });
        match id {
            MM_SET_TX_POWER_INDEX_LEVEL_REQUEST => {
                self.prepare_response(MM_SET_TX_POWER_INDEX_LEVEL_CONFIRM, &[])
            }
            MM_SET_RF_CALIB_REQUEST => self.prepare_response(
                MM_SET_RF_CALIB_CONFIRM,
                &[
                    0x11, 0x11, 0x11, 0x11, 0x22, 0x22, 0x22, 0x22, 0x33, 0x33, 0x33, 0x33, 0x44,
                    0x44, 0x44, 0x44,
                ],
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
fn encodes_the_exact_board_tx_power_table_in_the_fixed_union_size() {
    let request = encode_d80_tx_power_level_request(&D80_BOARD_TX_POWER_LEVEL_V3);

    assert_eq!(request.len(), 95);
    assert_eq!(request[0], 1);
    assert_eq!(
        &request[1..13],
        &[18, 18, 18, 18, 18, 18, 18, 18, 16, 16, 15, 15]
    );
    assert_eq!(&request[13..23], &[18, 18, 18, 18, 16, 16, 15, 15, 14, 14]);
    assert_eq!(
        &request[23..35],
        &[18, 18, 18, 18, 16, 16, 15, 15, 14, 14, 13, 13]
    );
    assert_eq!(
        &request[35..47],
        &[0x80, 0x80, 0x80, 0x80, 18, 18, 18, 18, 16, 16, 15, 15]
    );
    assert_eq!(&request[47..57], &[18, 18, 18, 18, 16, 16, 15, 15, 14, 14]);
    assert_eq!(
        &request[57..69],
        &[18, 18, 18, 18, 16, 16, 14, 14, 13, 13, 12, 12]
    );
    assert_eq!(&request[69..], &[0_u8; 26]);
}

#[test]
fn encodes_the_fixed_d80_rf_calibration_request_with_disabled_xtal_override() {
    assert_eq!(
        encode_d80_rf_calibration_request(),
        [
            0x8f, 0x0f, 0x00, 0x00, 0x0f, 0x0f, 0x00, 0x00, 0x08, 0xc0, 0x34, 0x0c, 0x00, 0x00,
            0x00, 0x00, 0x03, 0x42, 0x26, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn configures_d80_rf_in_the_exact_enabled_board_order() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let mut client = AicRfClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut transmit,
        &mut receive,
        6000,
    );

    let addresses = client.configure_d80_board().unwrap();

    assert_eq!(addresses.rx_gain_24g, 0x1111_1111);
    assert_eq!(addresses.rx_gain_5g, 0x2222_2222);
    assert_eq!(addresses.tx_gain_24g, 0x3333_3333);
    assert_eq!(addresses.tx_gain_5g, 0x4444_4444);
    assert_eq!(io.requests.len(), 2);
    assert_eq!(
        (
            io.requests[0].id,
            io.requests[0].destination,
            io.requests[0].source
        ),
        (MM_SET_TX_POWER_INDEX_LEVEL_REQUEST, 0, 100)
    );
    assert_eq!(io.requests[0].parameter.len(), 95);
    assert_eq!(
        (
            io.requests[1].id,
            io.requests[1].destination,
            io.requests[1].source
        ),
        (MM_SET_RF_CALIB_REQUEST, 0, 100)
    );
    assert_eq!(io.requests[1].parameter.len(), 24);
}
