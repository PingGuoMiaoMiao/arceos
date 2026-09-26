use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::management::MM_TASK_ID;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const MM_SET_RF_CALIB_REQUEST: u16 = (MM_TASK_ID << 10) + 105;
pub const MM_SET_RF_CALIB_CONFIRM: u16 = (MM_TASK_ID << 10) + 106;
pub const MM_SET_TX_POWER_INDEX_LEVEL_REQUEST: u16 = (MM_TASK_ID << 10) + 119;
pub const MM_SET_TX_POWER_INDEX_LEVEL_CONFIRM: u16 = (MM_TASK_ID << 10) + 120;

const TX_POWER_LEVEL_REQUEST_UNION_SIZE: usize = 95;
const RF_CALIBRATION_REQUEST_SIZE: usize = 24;
const RF_CALIBRATION_CONFIRMATION_SIZE: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D80TxPowerLevelV3 {
    pub enabled: bool,
    pub level_11b_11ag_2g4: [i8; 12],
    pub level_11n_11ac_2g4: [i8; 10],
    pub level_11ax_2g4: [i8; 12],
    pub level_11a_5g: [i8; 12],
    pub level_11n_11ac_5g: [i8; 10],
    pub level_11ax_5g: [i8; 12],
}

pub const D80_BOARD_TX_POWER_LEVEL_V3: D80TxPowerLevelV3 = D80TxPowerLevelV3 {
    enabled: true,
    level_11b_11ag_2g4: [18, 18, 18, 18, 18, 18, 18, 18, 16, 16, 15, 15],
    level_11n_11ac_2g4: [18, 18, 18, 18, 16, 16, 15, 15, 14, 14],
    level_11ax_2g4: [18, 18, 18, 18, 16, 16, 15, 15, 14, 14, 13, 13],
    level_11a_5g: [
        i8::MIN,
        i8::MIN,
        i8::MIN,
        i8::MIN,
        18,
        18,
        18,
        18,
        16,
        16,
        15,
        15,
    ],
    level_11n_11ac_5g: [18, 18, 18, 18, 16, 16, 15, 15, 14, 14],
    level_11ax_5g: [18, 18, 18, 18, 16, 16, 14, 14, 13, 13, 12, 12],
};

fn copy_signed<const N: usize>(output: &mut [u8], offset: &mut usize, values: &[i8; N]) {
    for value in values {
        output[*offset] = *value as u8;
        *offset += 1;
    }
}

pub fn encode_d80_tx_power_level_request(
    levels: &D80TxPowerLevelV3,
) -> [u8; TX_POWER_LEVEL_REQUEST_UNION_SIZE] {
    let mut output = [0_u8; TX_POWER_LEVEL_REQUEST_UNION_SIZE];
    output[0] = u8::from(levels.enabled);
    let mut offset = 1;
    copy_signed(&mut output, &mut offset, &levels.level_11b_11ag_2g4);
    copy_signed(&mut output, &mut offset, &levels.level_11n_11ac_2g4);
    copy_signed(&mut output, &mut offset, &levels.level_11ax_2g4);
    copy_signed(&mut output, &mut offset, &levels.level_11a_5g);
    copy_signed(&mut output, &mut offset, &levels.level_11n_11ac_5g);
    copy_signed(&mut output, &mut offset, &levels.level_11ax_5g);
    output
}

pub fn encode_d80_rf_calibration_request() -> [u8; RF_CALIBRATION_REQUEST_SIZE] {
    let mut output = [0_u8; RF_CALIBRATION_REQUEST_SIZE];
    output[0..4].copy_from_slice(&0x0000_0f8f_u32.to_le_bytes());
    output[4..8].copy_from_slice(&0x0000_0f0f_u32.to_le_bytes());
    output[8..12].copy_from_slice(&0x0c34_c008_u32.to_le_bytes());
    output[12..16].copy_from_slice(&0_u32.to_le_bytes());
    output[16..20].copy_from_slice(&0x0026_4203_u32.to_le_bytes());
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RfCalibrationAddresses {
    pub rx_gain_24g: u32,
    pub rx_gain_5g: u32,
    pub tx_gain_24g: u32,
    pub tx_gain_5g: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RfDecodeError {
    TruncatedCalibrationConfirmation { required: usize, available: usize },
}

pub fn decode_rf_calibration_confirmation(
    parameter: &[u8],
) -> Result<RfCalibrationAddresses, RfDecodeError> {
    if parameter.len() < RF_CALIBRATION_CONFIRMATION_SIZE {
        return Err(RfDecodeError::TruncatedCalibrationConfirmation {
            required: RF_CALIBRATION_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }

    Ok(RfCalibrationAddresses {
        rx_gain_24g: u32::from_le_bytes(parameter[0..4].try_into().unwrap()),
        rx_gain_5g: u32::from_le_bytes(parameter[4..8].try_into().unwrap()),
        tx_gain_24g: u32::from_le_bytes(parameter[8..12].try_into().unwrap()),
        tx_gain_5g: u32::from_le_bytes(parameter[12..16].try_into().unwrap()),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicRfError<E> {
    UnsupportedProduct(Aic8800Product),
    Transaction(AicTransactionError<E>),
    Decode(RfDecodeError),
}

pub struct AicRfClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicRfClient<'a, I>
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

    pub fn configure_d80_board(
        &mut self,
    ) -> Result<RfCalibrationAddresses, AicRfError<<I as AicCommandIo>::Error>> {
        if self.product != Aic8800Product::Aic8800D80 {
            return Err(AicRfError::UnsupportedProduct(self.product));
        }

        let tx_power = encode_d80_tx_power_level_request(&D80_BOARD_TX_POWER_LEVEL_V3);
        execute_config_command(
            self.io,
            self.product,
            MM_SET_TX_POWER_INDEX_LEVEL_REQUEST,
            MM_SET_TX_POWER_INDEX_LEVEL_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &tx_power,
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicRfError::Transaction)?;

        let calibration = encode_d80_rf_calibration_request();
        let response = execute_config_command(
            self.io,
            self.product,
            MM_SET_RF_CALIB_REQUEST,
            MM_SET_RF_CALIB_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &calibration,
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicRfError::Transaction)?;
        decode_rf_calibration_confirmation(response.parameter).map_err(AicRfError::Decode)
    }
}
