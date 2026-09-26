use axdriver_aic8800::patch_table::{
    AICBT_PATCH_BTMODE_TYPE, AICBT_PATCH_INFO_TYPE, AICBT_PATCH_POWER_ON_TYPE,
    AICBT_PATCH_VERSION_TYPE, AicPatchMemory, D80_SDIO_BT_DEFAULT_PATCH_SETTINGS,
    apply_d80_bt_patch_table, parse_patch_table,
};

const TAG: &[u8; 16] = b"AICBT_PT_TAG\0\0\0\0";

fn append_section(output: &mut Vec<u8>, name: &[u8; 16], section_type: u32, pairs: &[(u32, u32)]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&section_type.to_le_bytes());
    output.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for &(address, value) in pairs {
        output.extend_from_slice(&address.to_le_bytes());
        output.extend_from_slice(&value.to_le_bytes());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Write(u32, u32),
    DelayUs(u32),
}

struct Writer {
    events: Vec<Event>,
}

impl AicPatchMemory for Writer {
    type Error = ();

    fn write_word(&mut self, address: u32, value: u32) -> Result<(), Self::Error> {
        self.events.push(Event::Write(address, value));
        Ok(())
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.events.push(Event::DelayUs(microseconds));
    }
}

#[test]
fn applies_the_d80_sdio_bt_overrides_and_sdk_section_rules() {
    let mut bytes = TAG.to_vec();
    append_section(
        &mut bytes,
        b"AICBT_PINF_T\0\0\0\0",
        AICBT_PATCH_INFO_TYPE,
        &[
            (0x001e_7e9c, 0x0020_1940),
            (0x001e_7ea0, 0x001e_0000),
            (0x4050_0150, 1),
            (0x4050_0150, 1),
            (1, 1),
            (0, 0x0020_b43c),
        ],
    );
    append_section(
        &mut bytes,
        b"AICBT_MODE_T\0\0\0\0",
        AICBT_PATCH_BTMODE_TYPE,
        &[
            (0x0020_1fd4, 0),
            (0x0020_1fd8, 0),
            (0x0020_1fd0, 0xff),
            (0x0020_1fc8, 5),
            (0x0020_1fc4, 2),
            (0x0020_1fe8, 0x000e_1000),
            (0x0020_1fe4, 0),
            (0x0020_1fdc, 0),
            (0x0020_1fe0, 0x5f2f_7f2f),
        ],
    );
    append_section(
        &mut bytes,
        b"AICBT_POWER_ON\0\0",
        AICBT_PATCH_POWER_ON_TYPE,
        &[(0x4050_0128, 0x80)],
    );
    append_section(
        &mut bytes,
        b"AICBT_VER_INFO\0\0",
        AICBT_PATCH_VERSION_TYPE,
        &[(0x7541_202d, 0x3130_2067)],
    );
    let table = parse_patch_table(&bytes).unwrap();
    let mut writer = Writer { events: Vec::new() };

    apply_d80_bt_patch_table(&mut writer, &table, D80_SDIO_BT_DEFAULT_PATCH_SETTINGS).unwrap();

    assert_eq!(
        writer.events,
        vec![
            Event::Write(0x001e_7e9c, 0x0020_1940),
            Event::Write(0x001e_7ea0, 0x001e_0000),
            Event::Write(0x4050_0150, 1),
            Event::Write(0x4050_0150, 1),
            Event::Write(0x0020_1fd4, 1),
            Event::Write(0x0020_1fd8, u32::MAX),
            Event::Write(0x0020_1fd0, 0),
            Event::Write(0x0020_1fc8, 5),
            Event::Write(0x0020_1fc4, 1),
            Event::Write(0x0020_1fe8, 1_500_000),
            Event::Write(0x0020_1fe4, 1),
            Event::Write(0x0020_1fdc, 0),
            Event::Write(0x0020_1fe0, 0x0000_6f2f),
            Event::Write(0x4050_0128, 0x80),
            Event::DelayUs(500),
        ]
    );
}

#[test]
fn applies_all_writable_pairs_from_the_local_board_table_when_provided() {
    let Ok(path) = std::env::var("AIC8800_PATCH_TABLE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let table = parse_patch_table(&bytes).unwrap();
    let mut writer = Writer { events: Vec::new() };

    apply_d80_bt_patch_table(&mut writer, &table, D80_SDIO_BT_DEFAULT_PATCH_SETTINGS).unwrap();

    assert_eq!(
        writer
            .events
            .iter()
            .filter(|event| matches!(event, Event::Write(_, _)))
            .count(),
        140
    );
    assert_eq!(
        writer
            .events
            .iter()
            .filter(|event| matches!(event, Event::DelayUs(500)))
            .count(),
        1
    );
    assert!(writer.events.contains(&Event::Write(0x0020_1fc4, 1)));
    assert!(
        writer
            .events
            .contains(&Event::Write(0x0020_1fe8, 1_500_000))
    );
    assert!(
        writer
            .events
            .contains(&Event::Write(0x0020_1fe0, 0x0000_6f2f))
    );
}
