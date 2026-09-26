use axdriver_aic8800::rsn::{
    RSN_AKM_PSK, RSN_CIPHER_CCMP, build_wpa2_psk_ccmp_station_information_element,
    parse_rsn_information_element,
};

const HOTSPOT_RSN: [u8; 26] = [
    0x30, 0x18, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x02, 0x00,
    0x00, 0x0f, 0xac, 0x02, 0x00, 0x0f, 0xac, 0x08, 0x8c, 0x40,
];

#[test]
fn parses_the_observed_meizu_hotspot_rsn_information_element() {
    let rsn = parse_rsn_information_element(&HOTSPOT_RSN).unwrap();

    assert_eq!(rsn.group_cipher, RSN_CIPHER_CCMP);
    assert_eq!(rsn.pairwise_cipher_suites, &RSN_CIPHER_CCMP);
    assert_eq!(
        rsn.authentication_key_management_suites,
        &[
            RSN_AKM_PSK[0],
            RSN_AKM_PSK[1],
            RSN_AKM_PSK[2],
            RSN_AKM_PSK[3],
            0,
            0x0f,
            0xac,
            8
        ]
    );
    assert_eq!(rsn.capabilities, 0x408c);
}

#[test]
fn builds_a_wpa2_psk_ccmp_station_rsn_information_element() {
    let ap_rsn = parse_rsn_information_element(&HOTSPOT_RSN).unwrap();
    let mut output = [0xaa_u8; 32];

    let length = build_wpa2_psk_ccmp_station_information_element(&mut output, &ap_rsn).unwrap();

    assert_eq!(length, 22);
    assert_eq!(
        &output[..length],
        &[
            0x30, 0x14, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04, 0x01, 0x00, 0x00, 0x0f, 0xac, 0x04,
            0x01, 0x00, 0x00, 0x0f, 0xac, 0x02, 0x00, 0x00,
        ]
    );
    assert_eq!(&output[length..], &[0xaa; 10]);
}

#[test]
fn rejects_truncated_or_unsupported_rsn_information_elements() {
    assert!(parse_rsn_information_element(&HOTSPOT_RSN[..25]).is_err());

    let mut unsupported = HOTSPOT_RSN;
    unsupported[13] = 2;
    let ap_rsn = parse_rsn_information_element(&unsupported).unwrap();
    let mut output = [0_u8; 22];
    assert!(build_wpa2_psk_ccmp_station_information_element(&mut output, &ap_rsn).is_err());
}
