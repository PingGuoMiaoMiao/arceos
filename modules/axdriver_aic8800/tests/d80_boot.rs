use axdriver_aic8800::d80::{
    D80ExtPatchImage, D80FirmwareIo, load_d80_bluetooth, load_d80_wifi_and_start,
};
use axdriver_aic8800::patch_table::{AicPatchMemory, parse_patch_table};

const TAG: &[u8; 16] = b"AICBT_PT_TAG\0\0\0\0";

fn board_information_table() -> Vec<u8> {
    let mut output = TAG.to_vec();
    output.extend_from_slice(b"AICBT_PINF_T\0\0\0\0");
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&6_u32.to_le_bytes());
    for (address, value) in [
        (0x001e_7e9c_u32, 0x0020_1940_u32),
        (0x001e_7ea0, 0x001e_0000),
        (0x4050_0150, 1),
        (0x4050_0150, 1),
        (1, 1),
        (0, 0x0020_b43c),
    ] {
        output.extend_from_slice(&address.to_le_bytes());
        output.extend_from_slice(&value.to_le_bytes());
    }
    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Upload(u32, usize),
    Read(u32),
    Write(u32, u32),
    DelayUs(u32),
    Start(u32, u32),
}

struct FakeFirmwareIo {
    events: Vec<Event>,
}

impl AicPatchMemory for FakeFirmwareIo {
    type Error = ();

    fn write_word(&mut self, address: u32, value: u32) -> Result<(), Self::Error> {
        self.events.push(Event::Write(address, value));
        Ok(())
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.events.push(Event::DelayUs(microseconds));
    }
}

impl D80FirmwareIo for FakeFirmwareIo {
    fn read_word(&mut self, address: u32) -> Result<u32, Self::Error> {
        self.events.push(Event::Read(address));
        Ok(match address {
            0x0012_0198 => 0x0020_0000,
            0x0012_01a0 => 0x0030_0000,
            0x0012_001c => 0x0609_0200,
            0x0012_01a4 => 0x0040_0000,
            value => panic!("unexpected read {value:#x}"),
        })
    }

    fn upload_image(&mut self, address: u32, image: &[u8]) -> Result<(), Self::Error> {
        self.events.push(Event::Upload(address, image.len()));
        Ok(())
    }

    fn start_app(&mut self, address: u32, boot_type: u32) -> Result<u32, Self::Error> {
        self.events.push(Event::Start(address, boot_type));
        Ok(0x5a)
    }
}

#[test]
fn loads_d80_bluetooth_images_then_applies_the_patch_table() {
    let table_bytes = board_information_table();
    let table = parse_patch_table(&table_bytes).unwrap();
    let ext = [D80ExtPatchImage {
        id: 0,
        bytes: &[0_u8; 7],
    }];
    let mut io = FakeFirmwareIo { events: Vec::new() };

    load_d80_bluetooth(&mut io, &table, &[0; 3], &[0; 5], &ext).unwrap();

    assert_eq!(
        io.events,
        vec![
            Event::Upload(0x0020_1940, 3),
            Event::Upload(0x001e_0000, 5),
            Event::Upload(0x0020_b43c, 7),
            Event::Write(0x001e_7e9c, 0x0020_1940),
            Event::Write(0x001e_7ea0, 0x001e_0000),
            Event::Write(0x4050_0150, 1),
            Event::Write(0x4050_0150, 1),
        ]
    );
}

#[test]
fn loads_wifi_configures_the_new_patch_buffer_and_starts_firmware() {
    let mut io = FakeFirmwareIo { events: Vec::new() };

    let boot_status = load_d80_wifi_and_start(&mut io, &[0_u8; 11]).unwrap();

    assert_eq!(boot_status, 0x5a);
    assert_eq!(
        io.events,
        vec![
            Event::Upload(0x0012_0000, 11),
            Event::Read(0x0012_0198),
            Event::Read(0x0012_01a0),
            Event::Read(0x0012_001c),
            Event::Read(0x0012_01a4),
            Event::Write(0x0030_0000, 0x4843_5450),
            Event::Write(0x0030_0008, 0x5054_4348),
            Event::Write(0x0030_0004, 0x0040_0000),
            Event::Write(0x0030_000c, 3),
            Event::Write(0x0040_0000, 0x0020_00b4),
            Event::Write(0x0040_0004, 0xf301_0000),
            Event::Write(0x0040_0008, 0x0020_0170),
            Event::Write(0x0040_000c, 0x0100_000a),
            Event::Write(0x0040_0010, 0x0020_0188),
            Event::Write(0x0040_0014, 3),
            Event::Write(0x0030_0030, 0),
            Event::Write(0x0030_0034, 0),
            Event::Write(0x0030_0038, 0),
            Event::Write(0x0030_003c, 0),
            Event::Start(0x0012_0000, 1),
        ]
    );
}
