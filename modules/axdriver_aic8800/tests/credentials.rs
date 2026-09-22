use axdriver_aic8800::credentials::{
    WifiCredentialError, parse_wifi_credentials, wifi_credential_wire_length,
};

#[test]
fn parses_the_uart_wifi_credential_frame_and_verifies_its_crc32() {
    let passphrase = b"test-passphrase";
    let station_nonce: [u8; 32] = core::array::from_fn(|index| index as u8);
    let mut frame = Vec::new();
    frame.push(passphrase.len() as u8);
    frame.extend_from_slice(passphrase);
    frame.extend_from_slice(&station_nonce);
    frame.extend_from_slice(&0x61dc_2b2e_u32.to_le_bytes());

    assert_eq!(wifi_credential_wire_length(frame[0]).unwrap(), frame.len());
    let credentials = parse_wifi_credentials(&frame).unwrap();
    assert_eq!(credentials.passphrase, passphrase);
    assert_eq!(credentials.station_nonce, station_nonce);
}

#[test]
fn rejects_a_crc_mismatch_and_lengths_outside_the_fixed_sdk_range() {
    let short = [7_u8];
    assert_eq!(
        wifi_credential_wire_length(short[0]),
        Err(WifiCredentialError::InvalidPassphraseLength { length: 7 })
    );

    let mut damaged = Vec::new();
    damaged.push(8);
    damaged.extend_from_slice(b"12345678");
    damaged.extend_from_slice(&[0; 32]);
    damaged.extend_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        parse_wifi_credentials(&damaged),
        Err(WifiCredentialError::ChecksumMismatch { .. })
    ));
}
