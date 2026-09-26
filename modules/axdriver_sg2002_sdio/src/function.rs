use crate::enumeration::SdioBus;
use crate::host::{DataDirection, EventSource, HostError, MonotonicClock, RegisterIo, SdhciHost};
use crate::protocol::{Command, ResponseKind, TransferCount, cmd52_argument, cmd53_argument};

const CCCR_IO_ENABLE: u32 = 0x02;
const CCCR_IO_READY: u32 = 0x03;
const FBR_BASE_STRIDE: u32 = 0x100;
const FBR_BLOCK_SIZE: u32 = 0x10;
const R5_STATUS_MASK: u32 = 0x0000_cb00;
const MAX_SDIO_TRANSFER_COUNT: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionError {
    Host(HostError),
    InvalidFunction,
    InvalidBlockSize,
    InvalidTransferLength,
    InvalidCommandArgument,
    R5Status(u32),
    EnableTimeout,
}

impl From<HostError> for FunctionError {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

pub trait SdioTransferBus: SdioBus {
    fn transfer(
        &self,
        command: Command,
        argument: u32,
        direction: DataDirection,
        buffer: &mut [u8],
        block_size: u16,
    ) -> Result<u32, HostError>;
}

impl<I, E, C> SdioTransferBus for SdhciHost<I, E, C>
where
    I: RegisterIo,
    E: EventSource,
    C: MonotonicClock,
{
    fn transfer(
        &self,
        command: Command,
        argument: u32,
        direction: DataDirection,
        buffer: &mut [u8],
        block_size: u16,
    ) -> Result<u32, HostError> {
        self.transfer_pio(command, argument, direction, buffer, block_size)
    }
}

pub struct SdioFunction<'a, B> {
    bus: &'a B,
    number: u8,
    block_size: u16,
}

impl<'a, B: SdioTransferBus> SdioFunction<'a, B> {
    pub fn new(bus: &'a B, number: u8, block_size: u16) -> Result<Self, FunctionError> {
        if number == 0 || number > 7 {
            return Err(FunctionError::InvalidFunction);
        }
        if block_size == 0 || block_size > 0x0fff {
            return Err(FunctionError::InvalidBlockSize);
        }
        Ok(Self {
            bus,
            number,
            block_size,
        })
    }

    pub const fn number(&self) -> u8 {
        self.number
    }

    pub const fn block_size(&self) -> u16 {
        self.block_size
    }

    pub fn configure_block_size(&self) -> Result<(), FunctionError> {
        let base = self.number as u32 * FBR_BASE_STRIDE + FBR_BLOCK_SIZE;
        write_direct(self.bus, 0, base, self.block_size as u8)?;
        write_direct(self.bus, 0, base + 1, (self.block_size >> 8) as u8)
    }

    pub fn enable(&self, timeout_ms: u32) -> Result<(), FunctionError> {
        let function_bit = 1u8 << self.number;
        let enabled = read_direct(self.bus, 0, CCCR_IO_ENABLE)?;
        write_direct(self.bus, 0, CCCR_IO_ENABLE, enabled | function_bit)?;

        for elapsed_ms in 0..=timeout_ms {
            if read_direct(self.bus, 0, CCCR_IO_READY)? & function_bit != 0 {
                return Ok(());
            }
            if elapsed_ms == timeout_ms {
                return Err(FunctionError::EnableTimeout);
            }
            self.bus.delay_ms(1);
        }
        Err(FunctionError::EnableTimeout)
    }

    pub fn read_register(&self, address: u32) -> Result<u8, FunctionError> {
        read_direct(self.bus, self.number, address)
    }

    pub fn write_register(&self, address: u32, value: u8) -> Result<(), FunctionError> {
        write_direct(self.bus, self.number, address, value)
    }

    pub fn read_function_zero_register(&self, address: u32) -> Result<u8, FunctionError> {
        read_direct(self.bus, 0, address)
    }

    pub fn write_function_zero_register(
        &self,
        address: u32,
        value: u8,
    ) -> Result<(), FunctionError> {
        write_direct(self.bus, 0, address, value)
    }

    pub fn delay_ms(&self, milliseconds: u32) {
        self.bus.delay_ms(milliseconds);
    }

    pub fn delay_us(&self, microseconds: u32) {
        self.bus.delay_us(microseconds);
    }

    pub fn write_fifo(&self, address: u32, buffer: &mut [u8]) -> Result<(), FunctionError> {
        self.transfer_fifo(address, DataDirection::Write, buffer)
    }

    pub fn read_fifo(&self, address: u32, buffer: &mut [u8]) -> Result<(), FunctionError> {
        self.transfer_fifo(address, DataDirection::Read, buffer)
    }

    fn transfer_fifo(
        &self,
        address: u32,
        direction: DataDirection,
        buffer: &mut [u8],
    ) -> Result<(), FunctionError> {
        if buffer.is_empty() {
            return Err(FunctionError::InvalidTransferLength);
        }

        let (count, transfer_block_size) = if buffer.len() % self.block_size as usize == 0 {
            let blocks = buffer.len() / self.block_size as usize;
            if blocks == 0 || blocks > MAX_SDIO_TRANSFER_COUNT {
                return Err(FunctionError::InvalidTransferLength);
            }
            (TransferCount::Blocks(blocks as u16), self.block_size)
        } else {
            if buffer.len() > MAX_SDIO_TRANSFER_COUNT {
                return Err(FunctionError::InvalidTransferLength);
            }
            (
                TransferCount::Bytes(buffer.len() as u16),
                buffer.len() as u16,
            )
        };

        let write = direction == DataDirection::Write;
        let argument = cmd53_argument(write, self.number, false, address, count)
            .ok_or(FunctionError::InvalidCommandArgument)?;
        let response = self.bus.transfer(
            Command::new(53, ResponseKind::R5).with_data(),
            argument,
            direction,
            buffer,
            transfer_block_size,
        )?;
        if response & R5_STATUS_MASK != 0 {
            return Err(FunctionError::R5Status(response));
        }
        Ok(())
    }
}

fn read_direct(bus: &impl SdioBus, function: u8, address: u32) -> Result<u8, FunctionError> {
    direct(bus, false, function, address, 0).map(|response| response as u8)
}

fn write_direct(
    bus: &impl SdioBus,
    function: u8,
    address: u32,
    value: u8,
) -> Result<(), FunctionError> {
    direct(bus, true, function, address, value).map(|_| ())
}

fn direct(
    bus: &impl SdioBus,
    write: bool,
    function: u8,
    address: u32,
    value: u8,
) -> Result<u32, FunctionError> {
    let argument = cmd52_argument(write, function, false, address, value)
        .ok_or(FunctionError::InvalidCommandArgument)?;
    let response = bus.command(Command::new(52, ResponseKind::R5), argument)?;
    if response & R5_STATUS_MASK != 0 {
        return Err(FunctionError::R5Status(response));
    }
    Ok(response)
}
