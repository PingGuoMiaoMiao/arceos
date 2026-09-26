use aes::Aes128;
use aes::cipher::{BlockDecrypt, KeyInit as AesKeyInit, generic_array::GenericArray};
use hmac::{Hmac, KeyInit as HmacKeyInit, Mac};
use pbkdf2::pbkdf2_hmac_array;
use sha1::Sha1;
use zeroize::Zeroize;

const PASSPHRASE_MINIMUM_LENGTH: usize = 8;
const PASSPHRASE_MAXIMUM_LENGTH: usize = 63;
const SSID_MINIMUM_LENGTH: usize = 1;
const SSID_MAXIMUM_LENGTH: usize = 32;
const PBKDF2_ROUNDS: u32 = 4096;
const PAIRWISE_KEY_EXPANSION_LABEL: &[u8] = b"Pairwise key expansion\0";
const PTK_LENGTH: usize = 48;
const SHA1_OUTPUT_LENGTH: usize = 20;

type HmacSha1 = Hmac<Sha1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    PassphraseLength {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    SsidLength {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyDataUnwrapError {
    InvalidEncryptedLength { actual: usize },
    OutputTooShort { required: usize, available: usize },
    IntegrityMismatch,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PairwiseMasterKey([u8; 32]);

impl PairwiseMasterKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for PairwiseMasterKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Wpa2CcmpPairwiseTransientKey {
    key_confirmation_key: [u8; 16],
    key_encryption_key: [u8; 16],
    temporal_key: [u8; 16],
}

impl Wpa2CcmpPairwiseTransientKey {
    pub fn key_confirmation_key(&self) -> &[u8; 16] {
        &self.key_confirmation_key
    }

    pub fn key_encryption_key(&self) -> &[u8; 16] {
        &self.key_encryption_key
    }

    pub fn temporal_key(&self) -> &[u8; 16] {
        &self.temporal_key
    }
}

impl Drop for Wpa2CcmpPairwiseTransientKey {
    fn drop(&mut self) {
        self.key_confirmation_key.zeroize();
        self.key_encryption_key.zeroize();
        self.temporal_key.zeroize();
    }
}

pub fn derive_wpa2_psk(
    passphrase: &[u8],
    ssid: &[u8],
) -> Result<PairwiseMasterKey, CredentialError> {
    if !(PASSPHRASE_MINIMUM_LENGTH..=PASSPHRASE_MAXIMUM_LENGTH).contains(&passphrase.len()) {
        return Err(CredentialError::PassphraseLength {
            minimum: PASSPHRASE_MINIMUM_LENGTH,
            maximum: PASSPHRASE_MAXIMUM_LENGTH,
            actual: passphrase.len(),
        });
    }
    if !(SSID_MINIMUM_LENGTH..=SSID_MAXIMUM_LENGTH).contains(&ssid.len()) {
        return Err(CredentialError::SsidLength {
            minimum: SSID_MINIMUM_LENGTH,
            maximum: SSID_MAXIMUM_LENGTH,
            actual: ssid.len(),
        });
    }
    Ok(PairwiseMasterKey(pbkdf2_hmac_array::<Sha1, 32>(
        passphrase,
        ssid,
        PBKDF2_ROUNDS,
    )))
}

pub fn derive_wpa2_ccmp_ptk(
    pairwise_master_key: &PairwiseMasterKey,
    authenticator_address: [u8; 6],
    station_address: [u8; 6],
    authenticator_nonce: [u8; 32],
    station_nonce: [u8; 32],
) -> Wpa2CcmpPairwiseTransientKey {
    let mut context = [0_u8; 76];
    let (first_address, second_address) = ordered(&authenticator_address, &station_address);
    context[..6].copy_from_slice(first_address);
    context[6..12].copy_from_slice(second_address);
    let (first_nonce, second_nonce) = ordered(&authenticator_nonce, &station_nonce);
    context[12..44].copy_from_slice(first_nonce);
    context[44..76].copy_from_slice(second_nonce);

    let mut expanded = [0_u8; SHA1_OUTPUT_LENGTH * 3];
    for counter in 0_u8..3 {
        let mut mac = <HmacSha1 as HmacKeyInit>::new_from_slice(pairwise_master_key.as_bytes())
            .expect("HMAC-SHA1 accepts a 32-byte PMK");
        mac.update(PAIRWISE_KEY_EXPANSION_LABEL);
        mac.update(&context);
        mac.update(&[counter]);
        let digest = mac.finalize().into_bytes();
        let start = usize::from(counter) * SHA1_OUTPUT_LENGTH;
        expanded[start..start + SHA1_OUTPUT_LENGTH].copy_from_slice(&digest);
    }

    let result = Wpa2CcmpPairwiseTransientKey {
        key_confirmation_key: expanded[..16].try_into().unwrap(),
        key_encryption_key: expanded[16..32].try_into().unwrap(),
        temporal_key: expanded[32..PTK_LENGTH].try_into().unwrap(),
    };
    context.zeroize();
    expanded.zeroize();
    result
}

pub fn unwrap_wpa2_key_data(
    output: &mut [u8],
    encrypted: &[u8],
    key_encryption_key: &[u8; 16],
) -> Result<usize, KeyDataUnwrapError> {
    if encrypted.len() < 16 || encrypted.len() % 8 != 0 {
        return Err(KeyDataUnwrapError::InvalidEncryptedLength {
            actual: encrypted.len(),
        });
    }
    let plain_length = encrypted.len() - 8;
    if output.len() < plain_length {
        return Err(KeyDataUnwrapError::OutputTooShort {
            required: plain_length,
            available: output.len(),
        });
    }

    let mut integrity = [0_u8; 8];
    integrity.copy_from_slice(&encrypted[..8]);
    output[..plain_length].copy_from_slice(&encrypted[8..]);
    let cipher = Aes128::new(&GenericArray::from(*key_encryption_key));
    let blocks = plain_length / 8;
    let mut decrypted = GenericArray::from([0_u8; 16]);
    for round in (0..=5).rev() {
        for block_index in (1..=blocks).rev() {
            decrypted[..8].copy_from_slice(&integrity);
            let counter = (blocks * round + block_index) as u64;
            let counter_bytes = counter.to_be_bytes();
            for index in 0..8 {
                decrypted[index] ^= counter_bytes[index];
            }
            let start = (block_index - 1) * 8;
            decrypted[8..].copy_from_slice(&output[start..start + 8]);
            cipher.decrypt_block(&mut decrypted);
            integrity.copy_from_slice(&decrypted[..8]);
            output[start..start + 8].copy_from_slice(&decrypted[8..]);
        }
    }

    let difference = integrity
        .iter()
        .fold(0_u8, |difference, byte| difference | (byte ^ 0xa6));
    integrity.zeroize();
    decrypted.iter_mut().for_each(|byte| *byte = 0);
    if difference != 0 {
        output[..plain_length].zeroize();
        return Err(KeyDataUnwrapError::IntegrityMismatch);
    }
    Ok(plain_length)
}

fn ordered<'a, const N: usize>(
    first: &'a [u8; N],
    second: &'a [u8; N],
) -> (&'a [u8; N], &'a [u8; N]) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}
