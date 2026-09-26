use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::management::MM_TASK_ID;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};
use zeroize::Zeroize;

pub const MM_KEY_ADD_REQUEST: u16 = 36;
pub const MM_KEY_ADD_CONFIRM: u16 = 37;

const MM_KEY_ADD_PARAMETER_LENGTH: usize = 44;
const MAC_CIPHER_CCMP: u8 = 2;
const CCMP_TEMPORAL_KEY_LENGTH: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyAddConfirmation {
    pub status: u8,
    pub hardware_key_index: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyDecodeError {
    TruncatedConfirmation { required: usize, available: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicKeyError<E> {
    Transaction(AicTransactionError<E>),
    Decode(KeyDecodeError),
}

fn decode_key_add_confirmation(parameter: &[u8]) -> Result<KeyAddConfirmation, KeyDecodeError> {
    if parameter.len() < 2 {
        return Err(KeyDecodeError::TruncatedConfirmation {
            required: 2,
            available: parameter.len(),
        });
    }
    Ok(KeyAddConfirmation {
        status: parameter[0],
        hardware_key_index: parameter[1],
    })
}

pub struct AicKeyClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicKeyClient<'a, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    pub fn new(
        io: &'a mut I,
        product: Aic8800Product,
        transmit_storage: &'a mut [u8],
        receive_storage: &'a mut [u8],
        response_timeout_ms: u32,
    ) -> Self {
        Self {
            io,
            product,
            transmit_storage,
            receive_storage,
            response_timeout_ms,
        }
    }

    pub fn install_pairwise_ccmp(
        &mut self,
        interface_index: u8,
        station_index: u8,
        temporal_key: &[u8; CCMP_TEMPORAL_KEY_LENGTH],
    ) -> Result<KeyAddConfirmation, AicKeyError<<I as AicCommandIo>::Error>> {
        self.install_ccmp(interface_index, station_index, 0, true, temporal_key)
    }

    pub fn install_group_ccmp(
        &mut self,
        interface_index: u8,
        key_index: u8,
        temporal_key: &[u8; CCMP_TEMPORAL_KEY_LENGTH],
    ) -> Result<KeyAddConfirmation, AicKeyError<<I as AicCommandIo>::Error>> {
        self.install_ccmp(interface_index, 0xff, key_index, false, temporal_key)
    }

    fn install_ccmp(
        &mut self,
        interface_index: u8,
        station_index: u8,
        key_index: u8,
        pairwise: bool,
        temporal_key: &[u8; CCMP_TEMPORAL_KEY_LENGTH],
    ) -> Result<KeyAddConfirmation, AicKeyError<<I as AicCommandIo>::Error>> {
        let mut parameter = [0_u8; MM_KEY_ADD_PARAMETER_LENGTH];
        parameter[0] = key_index;
        parameter[1] = station_index;
        parameter[4] = CCMP_TEMPORAL_KEY_LENGTH as u8;
        parameter[8..8 + CCMP_TEMPORAL_KEY_LENGTH].copy_from_slice(temporal_key);
        parameter[40] = MAC_CIPHER_CCMP;
        parameter[41] = interface_index;
        parameter[43] = u8::from(pairwise);
        let response = execute_config_command(
            self.io,
            self.product,
            MM_KEY_ADD_REQUEST,
            MM_KEY_ADD_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &parameter,
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        );
        parameter.zeroize();
        let response = response.map_err(AicKeyError::Transaction)?;
        decode_key_add_confirmation(response.parameter).map_err(AicKeyError::Decode)
    }
}
