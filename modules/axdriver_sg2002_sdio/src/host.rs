use crate::protocol::Command;
use crate::registers::clock_divider;

const ARGUMENT: usize = 0x08;
const BLOCK_SIZE: usize = 0x04;
const BLOCK_COUNT: usize = 0x06;
const TRANSFER_MODE: usize = 0x0c;
const COMMAND: usize = 0x0e;
const RESPONSE: usize = 0x10;
const BUFFER: usize = 0x20;
const PRESENT_STATE: usize = 0x24;
const HOST_CONTROL: usize = 0x28;
const POWER_CONTROL: usize = 0x29;
const CLOCK_CONTROL: usize = 0x2c;
const SOFTWARE_RESET: usize = 0x2f;
const INT_STATUS: usize = 0x30;
const INT_ENABLE: usize = 0x34;
const SIGNAL_ENABLE: usize = 0x38;

const VENDOR_MSHC_CONTROL: usize = 0x200;
const PHY_TX_RX_DELAY: usize = 0x240;
const PHY_CONFIG: usize = 0x24c;

const CMD_INHIBIT: u32 = 0x0000_0001;
const DATA_INHIBIT: u32 = 0x0000_0002;
const CLOCK_CARD_ENABLE: u16 = 0x0004;
const CLOCK_INTERNAL_STABLE: u16 = 0x0002;
const CLOCK_INTERNAL_ENABLE: u16 = 0x0001;
const RESET_ALL: u8 = 0x01;
const RESET_COMMAND: u8 = 0x02;
const RESET_DATA: u8 = 0x04;
const POWER_330_ON: u8 = 0x0f;
const CTRL_4BIT_BUS: u8 = 0x02;
const INT_RESPONSE: u32 = 0x0000_0001;
const INT_DATA_END: u32 = 0x0000_0002;
const INT_SPACE_AVAILABLE: u32 = 0x0000_0010;
const INT_DATA_AVAILABLE: u32 = 0x0000_0020;
const INT_ERROR: u32 = 0x0000_8000;
const INT_ERROR_DETAILS: u32 = 0x03ff_0000;
const COMMAND_INTERRUPT_MASK: u32 = INT_RESPONSE | INT_ERROR | INT_ERROR_DETAILS;
const DATA_INTERRUPT_MASK: u32 =
    INT_DATA_END | INT_SPACE_AVAILABLE | INT_DATA_AVAILABLE | INT_ERROR | INT_ERROR_DETAILS;
const TRANSFER_INTERRUPT_MASK: u32 = COMMAND_INTERRUPT_MASK | DATA_INTERRUPT_MASK;
const TRANSFER_BLOCK_COUNT_ENABLE: u16 = 0x02;
const TRANSFER_READ: u16 = 0x10;
const TRANSFER_MULTI_BLOCK: u16 = 0x20;
const CV181X_SDIO_VENDOR_BITS: u32 = (1 << 1) | (1 << 16);
const CV181X_DEFAULT_PHY_DELAY: u32 = 0x0100_0100;

const OPERATION_TIMEOUT_NS: u64 = 100_000_000;

pub trait RegisterIo {
    fn read_u8(&self, offset: usize) -> u8;
    fn read_u16(&self, offset: usize) -> u16;
    fn read_u32(&self, offset: usize) -> u32;
    fn write_u8(&self, offset: usize, value: u8);
    fn write_u16(&self, offset: usize, value: u16);
    fn write_u32(&self, offset: usize, value: u32);
}

pub trait EventSource {
    fn clear(&self);
    fn take(&self) -> u32;
}

pub trait MonotonicClock {
    fn now_ns(&self) -> u64;
    fn relax(&self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostError {
    InvalidClock,
    InvalidTransfer,
    Timeout,
    CommandInterrupt {
        command_index: u8,
        argument: u32,
        interrupt_status: u32,
        present_state: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataDirection {
    Read,
    Write,
}

pub struct SdhciHost<I, E, C> {
    io: I,
    events: E,
    clock: C,
    base_clock_hz: u32,
}

impl<I: RegisterIo, E: EventSource, C: MonotonicClock> SdhciHost<I, E, C> {
    pub const fn new(io: I, events: E, clock: C, base_clock_hz: u32) -> Self {
        Self {
            io,
            events,
            clock,
            base_clock_hz,
        }
    }

    #[cfg(test)]
    pub const fn io(&self) -> &I {
        &self.io
    }

    pub fn initialize(&self, requested_hz: u32) -> Result<u32, HostError> {
        self.io.write_u32(INT_ENABLE, 0);
        self.io.write_u32(SIGNAL_ENABLE, 0);
        self.io.write_u8(SOFTWARE_RESET, RESET_ALL);
        self.wait_until(|| self.io.read_u8(SOFTWARE_RESET) & RESET_ALL == 0)?;

        let vendor_control = self.io.read_u32(VENDOR_MSHC_CONTROL);
        self.io.write_u32(
            VENDOR_MSHC_CONTROL,
            vendor_control | CV181X_SDIO_VENDOR_BITS,
        );
        self.io
            .write_u32(PHY_CONFIG, self.io.read_u32(PHY_CONFIG) | 1);
        self.io.write_u32(PHY_TX_RX_DELAY, CV181X_DEFAULT_PHY_DELAY);

        self.io.write_u8(POWER_CONTROL, POWER_330_ON);
        let actual_hz = self.set_clock(requested_hz)?;

        self.io.write_u32(INT_STATUS, u32::MAX);
        self.io.write_u32(INT_ENABLE, COMMAND_INTERRUPT_MASK);
        self.io.write_u32(SIGNAL_ENABLE, COMMAND_INTERRUPT_MASK);
        Ok(actual_hz)
    }

    pub fn set_clock(&self, requested_hz: u32) -> Result<u32, HostError> {
        let divider =
            clock_divider(self.base_clock_hz, requested_hz).ok_or(HostError::InvalidClock)?;

        self.io.write_u16(CLOCK_CONTROL, 0);
        self.io
            .write_u16(CLOCK_CONTROL, divider.register_bits | CLOCK_INTERNAL_ENABLE);
        self.wait_until(|| self.io.read_u16(CLOCK_CONTROL) & CLOCK_INTERNAL_STABLE != 0)?;
        let stable_clock = self.io.read_u16(CLOCK_CONTROL);
        self.io
            .write_u16(CLOCK_CONTROL, stable_clock | CLOCK_CARD_ENABLE);
        Ok(divider.actual_hz)
    }

    pub fn set_bus_width_4(&self) {
        let control = self.io.read_u8(HOST_CONTROL);
        self.io.write_u8(HOST_CONTROL, control | CTRL_4BIT_BUS);
    }

    pub fn delay_ms(&self, milliseconds: u32) {
        self.delay_ns((milliseconds as u64).saturating_mul(1_000_000));
    }

    pub fn delay_us(&self, microseconds: u32) {
        self.delay_ns((microseconds as u64).saturating_mul(1_000));
    }

    fn delay_ns(&self, duration_ns: u64) {
        let deadline = self.clock.now_ns().saturating_add(duration_ns);
        while self.clock.now_ns() < deadline {
            self.clock.relax();
        }
    }

    pub fn send_command(&self, command: Command, argument: u32) -> Result<u32, HostError> {
        self.wait_until(|| self.io.read_u32(PRESENT_STATE) & CMD_INHIBIT == 0)?;
        self.events.clear();
        self.io.write_u32(INT_STATUS, u32::MAX);
        self.io.write_u32(ARGUMENT, argument);
        self.io.write_u16(COMMAND, command.register_word());

        let deadline = self.clock.now_ns().saturating_add(OPERATION_TIMEOUT_NS);
        loop {
            let status = self.events.take();
            if status & (INT_ERROR | INT_ERROR_DETAILS) != 0 {
                let error = HostError::CommandInterrupt {
                    command_index: command.index(),
                    argument,
                    interrupt_status: status,
                    present_state: self.io.read_u32(PRESENT_STATE),
                };
                let _ = self.reset_command_and_data_lines();
                return Err(error);
            }
            if status & INT_RESPONSE != 0 {
                return Ok(self.io.read_u32(RESPONSE));
            }
            if self.clock.now_ns() >= deadline {
                return Err(HostError::Timeout);
            }
            self.clock.relax();
        }
    }

    pub fn transfer_pio(
        &self,
        command: Command,
        argument: u32,
        direction: DataDirection,
        buffer: &mut [u8],
        block_size: u16,
    ) -> Result<u32, HostError> {
        let block_size_usize = block_size as usize;
        if block_size == 0
            || block_size > 0x0fff
            || buffer.is_empty()
            || buffer.len() % block_size_usize != 0
        {
            return Err(HostError::InvalidTransfer);
        }
        let block_count = buffer.len() / block_size_usize;
        if block_count == 0 || block_count > u16::MAX as usize {
            return Err(HostError::InvalidTransfer);
        }

        self.wait_until(|| self.io.read_u32(PRESENT_STATE) & (CMD_INHIBIT | DATA_INHIBIT) == 0)?;
        self.events.clear();
        self.io.write_u32(INT_STATUS, u32::MAX);
        self.io.write_u32(INT_ENABLE, TRANSFER_INTERRUPT_MASK);
        self.io.write_u32(SIGNAL_ENABLE, TRANSFER_INTERRUPT_MASK);

        let mut transfer_mode = TRANSFER_BLOCK_COUNT_ENABLE;
        if block_count > 1 {
            transfer_mode |= TRANSFER_MULTI_BLOCK;
        }
        if direction == DataDirection::Read {
            transfer_mode |= TRANSFER_READ;
        }
        self.io.write_u16(BLOCK_SIZE, block_size);
        self.io.write_u16(BLOCK_COUNT, block_count as u16);
        self.io.write_u16(TRANSFER_MODE, transfer_mode);
        self.io.write_u32(ARGUMENT, argument);
        self.io.write_u16(COMMAND, command.register_word());

        let result =
            self.wait_for_pio_transfer(command, argument, direction, buffer, block_size_usize);
        self.io.write_u32(INT_ENABLE, COMMAND_INTERRUPT_MASK);
        self.io.write_u32(SIGNAL_ENABLE, COMMAND_INTERRUPT_MASK);
        result
    }

    fn wait_for_pio_transfer(
        &self,
        command: Command,
        argument: u32,
        direction: DataDirection,
        buffer: &mut [u8],
        block_size: usize,
    ) -> Result<u32, HostError> {
        let deadline = self.clock.now_ns().saturating_add(OPERATION_TIMEOUT_NS);
        let ready_interrupt = match direction {
            DataDirection::Read => INT_DATA_AVAILABLE,
            DataDirection::Write => INT_SPACE_AVAILABLE,
        };
        let mut offset = 0usize;
        let mut response = None;
        let mut data_ended = false;

        loop {
            let status = self.events.take();
            if status & (INT_ERROR | INT_ERROR_DETAILS) != 0 {
                let error = HostError::CommandInterrupt {
                    command_index: command.index(),
                    argument,
                    interrupt_status: status,
                    present_state: self.io.read_u32(PRESENT_STATE),
                };
                let _ = self.reset_command_and_data_lines();
                return Err(error);
            }
            if status & INT_RESPONSE != 0 {
                response = Some(self.io.read_u32(RESPONSE));
            }
            if status & ready_interrupt != 0 && offset < buffer.len() {
                let end = offset + block_size;
                match direction {
                    DataDirection::Read => self.read_pio_block(&mut buffer[offset..end]),
                    DataDirection::Write => self.write_pio_block(&buffer[offset..end]),
                }
                offset = end;
            }
            if status & INT_DATA_END != 0 {
                data_ended = true;
            }
            if let Some(response) = response {
                if data_ended && offset == buffer.len() {
                    return Ok(response);
                }
            }
            if self.clock.now_ns() >= deadline {
                return Err(HostError::Timeout);
            }
            self.clock.relax();
        }
    }

    fn write_pio_block(&self, block: &[u8]) {
        for bytes in block.chunks(4) {
            let mut word = 0u32;
            for (shift, byte) in bytes.iter().copied().enumerate() {
                word |= (byte as u32) << (shift * 8);
            }
            self.io.write_u32(BUFFER, word);
        }
    }

    fn read_pio_block(&self, block: &mut [u8]) {
        for bytes in block.chunks_mut(4) {
            let word = self.io.read_u32(BUFFER);
            for (shift, byte) in bytes.iter_mut().enumerate() {
                *byte = (word >> (shift * 8)) as u8;
            }
        }
    }

    fn reset_command_and_data_lines(&self) -> Result<(), HostError> {
        self.io.write_u8(SOFTWARE_RESET, RESET_COMMAND);
        self.wait_until(|| self.io.read_u8(SOFTWARE_RESET) & RESET_COMMAND == 0)?;
        self.io.write_u8(SOFTWARE_RESET, RESET_DATA);
        self.wait_until(|| self.io.read_u8(SOFTWARE_RESET) & RESET_DATA == 0)
    }

    fn wait_until(&self, mut condition: impl FnMut() -> bool) -> Result<(), HostError> {
        let deadline = self.clock.now_ns().saturating_add(OPERATION_TIMEOUT_NS);
        while !condition() {
            if self.clock.now_ns() >= deadline {
                return Err(HostError::Timeout);
            }
            self.clock.relax();
        }
        Ok(())
    }
}
