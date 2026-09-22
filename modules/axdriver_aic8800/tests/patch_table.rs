use axdriver_aic8800::patch_table::{
    AICBT_PATCH_INFO_TYPE, AicPatchTableError, parse_d80_patch_info, parse_patch_table,
};

const TAG: &[u8; 16] = b"AICBT_PT_TAG\0\0\0\0";
const INFO_NAME: &[u8; 16] = b"AICBT_PINF_T\0\0\0\0";

fn append_section(output: &mut Vec<u8>, name: &[u8; 16], section_type: u32, pairs: &[(u32, u32)]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&section_type.to_le_bytes());
    output.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for &(address, value) in pairs {
        output.extend_from_slice(&address.to_le_bytes());
        output.extend_from_slice(&value.to_le_bytes());
    }
}

fn exact_board_info_section() -> Vec<u8> {
    let mut bytes = TAG.to_vec();
    append_section(
        &mut bytes,
        INFO_NAME,
        AICBT_PATCH_INFO_TYPE,
        &[
            (0x001e_7e9c, 0x0020_1940),
            (0x001e_7ea0, 0x001e_0000),
            (0x4050_0150, 0x0000_0001),
            (0x4050_0150, 0x0000_0001),
            (0x0000_0001, 0x0000_0001),
            (0x0000_0000, 0x0020_b43c),
        ],
    );
    bytes
}

#[test]
fn parses_the_exact_d80_u02_patch_information_layout() {
    let bytes = exact_board_info_section();
    let table = parse_patch_table(&bytes).unwrap();
    let mut sections = table.sections();
    let section = sections.next().unwrap();

    assert_eq!(section.name(), b"AICBT_PINF_T\0\0\0\0");
    assert_eq!(section.section_type(), 0);
    assert_eq!(section.pair_count(), 6);
    assert!(sections.next().is_none());

    let info = parse_d80_patch_info(&table).unwrap();
    assert_eq!(info.adid_address_info, 0x001e_7e9c);
    assert_eq!(info.adid_address, 0x0020_1940);
    assert_eq!(info.patch_address_info, 0x001e_7ea0);
    assert_eq!(info.patch_address, 0x001e_0000);
    assert_eq!(info.reset_address, 0x4050_0150);
    assert_eq!(info.reset_value, 1);
    assert_eq!(info.adid_flag_address, 0x4050_0150);
    assert_eq!(info.adid_flag_value, 1);
    assert_eq!(info.ext_patch_count_address, 1);
    assert_eq!(info.ext_patch_count(), 1);
    assert_eq!(
        info.ext_patches().collect::<Vec<_>>(),
        vec![(0, 0x0020_b43c)]
    );
}

#[test]
fn rejects_a_patch_table_with_the_wrong_sdk_tag() {
    let mut bytes = exact_board_info_section();
    bytes[0] = b'X';

    assert_eq!(
        parse_patch_table(&bytes),
        Err(AicPatchTableError::InvalidTag)
    );
}

#[test]
fn rejects_a_truncated_section_header() {
    let mut bytes = TAG.to_vec();
    bytes.extend_from_slice(&[0_u8; 23]);

    assert_eq!(
        parse_patch_table(&bytes),
        Err(AicPatchTableError::TruncatedSectionHeader {
            offset: 16,
            available: 23,
        })
    );
}

#[test]
fn rejects_section_data_shorter_than_the_declared_pair_count() {
    let mut bytes = TAG.to_vec();
    bytes.extend_from_slice(INFO_NAME);
    bytes.extend_from_slice(&AICBT_PATCH_INFO_TYPE.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0x1234_5678_u32.to_le_bytes());
    bytes.extend_from_slice(&0x9abc_def0_u32.to_le_bytes());

    assert_eq!(
        parse_patch_table(&bytes),
        Err(AicPatchTableError::TruncatedSectionData {
            offset: 16,
            required: 40,
            available: 32,
        })
    );
}

#[test]
fn rejects_patch_information_without_all_declared_extension_pairs() {
    let mut bytes = TAG.to_vec();
    append_section(
        &mut bytes,
        INFO_NAME,
        AICBT_PATCH_INFO_TYPE,
        &[
            (0x001e_7e9c, 0x0020_1940),
            (0x001e_7ea0, 0x001e_0000),
            (0x4050_0150, 1),
            (0x4050_0150, 1),
            (1, 2),
            (0, 0x0020_b43c),
        ],
    );
    let table = parse_patch_table(&bytes).unwrap();

    assert_eq!(
        parse_d80_patch_info(&table),
        Err(AicPatchTableError::MissingExtensionPairs {
            declared: 2,
            available: 1,
        })
    );
}

#[test]
fn validates_the_local_board_patch_table_when_its_path_is_provided() {
    let Ok(path) = std::env::var("AIC8800_PATCH_TABLE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let table = parse_patch_table(&bytes).unwrap();
    let observed: Vec<_> = table
        .sections()
        .map(|section| {
            (
                section.name().to_vec(),
                section.section_type(),
                section.pair_count(),
            )
        })
        .collect();

    assert_eq!(
        observed,
        vec![
            (b"AICBT_PINF_T\0\0\0\0".to_vec(), 0, 6),
            (b"AICBT_TRAP_T\0\0\0\0".to_vec(), 1, 27),
            (b"AICBT_PATCH_TB4\0".to_vec(), 2, 57),
            (b"AICBT_MODE_T\0\0\0\0".to_vec(), 3, 18),
            (b"AICBT_POWER_ON\0\0".to_vec(), 4, 3),
            (b"AICBT_PATCH_TAF\0".to_vec(), 5, 31),
            (b"AICBT_VER_INFO\0\0".to_vec(), 6, 8),
        ]
    );

    let info = parse_d80_patch_info(&table).unwrap();
    assert_eq!(info.adid_address, 0x0020_1940);
    assert_eq!(info.patch_address, 0x001e_0000);
    assert_eq!(
        info.ext_patches().collect::<Vec<_>>(),
        vec![(0, 0x0020_b43c)]
    );
}
