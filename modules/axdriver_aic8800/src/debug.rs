use crate::d80::D80FirmwareIo;
use crate::device::Aic8800Product;
use crate::firmware::{
    DBG_MEM_READ_CONFIRM, DBG_MEM_READ_REQUEST, DBG_MEM_WRITE_CONFIRM, DBG_MEM_WRITE_REQUEST,
    DBG_START_APP_CONFIRM, DBG_START_APP_REQUEST, DEBUG_TASK_ID, DRIVER_TASK_ID,
    DebugMemoryReadError, DebugMemoryWriteError, DebugStartAppError, FirmwareUploadError,
    decode_debug_memory_read_confirmation, decode_debug_start_app_confirmation,
    encode_debug_memory_read_parameters, encode_debug_memory_write_parameters,
    encode_debug_start_app_parameters, upload_firmware_image,
};
use crate::patch_table::AicPatchMemory;
use crate::response::AicResponseIo;
use crate::sdio::AicCommandIo;
use crate::transaction::{AicTransactionError, execute_config_command};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicDebugError<E> {
    ReadParameters(DebugMemoryReadError),
    ReadConfirmation(DebugMemoryReadError),
    WriteParameters(DebugMemoryWriteError),
    StartParameters(DebugStartAppError),
    StartConfirmation(DebugStartAppError),
    Transaction(AicTransactionError<E>),
    Upload(FirmwareUploadError<E>),
}

pub struct AicDebugClient<'a, I> {
    io: &'a mut I,
    product: Aic8800Product,
    parameter_storage: &'a mut [u8],
    transmit_storage: &'a mut [u8],
    receive_storage: &'a mut [u8],
    response_timeout_ms: u32,
}

impl<'a, I> AicDebugClient<'a, I>
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

    pub fn read_word(
        &mut self,
        address: u32,
    ) -> Result<u32, AicDebugError<<I as AicCommandIo>::Error>> {
        let parameter_length = encode_debug_memory_read_parameters(self.parameter_storage, address)
            .map_err(AicDebugError::ReadParameters)?;
        let response = execute_config_command(
            self.io,
            self.product,
            DBG_MEM_READ_REQUEST,
            DBG_MEM_READ_CONFIRM,
            DEBUG_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..parameter_length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicDebugError::Transaction)?;
        decode_debug_memory_read_confirmation(response.parameter, address)
            .map_err(AicDebugError::ReadConfirmation)
    }

    pub fn write_word(
        &mut self,
        address: u32,
        value: u32,
    ) -> Result<(), AicDebugError<<I as AicCommandIo>::Error>> {
        let parameter_length =
            encode_debug_memory_write_parameters(self.parameter_storage, address, value)
                .map_err(AicDebugError::WriteParameters)?;
        execute_config_command(
            self.io,
            self.product,
            DBG_MEM_WRITE_REQUEST,
            DBG_MEM_WRITE_CONFIRM,
            DEBUG_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..parameter_length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicDebugError::Transaction)?;
        Ok(())
    }

    pub fn upload_image(
        &mut self,
        address: u32,
        image: &[u8],
    ) -> Result<(), AicDebugError<<I as AicCommandIo>::Error>> {
        upload_firmware_image(
            self.io,
            self.product,
            address,
            image,
            self.parameter_storage,
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicDebugError::Upload)
    }

    pub fn start_app(
        &mut self,
        boot_address: u32,
        boot_type: u32,
    ) -> Result<u32, AicDebugError<<I as AicCommandIo>::Error>> {
        let parameter_length =
            encode_debug_start_app_parameters(self.parameter_storage, boot_address, boot_type)
                .map_err(AicDebugError::StartParameters)?;
        let response = execute_config_command(
            self.io,
            self.product,
            DBG_START_APP_REQUEST,
            DBG_START_APP_CONFIRM,
            DEBUG_TASK_ID,
            DRIVER_TASK_ID,
            &self.parameter_storage[..parameter_length],
            self.transmit_storage,
            self.receive_storage,
            self.response_timeout_ms,
        )
        .map_err(AicDebugError::Transaction)?;
        decode_debug_start_app_confirmation(response.parameter)
            .map_err(AicDebugError::StartConfirmation)
    }
}

impl<I> AicPatchMemory for AicDebugClient<'_, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    type Error = AicDebugError<<I as AicCommandIo>::Error>;

    fn write_word(&mut self, address: u32, value: u32) -> Result<(), Self::Error> {
        AicDebugClient::write_word(self, address, value)
    }

    fn delay_us(&mut self, microseconds: u32) {
        AicCommandIo::delay_us(self.io, microseconds);
    }
}

impl<I> D80FirmwareIo for AicDebugClient<'_, I>
where
    I: AicCommandIo + AicResponseIo<Error = <I as AicCommandIo>::Error>,
{
    fn read_word(&mut self, address: u32) -> Result<u32, Self::Error> {
        AicDebugClient::read_word(self, address)
    }

    fn upload_image(&mut self, address: u32, image: &[u8]) -> Result<(), Self::Error> {
        AicDebugClient::upload_image(self, address, image)
    }

    fn start_app(&mut self, address: u32, boot_type: u32) -> Result<u32, Self::Error> {
        AicDebugClient::start_app(self, address, boot_type)
    }
}
