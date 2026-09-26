use crate::cis::{CisPointer, parse_function_extension, parse_manufacturer_id};
use crate::host::{EventSource, HostError, MonotonicClock, RegisterIo, SdhciHost};
use crate::protocol::{Command, ResponseKind, cmd52_argument, parse_r4};

const MAX_IO_FUNCTIONS: usize = 7;
const IO_OP_COND_ATTEMPTS: usize = 100;
const IO_OP_COND_RETRY_DELAY_MS: u32 = 10;
const RUN_CLOCK_HZ: u32 = 25_000_000;

const CCCR_REVISION: u32 = 0x00;
const CCCR_ABORT: u32 = 0x06;
const CCCR_BUS_INTERFACE: u32 = 0x07;
const CCCR_CAPABILITIES: u32 = 0x08;
const CCCR_SPEED: u32 = 0x13;
const BUS_WIDTH_MASK: u8 = 0x03;
const BUS_WIDTH_4: u8 = 0x02;

const FBR_BASE_STRIDE: u32 = 0x100;
const FBR_STANDARD_INTERFACE: u32 = 0x00;
const FBR_STANDARD_INTERFACE_EXTENDED: u32 = 0x01;
const FBR_CIS_POINTER: u32 = 0x09;

const CIS_NULL: u8 = 0x00;
const CIS_MANUFACTURER: u8 = 0x20;
const CIS_FUNCTION_EXTENSION: u8 = 0x22;
const CIS_END: u8 = 0xff;
const CIS_ADDRESS_LIMIT: u32 = 0x1ffff;
const CIS_SCAN_BYTE_LIMIT: usize = 4096;

const R5_STATUS_MASK: u32 = 0x0000_cb00;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionInfo {
    pub number: u8,
    pub class: u8,
    pub vendor: u16,
    pub device: u16,
    pub max_block_size: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardInfo {
    pub function_count: u8,
    pub ocr: u32,
    pub rca: u16,
    pub cccr_revision: u8,
    pub sdio_revision: u8,
    pub capabilities: u8,
    pub speed: u8,
    pub actual_clock_hz: u32,
    pub functions: [Option<FunctionInfo>; MAX_IO_FUNCTIONS],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumerationError {
    Host(HostError),
    InvalidCmd52Argument,
    R5Status(u32),
    NoIoFunctions,
    IoOpCondNotReady,
    UnsupportedCccrRevision(u8),
    InvalidCisAddress(u32),
    CisScanLimit,
    MissingManufacturer(u8),
    MissingFunctionExtension(u8),
}

impl From<HostError> for EnumerationError {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

pub trait SdioBus {
    fn command(&self, command: Command, argument: u32) -> Result<u32, HostError>;
    fn delay_us(&self, microseconds: u32);
    fn delay_ms(&self, milliseconds: u32);
    fn set_host_bus_width_4(&self);
    fn set_clock(&self, requested_hz: u32) -> Result<u32, HostError>;
}

impl<I, E, C> SdioBus for SdhciHost<I, E, C>
where
    I: RegisterIo,
    E: EventSource,
    C: MonotonicClock,
{
    fn command(&self, command: Command, argument: u32) -> Result<u32, HostError> {
        self.send_command(command, argument)
    }

    fn delay_ms(&self, milliseconds: u32) {
        SdhciHost::delay_ms(self, milliseconds);
    }

    fn delay_us(&self, microseconds: u32) {
        SdhciHost::delay_us(self, microseconds);
    }

    fn set_host_bus_width_4(&self) {
        self.set_bus_width_4();
    }

    fn set_clock(&self, requested_hz: u32) -> Result<u32, HostError> {
        SdhciHost::set_clock(self, requested_hz)
    }
}

pub fn enumerate(bus: &impl SdioBus) -> Result<CardInfo, EnumerationError> {
    let _ = reset_card(bus);
    bus.command(Command::new(0, ResponseKind::None), 0)?;

    let probe = parse_r4(bus.command(Command::new(5, ResponseKind::R4), 0)?);
    if probe.function_count == 0 {
        return Err(EnumerationError::NoIoFunctions);
    }

    let mut ready_response = None;
    for attempt in 0..IO_OP_COND_ATTEMPTS {
        let response = parse_r4(bus.command(Command::new(5, ResponseKind::R4), probe.ocr)?);
        if response.ready {
            ready_response = Some(response);
            break;
        }
        if attempt + 1 < IO_OP_COND_ATTEMPTS {
            bus.delay_ms(IO_OP_COND_RETRY_DELAY_MS);
        }
    }
    let ready = ready_response.ok_or(EnumerationError::IoOpCondNotReady)?;

    let rca_response = bus.command(Command::new(3, ResponseKind::R6), 0)?;
    let rca = (rca_response >> 16) as u16;
    bus.command(Command::new(7, ResponseKind::R1), (rca as u32) << 16)?;

    let revision = read_byte(bus, CCCR_REVISION)?;
    let cccr_revision = revision & 0x0f;
    if cccr_revision > 3 {
        return Err(EnumerationError::UnsupportedCccrRevision(cccr_revision));
    }
    let sdio_revision = revision >> 4;
    let capabilities = read_byte(bus, CCCR_CAPABILITIES)?;
    let speed = read_byte(bus, CCCR_SPEED)?;

    let common_cis = read_cis(bus, 0)?;
    let common_identity = common_cis
        .manufacturer
        .ok_or(EnumerationError::MissingManufacturer(0))?;

    let mut functions = [None; MAX_IO_FUNCTIONS];
    for function_number in 1..=probe.function_count {
        let fbr_base = function_number as u32 * FBR_BASE_STRIDE;
        let mut class = read_byte(bus, fbr_base + FBR_STANDARD_INTERFACE)? & 0x0f;
        if class == 0x0f {
            class = read_byte(bus, fbr_base + FBR_STANDARD_INTERFACE_EXTENDED)?;
        }

        let function_cis = read_cis(bus, function_number)?;
        let (vendor, device) = function_cis.manufacturer.unwrap_or(common_identity);
        let max_block_size = function_cis
            .max_block_size
            .ok_or(EnumerationError::MissingFunctionExtension(function_number))?;
        functions[function_number as usize - 1] = Some(FunctionInfo {
            number: function_number,
            class,
            vendor,
            device,
            max_block_size,
        });
    }

    let bus_interface = read_byte(bus, CCCR_BUS_INTERFACE)?;
    let four_bit = (bus_interface & !BUS_WIDTH_MASK) | BUS_WIDTH_4;
    write_byte(bus, CCCR_BUS_INTERFACE, four_bit)?;
    bus.set_host_bus_width_4();
    let actual_clock_hz = bus.set_clock(RUN_CLOCK_HZ)?;

    Ok(CardInfo {
        function_count: probe.function_count,
        ocr: ready.ocr,
        rca,
        cccr_revision,
        sdio_revision,
        capabilities,
        speed,
        actual_clock_hz,
        functions,
    })
}

fn reset_card(bus: &impl SdioBus) -> Result<(), EnumerationError> {
    let abort = read_byte(bus, CCCR_ABORT).unwrap_or(0x08) | 0x08;
    write_byte(bus, CCCR_ABORT, abort)
}

fn read_byte(bus: &impl SdioBus, address: u32) -> Result<u8, EnumerationError> {
    cmd52(bus, false, address, 0)
}

fn write_byte(bus: &impl SdioBus, address: u32, value: u8) -> Result<(), EnumerationError> {
    cmd52(bus, true, address, value).map(|_| ())
}

fn cmd52(bus: &impl SdioBus, write: bool, address: u32, value: u8) -> Result<u8, EnumerationError> {
    let argument = cmd52_argument(write, 0, false, address, value)
        .ok_or(EnumerationError::InvalidCmd52Argument)?;
    let response = bus.command(Command::new(52, ResponseKind::R5), argument)?;
    if response & R5_STATUS_MASK != 0 {
        return Err(EnumerationError::R5Status(response));
    }
    Ok(response as u8)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CisData {
    manufacturer: Option<(u16, u16)>,
    max_block_size: Option<u16>,
}

fn read_cis(bus: &impl SdioBus, function_number: u8) -> Result<CisData, EnumerationError> {
    let fbr_base = function_number as u32 * FBR_BASE_STRIDE;
    let pointer = CisPointer::from_bytes([
        read_byte(bus, fbr_base + FBR_CIS_POINTER)?,
        read_byte(bus, fbr_base + FBR_CIS_POINTER + 1)?,
        read_byte(bus, fbr_base + FBR_CIS_POINTER + 2)?,
    ])
    .address();
    if pointer == 0 || pointer > CIS_ADDRESS_LIMIT {
        return Err(EnumerationError::InvalidCisAddress(pointer));
    }

    let mut address = pointer;
    let mut scanned = 0usize;
    let mut result = CisData {
        manufacturer: None,
        max_block_size: None,
    };

    loop {
        if scanned >= CIS_SCAN_BYTE_LIMIT || address > CIS_ADDRESS_LIMIT {
            return Err(EnumerationError::CisScanLimit);
        }
        let code = read_byte(bus, address)?;
        address += 1;
        scanned += 1;
        if code == CIS_END {
            break;
        }
        if code == CIS_NULL {
            continue;
        }

        if scanned >= CIS_SCAN_BYTE_LIMIT || address > CIS_ADDRESS_LIMIT {
            return Err(EnumerationError::CisScanLimit);
        }
        let link = read_byte(bus, address)?;
        address += 1;
        scanned += 1;
        if link == CIS_END {
            break;
        }
        let link_len = link as usize;
        if scanned.saturating_add(link_len) > CIS_SCAN_BYTE_LIMIT
            || address.saturating_add(link_len as u32) > CIS_ADDRESS_LIMIT + 1
        {
            return Err(EnumerationError::CisScanLimit);
        }

        let mut data = [0_u8; 255];
        for byte in &mut data[..link_len] {
            *byte = read_byte(bus, address)?;
            address += 1;
            scanned += 1;
        }

        match code {
            CIS_MANUFACTURER => {
                result.manufacturer = parse_manufacturer_id(&data[..link_len]);
            }
            CIS_FUNCTION_EXTENSION if function_number != 0 => {
                result.max_block_size = parse_function_extension(&data[..link_len]);
            }
            _ => {}
        }
    }

    Ok(result)
}
