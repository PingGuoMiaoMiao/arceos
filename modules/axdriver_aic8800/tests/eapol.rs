use axdriver_aic8800::eapol::{
    EapolKeyBuildError, EapolKeyDecodeError, build_wpa2_psk_ccmp_message_2,
    build_wpa2_psk_ccmp_message_4, parse_wpa2_psk_ccmp_key_data, parse_wpa2_psk_ccmp_message_1,
    parse_wpa2_psk_ccmp_message_3,
};

fn decode_hex<const N: usize>(text: &str) -> [u8; N] {
    assert_eq!(text.len(), N * 2);
    core::array::from_fn(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap())
}
use axdriver_aic8800::wpa_crypto::{derive_wpa2_ccmp_ptk, derive_wpa2_psk};

const CAPTURED_MESSAGE_1: [u8; 99] = [
    0x02, 0x03, 0x00, 0x5f, 0x02, 0x00, 0x8a, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x13, 0x04, 0x2a, 0x53, 0x34, 0xba, 0xd0, 0x9a, 0x99, 0x46, 0x77, 0x08, 0x4f, 0x79, 0x1c,
    0x08, 0xe7, 0xa7, 0x50, 0xe6, 0x4b, 0x71, 0xef, 0xe3, 0x0c, 0x1c, 0x1f, 0x40, 0x29, 0x40, 0x03,
    0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00,
];

#[test]
fn parses_the_exact_true_board_wpa2_psk_ccmp_message_1() {
    let message = parse_wpa2_psk_ccmp_message_1(&CAPTURED_MESSAGE_1).unwrap();

    assert_eq!(message.protocol_version, 2);
    assert_eq!(message.key_information, 0x008a);
    assert_eq!(message.descriptor_version, 2);
    assert_eq!(message.key_length, 16);
    assert_eq!(message.replay_counter, 1);
    assert_eq!(
        message.authenticator_nonce,
        [
            0x13, 0x04, 0x2a, 0x53, 0x34, 0xba, 0xd0, 0x9a, 0x99, 0x46, 0x77, 0x08, 0x4f, 0x79,
            0x1c, 0x08, 0xe7, 0xa7, 0x50, 0xe6, 0x4b, 0x71, 0xef, 0xe3, 0x0c, 0x1c, 0x1f, 0x40,
            0x29, 0x40, 0x03, 0x78,
        ]
    );
    assert_eq!(message.key_mic, [0; 16]);
    assert!(message.key_data.is_empty());
    assert!(message.trailing.is_empty());
}

#[test]
fn rejects_non_message_1_flags_and_invalid_lengths() {
    let mut mic_present = CAPTURED_MESSAGE_1;
    mic_present[5..7].copy_from_slice(&0x018a_u16.to_be_bytes());
    assert_eq!(
        parse_wpa2_psk_ccmp_message_1(&mic_present),
        Err(EapolKeyDecodeError::UnexpectedMessage1KeyInformation { actual: 0x018a })
    );

    let mut wrong_descriptor_version = CAPTURED_MESSAGE_1;
    wrong_descriptor_version[5..7].copy_from_slice(&0x0089_u16.to_be_bytes());
    assert_eq!(
        parse_wpa2_psk_ccmp_message_1(&wrong_descriptor_version),
        Err(EapolKeyDecodeError::UnexpectedDescriptorVersion { actual: 1 })
    );

    let mut overflowing_key_data = CAPTURED_MESSAGE_1;
    overflowing_key_data[97..99].copy_from_slice(&1_u16.to_be_bytes());
    assert_eq!(
        parse_wpa2_psk_ccmp_message_1(&overflowing_key_data),
        Err(EapolKeyDecodeError::KeyDataOverflow {
            declared: 1,
            available: 0,
        })
    );
}

#[test]
fn builds_message_2_with_the_fixed_sdk_layout_and_hmac_sha1_mic() {
    let message_1 = parse_wpa2_psk_ccmp_message_1(&CAPTURED_MESSAGE_1).unwrap();
    let pairwise_master_key = derive_wpa2_psk(b"password", b"IEEE").unwrap();
    let station_nonce = core::array::from_fn(|index| (index + 32) as u8);
    let pairwise_transient_key = derive_wpa2_ccmp_ptk(
        &pairwise_master_key,
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb],
        core::array::from_fn(|index| index as u8),
        station_nonce,
    );
    let station_rsn = [
        0x30, 0x14, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01,
        0x00, 0x00, 0x0f, 0xac, 0x02, 0x00, 0x00,
    ];
    let mut output = [0xaa_u8; 128];

    let length = build_wpa2_psk_ccmp_message_2(
        &mut output,
        &message_1,
        station_nonce,
        &station_rsn,
        &pairwise_transient_key,
    )
    .unwrap();

    assert_eq!(length, 121);
    assert_eq!(&output[..4], &[0x02, 0x03, 0x00, 0x75]);
    assert_eq!(&output[4..9], &[0x02, 0x01, 0x0a, 0x00, 0x00]);
    assert_eq!(&output[9..17], &1_u64.to_be_bytes());
    assert_eq!(&output[17..49], &station_nonce);
    assert_eq!(
        &output[81..97],
        &[
            0x0d, 0x4b, 0xeb, 0x08, 0xf2, 0xce, 0xc6, 0x33, 0x95, 0x13, 0x64, 0xe3, 0xdb, 0xb6,
            0x82, 0xe0
        ]
    );
    assert_eq!(&output[97..99], &22_u16.to_be_bytes());
    assert_eq!(&output[99..length], &station_rsn);
    assert_eq!(&output[length..], &[0xaa; 7]);

    assert_eq!(
        build_wpa2_psk_ccmp_message_2(
            &mut output[..120],
            &message_1,
            station_nonce,
            &station_rsn,
            &pairwise_transient_key,
        ),
        Err(EapolKeyBuildError::OutputTooShort {
            required: 121,
            available: 120,
        })
    );
}

#[test]
fn parses_and_authenticates_a_fixed_wpa2_psk_ccmp_message_3() {
    let mut message_1_frame = CAPTURED_MESSAGE_1;
    let authenticator_nonce = core::array::from_fn(|index| index as u8);
    message_1_frame[17..49].copy_from_slice(&authenticator_nonce);
    let message_1 = parse_wpa2_psk_ccmp_message_1(&message_1_frame).unwrap();
    let pairwise_master_key = derive_wpa2_psk(b"password", b"IEEE").unwrap();
    let pairwise_transient_key = derive_wpa2_ccmp_ptk(
        &pairwise_master_key,
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb],
        authenticator_nonce,
        core::array::from_fn(|index| (index + 32) as u8),
    );
    let frame = decode_hex::<123>(
        "020300770213ca00100000000000000002000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1111111111111111111111111111111100000000000000000000000000000000643a8e807b3be334588b2e7063485ce10018000102030405060708090a0b0c0d0e0f1011121314151617",
    );

    let message_3 =
        parse_wpa2_psk_ccmp_message_3(&frame, &message_1, &pairwise_transient_key).unwrap();

    assert_eq!(message_3.key_information, 0x13ca);
    assert_eq!(message_3.key_length, 16);
    assert_eq!(message_3.replay_counter, 2);
    assert_eq!(message_3.authenticator_nonce, authenticator_nonce);
    assert_eq!(message_3.key_data, &(0_u8..24).collect::<Vec<_>>());
    assert!(message_3.encrypted_key_data);
    assert!(message_3.install);
    assert!(message_3.secure);

    let mut damaged = frame;
    damaged[81] ^= 1;
    assert_eq!(
        parse_wpa2_psk_ccmp_message_3(&damaged, &message_1, &pairwise_transient_key,),
        Err(EapolKeyDecodeError::Message3MicMismatch)
    );
}

#[test]
fn builds_message_4_with_the_wpa_supplicant_2_10_layout() {
    let mut message_1_frame = CAPTURED_MESSAGE_1;
    let authenticator_nonce = core::array::from_fn(|index| index as u8);
    message_1_frame[17..49].copy_from_slice(&authenticator_nonce);
    let message_1 = parse_wpa2_psk_ccmp_message_1(&message_1_frame).unwrap();
    let pairwise_master_key = derive_wpa2_psk(b"password", b"IEEE").unwrap();
    let pairwise_transient_key = derive_wpa2_ccmp_ptk(
        &pairwise_master_key,
        [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        [0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb],
        authenticator_nonce,
        core::array::from_fn(|index| (index + 32) as u8),
    );
    let message_3_frame = decode_hex::<123>(
        "020300770213ca00100000000000000002000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f1111111111111111111111111111111100000000000000000000000000000000643a8e807b3be334588b2e7063485ce10018000102030405060708090a0b0c0d0e0f1011121314151617",
    );
    let message_3 =
        parse_wpa2_psk_ccmp_message_3(&message_3_frame, &message_1, &pairwise_transient_key)
            .unwrap();
    let expected = decode_hex::<99>(
        "0203005f02030a00000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008e8a46e744990e81e9e1a297ca353cff0000",
    );
    let mut output = [0xaa_u8; 128];

    let length =
        build_wpa2_psk_ccmp_message_4(&mut output, &message_3, &pairwise_transient_key).unwrap();

    assert_eq!(length, expected.len());
    assert_eq!(&output[..length], &expected);
    assert_eq!(&output[length..], &[0xaa; 29]);
}

#[test]
fn parses_the_wpa_supplicant_2_10_rsn_and_gtk_key_data_layout() {
    let access_point_rsn = [
        0x30, 0x18, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x02,
        0x00, 0x00, 0x0f, 0xac, 0x02, 0x00, 0x0f, 0xac, 0x08, 0x8c, 0x40,
    ];
    let key_data = [
        0x30, 0x18, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x02,
        0x00, 0x00, 0x0f, 0xac, 0x02, 0x00, 0x0f, 0xac, 0x08, 0x8c, 0x40, 0xdd, 0x16, 0x00, 0x0f,
        0xac, 0x01, 0x01, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a,
        0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0xdd, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let parsed = parse_wpa2_psk_ccmp_key_data(&key_data, &access_point_rsn).unwrap();

    assert_eq!(parsed.group_key_index, 1);
    assert!(!parsed.group_key_transmit);
    assert_eq!(
        parsed.group_temporal_key,
        [
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f,
        ]
    );
}
