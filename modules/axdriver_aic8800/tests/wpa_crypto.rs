use axdriver_aic8800::wpa_crypto::{CredentialError, derive_wpa2_ccmp_ptk, derive_wpa2_psk};

#[test]
fn derives_the_ieee_pmk_and_a_fixed_wpa2_ccmp_ptk() {
    let pmk = derive_wpa2_psk(b"password", b"IEEE").unwrap();
    assert_eq!(
        pmk.as_bytes(),
        &[
            0xf4, 0x2c, 0x6f, 0xc5, 0x2d, 0xf0, 0xeb, 0xef, 0x9e, 0xbb, 0x4b, 0x90, 0xb3, 0x8a,
            0x5f, 0x90, 0x2e, 0x83, 0xfe, 0x1b, 0x13, 0x5a, 0x70, 0xe2, 0x3a, 0xed, 0x76, 0x2e,
            0x97, 0x10, 0xa1, 0x2e,
        ]
    );

    let authenticator_address = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let station_address = [0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb];
    let authenticator_nonce = core::array::from_fn(|index| index as u8);
    let station_nonce = core::array::from_fn(|index| (index + 32) as u8);
    let ptk = derive_wpa2_ccmp_ptk(
        &pmk,
        authenticator_address,
        station_address,
        authenticator_nonce,
        station_nonce,
    );

    assert_eq!(
        ptk.key_confirmation_key(),
        &[
            0x85, 0xc9, 0x8e, 0xca, 0x56, 0x14, 0x56, 0x29, 0x35, 0x9a, 0xc8, 0x83, 0x0b, 0xb6,
            0x6a, 0x59
        ]
    );
    assert_eq!(
        ptk.key_encryption_key(),
        &[
            0xc5, 0x56, 0x2d, 0x47, 0x3f, 0xdd, 0xcb, 0x4e, 0xee, 0x9c, 0xe4, 0xde, 0x54, 0xe1,
            0xcb, 0x1a
        ]
    );
    assert_eq!(
        ptk.temporal_key(),
        &[
            0x12, 0xcd, 0xd4, 0x44, 0x83, 0x25, 0xc8, 0x40, 0x79, 0xab, 0xcd, 0x76, 0xb1, 0xb8,
            0x9f, 0x8f
        ]
    );
}

#[test]
fn validates_passphrase_and_ssid_lengths_before_derivation() {
    assert_eq!(
        derive_wpa2_psk(b"short", b"IEEE").unwrap_err(),
        CredentialError::PassphraseLength {
            minimum: 8,
            maximum: 63,
            actual: 5,
        }
    );
    assert_eq!(
        derive_wpa2_psk(b"password", b"").unwrap_err(),
        CredentialError::SsidLength {
            minimum: 1,
            maximum: 32,
            actual: 0,
        }
    );
}
