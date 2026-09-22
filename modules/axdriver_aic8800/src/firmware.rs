use crate::device::Aic8800Product;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const DEBUG_TASK_ID: u16 = 1;
pub const DRIVER_TASK_ID: u16 = 100;
pub const DBG_MEM_READ_REQUEST: u16 = DEBUG_TASK_ID << 10;
pub const DBG_MEM_READ_CONFIRM: u16 = DBG_MEM_READ_REQUEST + 1;
pub const DBG_MEM_WRITE_REQUEST: u16 = DBG_MEM_READ_REQUEST + 2;
pub const DBG_MEM_WRITE_CONFIRM: u16 = DBG_MEM_READ_REQUEST + 3;
pub const DBG_MEM_BLOCK_WRITE_REQUEST: u16 = (DEBUG_TASK_ID << 10) + 11;
pub const DBG_MEM_BLOCK_WRITE_CONFIRM: u16 = (DEBUG_TASK_ID << 10) + 12;
pub const DBG_START_APP_REQUEST: u16 = (DEBUG_TASK_ID << 10) + 13;
pub const DBG_START_APP_CONFIRM: u16 = (DEBUG_TASK_ID << 10) + 14;
pub const MAX_FIRMWARE_BLOCK_SIZE: usize = 1024;
const FIRMWARE_BLOCK_HEADER_SIZE: usize = 8;
const DEBUG_MEMORY_READ_PARAMETER_SIZE: usize = 4;
const DEBUG_MEMORY_READ_CONFIRMATION_SIZE: usize = 8;
const DEBUG_MEMORY_WRITE_PARAMETER_SIZE: usize = 8;
const DEBUG_START_APP_PARAMETER_SIZE: usize = 8;
const DEBUG_START_APP_CONFIRMATION_SIZE: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugMemoryReadError {
    OutputTooShort { required: usize, available: usize },
    TruncatedConfirmation { required: usize, available: usize },
    AddressMismatch { expected: u32, actual: u32 },
}

pub fn encode_debug_memory_read_parameters(
    output: &mut [u8],
    memory_address: u32,
) -> Result<usize, DebugMemoryReadError> {
    if output.len() < DEBUG_MEMORY_READ_PARAMETER_SIZE {
        return Err(DebugMemoryReadError::OutputTooShort {
            required: DEBUG_MEMORY_READ_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    output[..DEBUG_MEMORY_READ_PARAMETER_SIZE].copy_from_slice(&memory_address.to_le_bytes());
    Ok(DEBUG_MEMORY_READ_PARAMETER_SIZE)
}

pub fn decode_debug_memory_read_confirmation(
    parameter: &[u8],
    expected_address: u32,
) -> Result<u32, DebugMemoryReadError> {
    if parameter.len() < DEBUG_MEMORY_READ_CONFIRMATION_SIZE {
        return Err(DebugMemoryReadError::TruncatedConfirmation {
            required: DEBUG_MEMORY_READ_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }
    let actual_address = u32::from_le_bytes(parameter[..4].try_into().unwrap());
    if actual_address != expected_address {
        return Err(DebugMemoryReadError::AddressMismatch {
            expected: expected_address,
            actual: actual_address,
        });
    }
    Ok(u32::from_le_bytes(parameter[4..8].try_into().unwrap()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugMemoryWriteError {
    OutputTooShort { required: usize, available: usize },
}

pub fn encode_debug_memory_write_parameters(
    output: &mut [u8],
    memory_address: u32,
    value: u32,
) -> Result<usize, DebugMemoryWriteError> {
    if output.len() < DEBUG_MEMORY_WRITE_PARAMETER_SIZE {
        return Err(DebugMemoryWriteError::OutputTooShort {
            required: DEBUG_MEMORY_WRITE_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    output[..4].copy_from_slice(&memory_address.to_le_bytes());
    output[4..8].copy_from_slice(&value.to_le_bytes());
    Ok(DEBUG_MEMORY_WRITE_PARAMETER_SIZE)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugStartAppError {
    OutputTooShort { required: usize, available: usize },
    TruncatedConfirmation { required: usize, available: usize },
}

pub fn encode_debug_start_app_parameters(
    output: &mut [u8],
    boot_address: u32,
    boot_type: u32,
) -> Result<usize, DebugStartAppError> {
    if output.len() < DEBUG_START_APP_PARAMETER_SIZE {
        return Err(DebugStartAppError::OutputTooShort {
            required: DEBUG_START_APP_PARAMETER_SIZE,
            available: output.len(),
        });
    }
    output[..4].copy_from_slice(&boot_address.to_le_bytes());
    output[4..8].copy_from_slice(&boot_type.to_le_bytes());
    Ok(DEBUG_START_APP_PARAMETER_SIZE)
}

pub fn decode_debug_start_app_confirmation(parameter: &[u8]) -> Result<u32, DebugStartAppError> {
    if parameter.len() < DEBUG_START_APP_CONFIRMATION_SIZE {
        return Err(DebugStartAppError::TruncatedConfirmation {
            required: DEBUG_START_APP_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }
    Ok(u32::from_le_bytes(parameter[..4].try_into().unwrap()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareBlockError {
    BlockTooLong { length: usize },
    OutputTooShort { required: usize, available: usize },
}

pub fn encode_firmware_block_write_parameters(
    output: &mut [u8],
    memory_address: u32,
    block: &[u8],
) -> Result<usize, FirmwareBlockError> {
    if block.len() > MAX_FIRMWARE_BLOCK_SIZE {
        return Err(FirmwareBlockError::BlockTooLong {
            length: block.len(),
        });
    }

    let required = FIRMWARE_BLOCK_HEADER_SIZE + block.len();
    if output.len() < required {
        return Err(FirmwareBlockError::OutputTooShort {
            required,
            available: output.len(),
        });
    }

    output[..4].copy_from_slice(&memory_address.to_le_bytes());
    output[4..8].copy_from_slice(&(block.len() as u32).to_le_bytes());
    output[8..required].copy_from_slice(block);
    Ok(required)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareUploadError<E> {
    EmptyImage,
    AddressOverflow,
    Encode(FirmwareBlockError),
    Transaction(AicTransactionError<E>),
}

pub fn upload_firmware_image<I>(
    io: &mut I,
    product: Aic8800Product,
    base_address: u32,
    image: &[u8],
    parameter_storage: &mut [u8],
    transmit_storage: &mut [u8],
    receive_storage: &mut [u8],
    response_timeout_ms: u32,
) -> Result<(), FirmwareUploadError<<I as AicCommandIo>::Error>>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    if image.is_empty() {
        return Err(FirmwareUploadError::EmptyImage);
    }

    for (block_index, block) in image.chunks(MAX_FIRMWARE_BLOCK_SIZE).enumerate() {
        let offset = block_index
            .checked_mul(MAX_FIRMWARE_BLOCK_SIZE)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(FirmwareUploadError::AddressOverflow)?;
        let address = base_address
            .checked_add(offset)
            .ok_or(FirmwareUploadError::AddressOverflow)?;
        let parameter_length =
            encode_firmware_block_write_parameters(parameter_storage, address, block)
                .map_err(FirmwareUploadError::Encode)?;

        execute_config_command(
            io,
            product,
            DBG_MEM_BLOCK_WRITE_REQUEST,
            DBG_MEM_BLOCK_WRITE_CONFIRM,
            DEBUG_TASK_ID,
            DRIVER_TASK_ID,
            &parameter_storage[..parameter_length],
            transmit_storage,
            receive_storage,
            response_timeout_ms,
        )
        .map_err(FirmwareUploadError::Transaction)?;
    }

    Ok(())
}
