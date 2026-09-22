use axdriver_sg2002_sdio::function::{FunctionError, SdioFunction, SdioTransferBus};

use crate::response::AicResponseIo;
use crate::sdio::{AicCommandIo, AicSdioIo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sg2002AicSdioError {
    Function(FunctionError),
    UnsupportedFunction(u8),
    BlockSizeMismatch { requested: u16, configured: u16 },
}

impl From<FunctionError> for Sg2002AicSdioError {
    fn from(value: FunctionError) -> Self {
        Self::Function(value)
    }
}

pub struct Sg2002SdioIo<'a, B> {
    function1: SdioFunction<'a, B>,
    function2: SdioFunction<'a, B>,
    enable_timeout_ms: u32,
}

impl<'a, B: SdioTransferBus> Sg2002SdioIo<'a, B> {
    pub fn new(bus: &'a B, enable_timeout_ms: u32) -> Result<Self, Sg2002AicSdioError> {
        Ok(Self {
            function1: SdioFunction::new(bus, 1, 512)?,
            function2: SdioFunction::new(bus, 2, 512)?,
            enable_timeout_ms,
        })
    }

    pub const fn function1(&self) -> &SdioFunction<'a, B> {
        &self.function1
    }

    fn function(&self, number: u8) -> Result<&SdioFunction<'a, B>, Sg2002AicSdioError> {
        match number {
            1 => Ok(&self.function1),
            2 => Ok(&self.function2),
            value => Err(Sg2002AicSdioError::UnsupportedFunction(value)),
        }
    }
}

impl<B: SdioTransferBus> AicSdioIo for Sg2002SdioIo<'_, B> {
    type Error = Sg2002AicSdioError;

    fn set_block_size(&mut self, function: u8, size: u16) -> Result<(), Self::Error> {
        let selected = self.function(function)?;
        if selected.block_size() != size {
            return Err(Sg2002AicSdioError::BlockSizeMismatch {
                requested: size,
                configured: selected.block_size(),
            });
        }
        selected.configure_block_size().map_err(Into::into)
    }

    fn enable_function(&mut self, function: u8) -> Result<(), Self::Error> {
        self.function(function)?
            .enable(self.enable_timeout_ms)
            .map_err(Into::into)
    }

    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error> {
        match function {
            0 => self
                .function1
                .write_function_zero_register(address, value)
                .map_err(Into::into),
            1 | 2 => self
                .function(function)?
                .write_register(address, value)
                .map_err(Into::into),
            value => Err(Sg2002AicSdioError::UnsupportedFunction(value)),
        }
    }

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        match function {
            0 => self
                .function1
                .read_function_zero_register(address)
                .map_err(Into::into),
            1 | 2 => self
                .function(function)?
                .read_register(address)
                .map_err(Into::into),
            value => Err(Sg2002AicSdioError::UnsupportedFunction(value)),
        }
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.function1.delay_ms(milliseconds);
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.function1.delay_us(microseconds);
    }
}

impl<B: SdioTransferBus> AicCommandIo for Sg2002SdioIo<'_, B> {
    type Error = Sg2002AicSdioError;

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        AicSdioIo::read_register(self, function, address)
    }

    fn write_fifo(
        &mut self,
        function: u8,
        address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.function(function)?
            .write_fifo(address, data)
            .map_err(Into::into)
    }

    fn delay_us(&mut self, microseconds: u32) {
        self.function1.delay_us(microseconds);
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.function1.delay_ms(milliseconds);
    }
}

impl<B: SdioTransferBus> AicResponseIo for Sg2002SdioIo<'_, B> {
    type Error = Sg2002AicSdioError;

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error> {
        AicSdioIo::read_register(self, function, address)
    }

    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error> {
        AicSdioIo::write_register(self, function, address, value)
    }

    fn read_fifo(
        &mut self,
        function: u8,
        address: u32,
        output: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.function(function)?
            .read_fifo(address, output)
            .map_err(Into::into)
    }

    fn delay_ms(&mut self, milliseconds: u32) {
        self.function1.delay_ms(milliseconds);
    }
}
