use crate::device::Aic8800Product;
use crate::firmware::DRIVER_TASK_ID;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

pub const MM_TASK_ID: u16 = 0;
pub const MM_RESET_REQUEST: u16 = MM_TASK_ID << 10;
pub const MM_RESET_CONFIRM: u16 = MM_RESET_REQUEST + 1;
pub const MM_VERSION_REQUEST: u16 = MM_RESET_REQUEST + 4;
pub const MM_VERSION_CONFIRM: u16 = MM_RESET_REQUEST + 5;
pub const MM_GET_MAC_ADDRESS_REQUEST: u16 = MM_RESET_REQUEST + 115;
pub const MM_GET_MAC_ADDRESS_CONFIRM: u16 = MM_RESET_REQUEST + 116;
pub const MM_SET_STACK_START_REQUEST: u16 = MM_RESET_REQUEST + 123;
pub const MM_SET_STACK_START_CONFIRM: u16 = MM_RESET_REQUEST + 124;
pub const MM_GET_FIRMWARE_VERSION_REQUEST: u16 = MM_RESET_REQUEST + 128;
pub const MM_GET_FIRMWARE_VERSION_CONFIRM: u16 = MM_RESET_REQUEST + 129;

const MM_VERSION_CONFIRMATION_MINIMUM_SIZE: usize = 27;
const MM_MAC_ADDRESS_CONFIRMATION_SIZE: usize = 6;
const MM_STACK_START_CONFIRMATION_SIZE: usize = 2;
const MM_FIRMWARE_VERSION_TEXT_CAPACITY: usize = 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareVersion {
    pub version_lmac: u32,
    pub version_machw_1: u32,
    pub version_machw_2: u32,
    pub version_phy_1: u32,
    pub version_phy_2: u32,
    pub features: u32,
    pub max_sta_nb: u16,
    pub max_vif_nb: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackStartConfirmation {
    pub supports_5ghz: bool,
    pub vendor_info: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareBuildVersion {
    length: u8,
    bytes: [u8; MM_FIRMWARE_VERSION_TEXT_CAPACITY],
}

impl FirmwareBuildVersion {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementDecodeError {
    TruncatedVersion { required: usize, available: usize },
    TruncatedMacAddress { required: usize, available: usize },
    TruncatedStackStart { required: usize, available: usize },
    FirmwareVersionTextTooLong { length: usize, capacity: usize },
    TruncatedFirmwareVersionText { required: usize, available: usize },
}

pub fn decode_mm_version_confirmation(
    parameter: &[u8],
) -> Result<FirmwareVersion, ManagementDecodeError> {
    if parameter.len() < MM_VERSION_CONFIRMATION_MINIMUM_SIZE {
        return Err(ManagementDecodeError::TruncatedVersion {
            required: MM_VERSION_CONFIRMATION_MINIMUM_SIZE,
            available: parameter.len(),
        });
    }

    Ok(FirmwareVersion {
        version_lmac: u32::from_le_bytes(parameter[0..4].try_into().unwrap()),
        version_machw_1: u32::from_le_bytes(parameter[4..8].try_into().unwrap()),
        version_machw_2: u32::from_le_bytes(parameter[8..12].try_into().unwrap()),
        version_phy_1: u32::from_le_bytes(parameter[12..16].try_into().unwrap()),
        version_phy_2: u32::from_le_bytes(parameter[16..20].try_into().unwrap()),
        features: u32::from_le_bytes(parameter[20..24].try_into().unwrap()),
        max_sta_nb: u16::from_le_bytes(parameter[24..26].try_into().unwrap()),
        max_vif_nb: parameter[26],
    })
}

pub fn decode_mm_get_mac_address_confirmation(
    parameter: &[u8],
) -> Result<[u8; 6], ManagementDecodeError> {
    if parameter.len() < MM_MAC_ADDRESS_CONFIRMATION_SIZE {
        return Err(ManagementDecodeError::TruncatedMacAddress {
            required: MM_MAC_ADDRESS_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }

    Ok(parameter[..MM_MAC_ADDRESS_CONFIRMATION_SIZE]
        .try_into()
        .unwrap())
}

pub fn decode_mm_set_stack_start_confirmation(
    parameter: &[u8],
) -> Result<StackStartConfirmation, ManagementDecodeError> {
    if parameter.len() < MM_STACK_START_CONFIRMATION_SIZE {
        return Err(ManagementDecodeError::TruncatedStackStart {
            required: MM_STACK_START_CONFIRMATION_SIZE,
            available: parameter.len(),
        });
    }

    Ok(StackStartConfirmation {
        supports_5ghz: parameter[0] != 0,
        vendor_info: parameter[1],
    })
}

pub fn decode_mm_get_firmware_version_confirmation(
    parameter: &[u8],
) -> Result<FirmwareBuildVersion, ManagementDecodeError> {
    if parameter.is_empty() {
        return Err(ManagementDecodeError::TruncatedFirmwareVersionText {
            required: 1,
            available: 0,
        });
    }

    let length = usize::from(parameter[0]);
    if length > MM_FIRMWARE_VERSION_TEXT_CAPACITY {
        return Err(ManagementDecodeError::FirmwareVersionTextTooLong {
            length,
            capacity: MM_FIRMWARE_VERSION_TEXT_CAPACITY,
        });
    }
    let required = 1 + length;
    if parameter.len() < required {
        return Err(ManagementDecodeError::TruncatedFirmwareVersionText {
            required,
            available: parameter.len(),
        });
    }

    let mut bytes = [0_u8; MM_FIRMWARE_VERSION_TEXT_CAPACITY];
    bytes[..length].copy_from_slice(&parameter[1..required]);
    Ok(FirmwareBuildVersion {
        length: parameter[0],
        bytes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicManagementError<E> {
    Transaction(AicTransactionError<E>),
    Decode(ManagementDecodeError),
}

pub struct AicManagementClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicManagementClient<'a, I>
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

    pub fn reset(&mut self) -> Result<(), AicManagementError<<I as AicCommandIo>::Error>> {
        execute_config_command(
            self.io,
            self.product,
            MM_RESET_REQUEST,
            MM_RESET_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &[],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicManagementError::Transaction)?;
        Ok(())
    }

    pub fn read_version(
        &mut self,
    ) -> Result<FirmwareVersion, AicManagementError<<I as AicCommandIo>::Error>> {
        let response = execute_config_command(
            self.io,
            self.product,
            MM_VERSION_REQUEST,
            MM_VERSION_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &[],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicManagementError::Transaction)?;
        decode_mm_version_confirmation(response.parameter).map_err(AicManagementError::Decode)
    }

    pub fn read_mac_address(
        &mut self,
    ) -> Result<[u8; 6], AicManagementError<<I as AicCommandIo>::Error>> {
        let response = execute_config_command(
            self.io,
            self.product,
            MM_GET_MAC_ADDRESS_REQUEST,
            MM_GET_MAC_ADDRESS_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &1_u32.to_le_bytes(),
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicManagementError::Transaction)?;
        decode_mm_get_mac_address_confirmation(response.parameter)
            .map_err(AicManagementError::Decode)
    }

    pub fn start_d80_stack(
        &mut self,
    ) -> Result<StackStartConfirmation, AicManagementError<<I as AicCommandIo>::Error>> {
        let response = execute_config_command(
            self.io,
            self.product,
            MM_SET_STACK_START_REQUEST,
            MM_SET_STACK_START_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &[1, 0, 1 << 5, 0],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicManagementError::Transaction)?;
        decode_mm_set_stack_start_confirmation(response.parameter)
            .map_err(AicManagementError::Decode)
    }

    pub fn read_firmware_build_version(
        &mut self,
    ) -> Result<FirmwareBuildVersion, AicManagementError<<I as AicCommandIo>::Error>> {
        let response = execute_config_command(
            self.io,
            self.product,
            MM_GET_FIRMWARE_VERSION_REQUEST,
            MM_GET_FIRMWARE_VERSION_CONFIRM,
            MM_TASK_ID,
            DRIVER_TASK_ID,
            &[0],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicManagementError::Transaction)?;
        decode_mm_get_firmware_version_confirmation(response.parameter)
            .map_err(AicManagementError::Decode)
    }
}
