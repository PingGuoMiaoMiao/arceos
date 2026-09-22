use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::key::{AicKeyClient, MM_KEY_ADD_CONFIRM, MM_KEY_ADD_REQUEST};
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;

struct FakeTransport {
    response: [u8; 512],
    request: Option<(u16, u16, u16, Vec<u8>)>,
}

impl FakeTransport {
    fn new() -> Self {
        Self {
            response: [0_u8; 512],
            request: None,
        }
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
        let parameter_length = usize::from(u16::from_le_bytes(data[14..16].try_into().unwrap()));
        self.request = Some((
            request,
            destination,
            source,
            data[16..16 + parameter_length].to_vec(),
        ));
        self.response.fill(0);
        self.response[0..2].copy_from_slice(&14_u16.to_le_bytes());
        self.response[2] = 0x11;
        self.response[4..6].copy_from_slice(&MM_KEY_ADD_CONFIRM.to_le_bytes());
        self.response[10..12].copy_from_slice(&2_u16.to_le_bytes());
        self.response[16..18].copy_from_slice(&[0, 7]);
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
fn installs_a_pairwise_ccmp_key_with_the_fixed_sdk_44_byte_layout() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 512];
    let mut receive = [0_u8; 512];
    let mut client = AicKeyClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut transmit,
        &mut receive,
        6000,
    );
    let temporal_key = core::array::from_fn(|index| (0x10 + index) as u8);

    let confirmation = client.install_pairwise_ccmp(3, 9, &temporal_key).unwrap();

    assert_eq!(confirmation.hardware_key_index, 7);
    let mut expected = vec![0_u8; 44];
    expected[1] = 9;
    expected[4] = 16;
    expected[8..24].copy_from_slice(&temporal_key);
    expected[40] = 2;
    expected[41] = 3;
    expected[43] = 1;
    assert_eq!(io.request, Some((MM_KEY_ADD_REQUEST, 0, 100, expected)));
}

#[test]
fn installs_a_group_ccmp_key_with_the_fixed_sdk_default_key_layout() {
    let mut io = FakeTransport::new();
    let mut transmit = [0_u8; 512];
    let mut receive = [0_u8; 512];
    let mut client = AicKeyClient::new(
        &mut io,
        Aic8800Product::Aic8800D80,
        &mut transmit,
        &mut receive,
        6000,
    );
    let temporal_key = core::array::from_fn(|index| (0x80 + index) as u8);

    let confirmation = client.install_group_ccmp(3, 1, &temporal_key).unwrap();

    assert_eq!(confirmation.hardware_key_index, 7);
    let mut expected = vec![0_u8; 44];
    expected[0] = 1;
    expected[1] = 0xff;
    expected[4] = 16;
    expected[8..24].copy_from_slice(&temporal_key);
    expected[40] = 2;
    expected[41] = 3;
    assert_eq!(io.request, Some((MM_KEY_ADD_REQUEST, 0, 100, expected)));
}
