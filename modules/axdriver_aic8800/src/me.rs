use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const ME_TASK_ID: u16 = 5;
pub const ME_CONFIG_REQUEST: u16 = ME_TASK_ID << 10;
pub const ME_CONFIG_CONFIRM: u16 = ME_CONFIG_REQUEST + 1;
pub const ME_CHANNEL_CONFIG_REQUEST: u16 = ME_CONFIG_REQUEST + 2;
pub const ME_CHANNEL_CONFIG_CONFIRM: u16 = ME_CONFIG_REQUEST + 3;
pub const ME_SET_CONTROL_PORT_REQUEST: u16 = ME_CONFIG_REQUEST + 4;
pub const ME_SET_CONTROL_PORT_CONFIRM: u16 = ME_CONFIG_REQUEST + 5;

const ME_CONFIG_PARAMETER_SIZE: usize = 112;
const ME_CHANNEL_CONFIG_PARAMETER_SIZE: usize = 254;
const ME_SET_CONTROL_PORT_PARAMETER_SIZE: usize = 2;
const MAC_CHANNEL_DEFINITION_SIZE: usize = 6;
const CHANNEL_2GHZ_CAPACITY: usize = 14;
const CHANNEL_5GHZ_CAPACITY: usize = 28;
const BOARD_CHANNEL_POWER_DBM: u8 = 20;

const CHANNELS_2GHZ: [u16; 14] = [
    2412, 2417, 2422, 2427, 2432, 2437, 2442, 2447, 2452, 2457, 2462, 2467, 2472, 2484,
];
const CHANNELS_5GHZ: [u16; 25] = [
    5180, 5200, 5220, 5240, 5260, 5280, 5300, 5320, 5500, 5520, 5540, 5560, 5580, 5600, 5620, 5640,
    5660, 5680, 5700, 5720, 5745, 5765, 5785, 5805, 5825,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeEncodeError {
    pub required: usize,
    pub available: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicMeError<E> {
    Encode(MeEncodeError),
    Transaction(AicTransactionError<E>),
}

pub fn encode_d80_board_me_config(output: &mut [u8]) -> Result<usize, MeEncodeError> {
    if output.len() < ME_CONFIG_PARAMETER_SIZE {
        return Err(MeEncodeError {
            required: ME_CONFIG_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    let output = &mut output[..ME_CONFIG_PARAMETER_SIZE];
    output.fill(0);

    // mac_htcapability, observed through `iw phy` after the fixed driver's
    // D80 dynamic-parameter handling.
    output[0..2].copy_from_slice(&0x0963_u16.to_le_bytes());
    output[2] = 0x1f;
    output[3..19].copy_from_slice(&[0xff, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0x96, 0, 1, 0, 0, 0]);

    // mac_vhtcapability.
    output[32..36].copy_from_slice(&0x0398_7131_u32.to_le_bytes());
    output[36..38].copy_from_slice(&0xfffe_u16.to_le_bytes());
    output[38..40].copy_from_slice(&390_u16.to_le_bytes());
    output[40..42].copy_from_slice(&0xfffe_u16.to_le_bytes());
    output[42..44].copy_from_slice(&390_u16.to_le_bytes());

    // mac_hecapability.
    output[44..50].copy_from_slice(&[0x00, 0x00, 0x02, 0x00, 0x00, 0x00]);
    output[50..61].copy_from_slice(&[
        0x06, 0xe0, 0x2b, 0x58, 0x0d, 0xc0, 0xcf, 0x00, 0x02, 0x30, 0x00,
    ]);
    output[62..74].copy_from_slice(&[
        0xfe, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ]);
    output[74..78].copy_from_slice(&[0x38, 0x1c, 0xc7, 0x01]);

    output[100..102].copy_from_slice(&1000_u16.to_le_bytes());
    output[102] = 2; // PHY_CHNL_BW_80
    output[103] = 1; // ht_supp
    output[104] = 1; // vht_supp
    output[105] = 1; // he_supp
    output[106] = 0; // he_ul_on
    output[107] = 1; // ps_on
    output[108] = 1; // ant_div_on
    output[109] = 0; // dpsm

    Ok(ME_CONFIG_PARAMETER_SIZE)
}

fn encode_channel(output: &mut [u8], index: usize, frequency: u16, band: u8) {
    let offset = index * MAC_CHANNEL_DEFINITION_SIZE;
    output[offset..offset + 2].copy_from_slice(&frequency.to_le_bytes());
    output[offset + 2] = band;
    output[offset + 3] = 0;
    output[offset + 4] = BOARD_CHANNEL_POWER_DBM;
    output[offset + 5] = 0;
}

pub fn encode_d80_board_channel_config(output: &mut [u8]) -> Result<usize, MeEncodeError> {
    if output.len() < ME_CHANNEL_CONFIG_PARAMETER_SIZE {
        return Err(MeEncodeError {
            required: ME_CHANNEL_CONFIG_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    let output = &mut output[..ME_CHANNEL_CONFIG_PARAMETER_SIZE];
    output.fill(0);

    for (index, frequency) in CHANNELS_2GHZ.iter().copied().enumerate() {
        encode_channel(output, index, frequency, 0);
    }
    for (index, frequency) in CHANNELS_5GHZ.iter().copied().enumerate() {
        encode_channel(output, CHANNEL_2GHZ_CAPACITY + index, frequency, 1);
    }
    output[(CHANNEL_2GHZ_CAPACITY + CHANNEL_5GHZ_CAPACITY) * MAC_CHANNEL_DEFINITION_SIZE] =
        CHANNELS_2GHZ.len() as u8;
    output[(CHANNEL_2GHZ_CAPACITY + CHANNEL_5GHZ_CAPACITY) * MAC_CHANNEL_DEFINITION_SIZE + 1] =
        CHANNELS_5GHZ.len() as u8;

    Ok(ME_CHANNEL_CONFIG_PARAMETER_SIZE)
}

pub fn encode_set_control_port_parameters(
    output: &mut [u8],
    station_index: u8,
    open: bool,
) -> Result<usize, MeEncodeError> {
    if output.len() < ME_SET_CONTROL_PORT_PARAMETER_SIZE {
        return Err(MeEncodeError {
            required: ME_SET_CONTROL_PORT_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    output[0] = station_index;
    output[1] = u8::from(open);
    Ok(ME_SET_CONTROL_PORT_PARAMETER_SIZE)
}

pub struct AicMeClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    parameter_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicMeClient<'a, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    pub fn new(
        io: &'a mut I,
        product: Aic8800Product,
        parameter_storage: &'a mut [u8],
        transmit_storage: &'a mut [u8],
        receive_storage: &'a mut [u8],
        response_timeout_ms: u32,
    ) -> Self {
        Self {
            io,
            product,
            parameter_storage,
            transmit_storage,
            receive_storage,
            response_timeout_ms,
        }
    }

    pub fn configure_d80_board(&mut self) -> Result<(), AicMeError<<I as AicCommandIo>::Error>> {
        let length =
            encode_d80_board_me_config(self.parameter_storage).map_err(AicMeError::Encode)?;
        execute_config_command(
            self.io,
            self.product,
            ME_CONFIG_REQUEST,
            ME_CONFIG_CONFIRM,
            ME_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicMeError::Transaction)?;

        let length =
            encode_d80_board_channel_config(self.parameter_storage).map_err(AicMeError::Encode)?;
        execute_config_command(
            self.io,
            self.product,
            ME_CHANNEL_CONFIG_REQUEST,
            ME_CHANNEL_CONFIG_CONFIRM,
            ME_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicMeError::Transaction)?;

        Ok(())
    }

    pub fn set_control_port(
        &mut self,
        station_index: u8,
        open: bool,
    ) -> Result<(), AicMeError<<I as AicCommandIo>::Error>> {
        let length =
            encode_set_control_port_parameters(self.parameter_storage, station_index, open)
                .map_err(AicMeError::Encode)?;
        execute_config_command(
            self.io,
            self.product,
            ME_SET_CONTROL_PORT_REQUEST,
            ME_SET_CONTROL_PORT_CONFIRM,
            ME_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map(|_| ())
        .map_err(AicMeError::Transaction)
    }
}
