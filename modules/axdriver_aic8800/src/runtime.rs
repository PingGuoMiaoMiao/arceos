use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::management::MM_TASK_ID;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const MM_START_REQUEST: u16 = (MM_TASK_ID << 10) + 2;
pub const MM_START_CONFIRM: u16 = (MM_TASK_ID << 10) + 3;
pub const MM_ADD_INTERFACE_REQUEST: u16 = (MM_TASK_ID << 10) + 6;
pub const MM_ADD_INTERFACE_CONFIRM: u16 = (MM_TASK_ID << 10) + 7;

const MM_START_PARAMETER_SIZE: usize = 72;
const MM_ADD_INTERFACE_PARAMETER_SIZE: usize = 10;
const MM_ADD_INTERFACE_CONFIRMATION_SIZE: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeEncodeError {
    pub required: usize,
    pub available: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDecodeError {
    pub required: usize,
    pub available: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddInterfaceConfirmation {
    pub status: u8,
    pub interface_index: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicRuntimeError<E> {
    Encode(RuntimeEncodeError),
    Decode(RuntimeDecodeError),
    Transaction(AicTransactionError<E>),
}

pub fn encode_d80_start_parameters(output: &mut [u8]) -> Result<usize, RuntimeEncodeError> {
    if output.len() < MM_START_PARAMETER_SIZE {
        return Err(RuntimeEncodeError {
            required: MM_START_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    let output = &mut output[..MM_START_PARAMETER_SIZE];
    output.fill(0);
    output[64..68].copy_from_slice(&300_u32.to_le_bytes());
    output[68..70].copy_from_slice(&20_u16.to_le_bytes());
    Ok(MM_START_PARAMETER_SIZE)
}

pub fn encode_station_interface_parameters(
    output: &mut [u8],
    mac_address: [u8; 6],
) -> Result<usize, RuntimeEncodeError> {
    if output.len() < MM_ADD_INTERFACE_PARAMETER_SIZE {
        return Err(RuntimeEncodeError {
            required: MM_ADD_INTERFACE_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    let output = &mut output[..MM_ADD_INTERFACE_PARAMETER_SIZE];
    output.fill(0);
    output[0] = 0; // MM_STA
    output[2..8].copy_from_slice(&mac_address);
    output[8] = 0; // p2p = false
    Ok(MM_ADD_INTERFACE_PARAMETER_SIZE)
}

pub fn decode_mm_add_interface_confirmation(
    parameter: &[u8],
) -> Result<AddInterfaceConfirmation, RuntimeDecodeError> {
    if parameter.len() < MM_ADD_INTERFACE_CONFIRMATION_SIZE {
        return Err(RuntimeDecodeError {
            required: MM_ADD_INTERFACE_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }
    Ok(AddInterfaceConfirmation {
        status: parameter[0],
        interface_index: parameter[1],
    })
}

pub struct AicRuntimeClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    parameter_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicRuntimeClient<'a, I>
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

    pub fn start(&mut self) -> Result<(), AicRuntimeError<<I as AicCommandIo>::Error>> {
        let length =
            encode_d80_start_parameters(self.parameter_storage).map_err(AicRuntimeError::Encode)?;
        execute_config_command(
            self.io,
            self.product,
            MM_START_REQUEST,
            MM_START_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicRuntimeError::Transaction)?;
        Ok(())
    }

    pub fn add_station_interface(
        &mut self,
        mac_address: [u8; 6],
    ) -> Result<AddInterfaceConfirmation, AicRuntimeError<<I as AicCommandIo>::Error>> {
        let length = encode_station_interface_parameters(self.parameter_storage, mac_address)
            .map_err(AicRuntimeError::Encode)?;
        let response = execute_config_command(
            self.io,
            self.product,
            MM_ADD_INTERFACE_REQUEST,
            MM_ADD_INTERFACE_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicRuntimeError::Transaction)?;
        decode_mm_add_interface_confirmation(response.parameter).map_err(AicRuntimeError::Decode)
    }
}
