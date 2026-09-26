use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::firmware::{FirmwareUploadError, upload_firmware_image};
use axdriver_aic8800::response::AicResponseIo;
use axdriver_aic8800::sdio::AicCommandIo;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    ReceiveFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Write {
    address: u32,
    data: Vec<u8>,
}

struct FakeTransport {
    writes: Vec<Write>,
    response: [u8; 512],
    completed_responses: usize,
    fail_response_number: Option<usize>,
}

impl FakeTransport {
    fn new() -> Self {
        let mut response = [0_u8; 512];
        response[0..2].copy_from_slice(&16_u16.to_le_bytes());
        response[2] = 0x11;
        response[4..6].copy_from_slice(&0x040c_u16.to_le_bytes());
        response[10..12].copy_from_slice(&0_u16.to_le_bytes());
        Self {
            writes: Vec::new(),
            response,
            completed_responses: 0,
            fail_response_number: None,
        }
    }
}

impl AicCommandIo for FakeTransport {
    type Error = TransportError;

    fn read_register(&mut self, _function: u8, _address: u32) -> Result<u8, Self::Error> {
        Ok(2)
    }

    fn write_fifo(
        &mut self,
        function: u8,
        address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        assert_eq!((function, address), (1, 0x10));
        assert_eq!(u16::from_le_bytes([data[8], data[9]]), 0x040b);
        assert_eq!(u16::from_le_bytes([data[10], data[11]]), 1);
        assert_eq!(u16::from_le_bytes([data[12], data[13]]), 100);

        let parameter_length = u16::from_le_bytes([data[14], data[15]]) as usize;
        let parameter = &data[16..16 + parameter_length];
        let memory_address = u32::from_le_bytes(parameter[0..4].try_into().unwrap());
        let memory_size = u32::from_le_bytes(parameter[4..8].try_into().unwrap()) as usize;
        self.writes.push(Write {
            address: memory_address,
            data: parameter[8..8 + memory_size].to_vec(),
        });
        Ok(())
    }

    fn delay_us(&mut self, _microseconds: u32) {}

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

impl AicResponseIo for FakeTransport {
    type Error = TransportError;

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
        let response_number = self.completed_responses + 1;
        if self.fail_response_number == Some(response_number) {
            return Err(TransportError::ReceiveFailed);
        }
        output.copy_from_slice(&self.response[..output.len()]);
        self.completed_responses = response_number;
        Ok(())
    }

    fn delay_ms(&mut self, _milliseconds: u32) {}
}

fn upload(
    io: &mut FakeTransport,
    address: u32,
    image: &[u8],
) -> Result<(), FirmwareUploadError<TransportError>> {
    let mut parameter = [0_u8; 1032];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    upload_firmware_image(
        io,
        Aic8800Product::Aic8800D80,
        address,
        image,
        &mut parameter,
        &mut transmit,
        &mut receive,
        6000,
    )
}

#[test]
fn rejects_an_empty_firmware_image_without_touching_the_bus() {
    let mut io = FakeTransport::new();

    assert_eq!(
        upload(&mut io, 0x0012_0000, &[]),
        Err(FirmwareUploadError::EmptyImage)
    );
    assert!(io.writes.is_empty());
}

#[test]
fn uploads_an_exact_1024_byte_image_as_one_transaction() {
    let mut io = FakeTransport::new();
    let image = vec![0xa5; 1024];

    upload(&mut io, 0x0012_0000, &image).unwrap();

    assert_eq!(
        io.writes,
        vec![Write {
            address: 0x0012_0000,
            data: image
        }]
    );
}

#[test]
fn uploads_1025_bytes_as_one_full_block_and_one_tail_block() {
    let mut io = FakeTransport::new();
    let image: Vec<u8> = (0..1025).map(|index| index as u8).collect();

    upload(&mut io, 0x0012_0000, &image).unwrap();

    assert_eq!(
        io.writes,
        vec![
            Write {
                address: 0x0012_0000,
                data: image[..1024].to_vec()
            },
            Write {
                address: 0x0012_0400,
                data: image[1024..].to_vec()
            },
        ]
    );
}

#[test]
fn stops_before_submitting_a_later_block_after_a_response_failure() {
    let mut io = FakeTransport::new();
    io.fail_response_number = Some(2);
    let image = vec![0x5a; 2050];

    assert!(matches!(
        upload(&mut io, 0x0012_0000, &image),
        Err(FirmwareUploadError::Transaction(_))
    ));
    assert_eq!(io.writes.len(), 2);
    assert_eq!(io.writes[0].address, 0x0012_0000);
    assert_eq!(io.writes[1].address, 0x0012_0400);
}
