use crate::device::{Aic8800Product, SDIO_BLOCK_SIZE};

const FUNCTION_0: u8 = 0;
const FUNCTION_1: u8 = 1;
const FUNCTION_2: u8 = 2;
const FLOW_CONTROL_RETRY_COUNT: u32 = 50;
const FLOW_CONTROL_BUFFER_SIZE: usize = 1536;
const COMMAND_FLOW_RETRIES: u8 = 10;

pub trait AicSdioIo {
    type Error;

    fn set_block_size(&mut self, function: u8, size: u16) -> Result<(), Self::Error>;
    fn enable_function(&mut self, function: u8) -> Result<(), Self::Error>;
    fn write_register(&mut self, function: u8, address: u32, value: u8) -> Result<(), Self::Error>;
    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error>;
    fn delay_us(&mut self, microseconds: u32);
    fn delay_ms(&mut self, milliseconds: u32);
}

pub trait AicCommandIo {
    type Error;

    fn read_register(&mut self, function: u8, address: u32) -> Result<u8, Self::Error>;
    fn write_fifo(
        &mut self,
        function: u8,
        address: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error>;
    fn delay_us(&mut self, microseconds: u32);
    fn delay_ms(&mut self, milliseconds: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicCommandError<E> {
    Io(E),
    InvalidTransferLength {
        length: usize,
    },
    InsufficientFlowControl {
        buffer_count: u8,
        transfer_length: usize,
    },
}

pub fn poll_command_flow_control<I: AicCommandIo>(
    io: &mut I,
    product: Aic8800Product,
) -> Result<u8, AicCommandError<I::Error>> {
    let mut count = 0;
    loop {
        let mut value = io
            .read_register(FUNCTION_1, product.registers().flow_control as u32)
            .map_err(AicCommandError::Io)?;
        if matches!(product, Aic8800Product::Aic8801 | Aic8800Product::Aic8800Dc) {
            value &= 0x7f;
        }
        if value != 0 {
            return Ok(value);
        }
        if count >= FLOW_CONTROL_RETRY_COUNT {
            return Ok(0);
        }

        count += 1;
        if count < 30 {
            io.delay_us(200);
        } else if count < 40 {
            io.delay_ms(1);
        } else {
            io.delay_ms(10);
        }
    }
}

pub fn send_command_transfer<I: AicCommandIo>(
    io: &mut I,
    product: Aic8800Product,
    transfer: &mut [u8],
) -> Result<(), AicCommandError<I::Error>> {
    if transfer.is_empty() || transfer.len() % SDIO_BLOCK_SIZE != 0 {
        return Err(AicCommandError::InvalidTransferLength {
            length: transfer.len(),
        });
    }

    match product {
        Aic8800Product::Aic8800Dc => io
            .write_fifo(FUNCTION_2, product.registers().write_fifo as u32, transfer)
            .map_err(AicCommandError::Io),
        Aic8800Product::Aic8801 | Aic8800Product::Aic8800D80 | Aic8800Product::Aic8800D80X2 => {
            let mut buffer_count = poll_command_flow_control(io, product)?;
            let mut retry = 0;
            while (buffer_count == 0
                || transfer.len() > buffer_count as usize * FLOW_CONTROL_BUFFER_SIZE)
                && retry < COMMAND_FLOW_RETRIES
            {
                retry += 1;
                buffer_count = poll_command_flow_control(io, product)?;
            }

            if buffer_count == 0
                || transfer.len() >= buffer_count as usize * FLOW_CONTROL_BUFFER_SIZE
            {
                return Err(AicCommandError::InsufficientFlowControl {
                    buffer_count,
                    transfer_length: transfer.len(),
                });
            }

            io.write_fifo(FUNCTION_1, product.registers().write_fifo as u32, transfer)
                .map_err(AicCommandError::Io)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicSdioInitError<E> {
    Io(E),
    MissingClassicRegister,
}

pub fn initialize_sdio_functions<I: AicSdioIo>(
    io: &mut I,
    product: Aic8800Product,
) -> Result<(), AicSdioInitError<I::Error>> {
    set_block_size(io, FUNCTION_1)?;
    enable_function(io, FUNCTION_1)?;

    match product {
        Aic8800Product::Aic8801 | Aic8800Product::Aic8800Dc => initialize_classic(io, product)?,
        Aic8800Product::Aic8800D80 | Aic8800Product::Aic8800D80X2 => initialize_v3(io, product)?,
    }
    initialize_interrupts(io, product)
}

fn initialize_classic<I: AicSdioIo>(
    io: &mut I,
    product: Aic8800Product,
) -> Result<(), AicSdioInitError<I::Error>> {
    io.delay_us(100);
    if product == Aic8800Product::Aic8800Dc {
        set_block_size(io, FUNCTION_2)?;
        enable_function(io, FUNCTION_2)?;
    }

    let registers = product.registers();
    let register_block = registers
        .register_block
        .ok_or(AicSdioInitError::MissingClassicRegister)?;
    if product == Aic8800Product::Aic8800Dc {
        write_register(io, FUNCTION_2, register_block as u32, 0x01)?;
        write_register(io, FUNCTION_2, registers.byte_mode_enable as u32, 0x01)?;
    }
    write_register(io, FUNCTION_1, register_block as u32, 0x01)?;
    write_register(io, FUNCTION_1, registers.byte_mode_enable as u32, 0x01)?;
    Ok(())
}

fn initialize_v3<I: AicSdioIo>(
    io: &mut I,
    product: Aic8800Product,
) -> Result<(), AicSdioInitError<I::Error>> {
    let registers = product.registers();
    write_register(io, FUNCTION_0, 0xf2, 0x7f)?;
    write_register(io, FUNCTION_1, registers.byte_mode_enable as u32, 0x01)?;
    Ok(())
}

fn initialize_interrupts<I: AicSdioIo>(
    io: &mut I,
    product: Aic8800Product,
) -> Result<(), AicSdioInitError<I::Error>> {
    let interrupt_register = product.registers().interrupt as u32;
    match product {
        Aic8800Product::Aic8801 => write_register(io, FUNCTION_1, interrupt_register, 0x07),
        Aic8800Product::Aic8800Dc => {
            write_register(io, FUNCTION_1, interrupt_register, 0x07)?;
            write_register(io, FUNCTION_2, interrupt_register, 0x07)
        }
        Aic8800Product::Aic8800D80 | Aic8800Product::Aic8800D80X2 => {
            write_register(io, FUNCTION_0, 0x04, 0x07)?;
            write_register(io, FUNCTION_1, interrupt_register, 0x07)
        }
    }
}

fn set_block_size<I: AicSdioIo>(
    io: &mut I,
    function: u8,
) -> Result<(), AicSdioInitError<I::Error>> {
    io.set_block_size(function, SDIO_BLOCK_SIZE as u16)
        .map_err(AicSdioInitError::Io)
}

fn enable_function<I: AicSdioIo>(
    io: &mut I,
    function: u8,
) -> Result<(), AicSdioInitError<I::Error>> {
    io.enable_function(function).map_err(AicSdioInitError::Io)
}

fn write_register<I: AicSdioIo>(
    io: &mut I,
    function: u8,
    address: u32,
    value: u8,
) -> Result<(), AicSdioInitError<I::Error>> {
    io.write_register(function, address, value)
        .map_err(AicSdioInitError::Io)
}
