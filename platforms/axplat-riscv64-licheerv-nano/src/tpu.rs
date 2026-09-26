//! Read-only SG2002 TPU MMIO access used by the first hardware probe.

/// TDMA MMIO base from the SG2002/CV181x device tree.
pub const TDMA_BASE: usize = 0x0c10_0000;

/// TIU MMIO base from the SG2002/CV181x device tree.
pub const TIU_BASE: usize = 0x0c10_1000;

/// `TDMA_STATUS` offset from the CV181x TPU driver.
pub const TDMA_STATUS_OFFSET: usize = 0x00ec;

/// Physical address of `TDMA_STATUS`.
pub const TDMA_STATUS_ADDRESS: usize = TDMA_BASE + TDMA_STATUS_OFFSET;

/// Clock-controller base from the SG2002/CV181x device tree.
pub const CLOCK_CONTROLLER_BASE: usize = 0x0300_2000;

/// `REG_CLK_EN_0` contains the TPU and TPU fabric clock gates.
pub const CLOCK_ENABLE_0_ADDRESS: usize = CLOCK_CONTROLLER_BASE;

/// Reset-controller base from the SG2002/CV181x device tree.
pub const RESET_CONTROLLER_BASE: usize = 0x0300_3000;

/// Reset bank 0 contains `RST_TDMA`, `RST_TPU`, and `RST_TPUSYS`.
pub const RESET_BANK_0_ADDRESS: usize = RESET_CONTROLLER_BASE;

/// `clk_tpu` gate bit in `REG_CLK_EN_0`.
pub const TPU_CLOCK_ENABLE_BIT: u32 = 4;

/// `clk_tpu_fab` gate bit in `REG_CLK_EN_0`.
pub const TPU_FAB_CLOCK_ENABLE_BIT: u32 = 5;

/// `RST_TDMA` bit in reset bank 0.
pub const TDMA_RESET_BIT: u32 = 7;

/// `RST_TPU` bit in reset bank 0.
pub const TPU_RESET_BIT: u32 = 8;

/// `RST_TPUSYS` bit in reset bank 0.
pub const TPUSYS_RESET_BIT: u32 = 9;

/// Main magic value required by the SG2002 TPU dmabuf driver.
pub const DMABUF_MAGIC_MAIN: u16 = 0xb5b5;

const TDMA_CTRL_ADDRESS: usize = TDMA_BASE;
const TDMA_DESCRIPTOR_BASE_ADDRESS: usize = TDMA_BASE + 0x04;
const TDMA_INTERRUPT_MASK_ADDRESS: usize = TDMA_BASE + 0x08;
const TDMA_SYNC_STATUS_ADDRESS: usize = TDMA_BASE + 0x0c;
const TDMA_ARRAY_BASE_LOW_ADDRESS: usize = TDMA_BASE + 0x70;
const TDMA_ARRAY_BASE_HIGH_ADDRESS: usize = TDMA_BASE + 0x90;
const TDMA_DEBUG_MODE_ADDRESS: usize = TDMA_BASE + 0xa0;
const TDMA_DCM_DISABLE_ADDRESS: usize = TDMA_BASE + 0xa4;
const TIU_CONTROL_ADDRESS: usize = TIU_BASE + 0x100;

const TDMA_MASK_INIT: u32 = 0x20;
const TPU_EXECUTION_TIMEOUT_TICKS: u64 = crate::config::devices::TIMER_FREQUENCY as u64 * 60;

/// SG2002 TPU dmabuf header, matching `struct dma_hdr_t` in the fixed SDK.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DmaBufferHeader {
    pub magic_main: u16,
    pub magic_sub: u16,
    pub buffer_size: u32,
    pub cpu_descriptor_count: u32,
    pub tiu_descriptor_count: u32,
    pub tdma_descriptor_count: u32,
    pub tpu_clock_rate: u32,
    pub pmu_buffer_size: u32,
    pub pmu_buffer_offset: u32,
    pub array_bases: [[u32; 2]; 8],
    pub reserved: [u32; 8],
}

/// CPU synchronization descriptor matching `struct cvi_cpu_sync_desc_t`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpuSyncDescriptor {
    pub operation_type: u32,
    pub tiu_descriptor_count: u32,
    pub tdma_descriptor_count: u32,
    pub tiu_descriptor_offset: u32,
    pub tdma_descriptor_offset: u32,
    pub reserved: [u32; 2],
    pub text: [u8; (56 - 7) * core::mem::size_of::<u32>()],
}

const _: () = assert!(core::mem::size_of::<DmaBufferHeader>() == 128);
const _: () = assert!(core::mem::size_of::<CpuSyncDescriptor>() == 224);

/// Errors detected before a TPU dmabuf can be submitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaBufferError {
    AddressNotPageAligned,
    BufferTooSmall,
    InvalidMagic,
    DeclaredSizeOutOfRange,
    DescriptorTableOutOfRange,
    PmuRangeOutOfRange,
    RelocatedAddressOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TpuExecutionError {
    InvalidDmaBuffer(DmaBufferError),
    TdmaTimeout,
    TdmaInterruptError(u32),
    TdmaCommandIdIncomplete { completed: u32, expected: u32 },
    TiuTimeout,
}

/// Validates the fixed header and bounds needed by the SG2002 driver.
pub fn validate_dma_buffer(
    physical_address: usize,
    bytes: &[u8],
) -> Result<DmaBufferHeader, DmaBufferError> {
    if physical_address & 0xfff != 0 {
        return Err(DmaBufferError::AddressNotPageAligned);
    }
    if bytes.len() < core::mem::size_of::<DmaBufferHeader>() {
        return Err(DmaBufferError::BufferTooSmall);
    }

    // SAFETY: the byte slice contains the complete header. `bytes` may be a
    // subslice whose pointer is not naturally aligned, so use an unaligned copy.
    let header = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const DmaBufferHeader) };
    if header.magic_main != DMABUF_MAGIC_MAIN {
        return Err(DmaBufferError::InvalidMagic);
    }

    let declared_size = header.buffer_size as usize;
    if declared_size < core::mem::size_of::<DmaBufferHeader>() || declared_size > bytes.len() {
        return Err(DmaBufferError::DeclaredSizeOutOfRange);
    }
    let descriptor_bytes = (header.cpu_descriptor_count as usize)
        .checked_mul(core::mem::size_of::<CpuSyncDescriptor>())
        .and_then(|size| size.checked_add(core::mem::size_of::<DmaBufferHeader>()))
        .ok_or(DmaBufferError::DescriptorTableOutOfRange)?;
    if descriptor_bytes > declared_size {
        return Err(DmaBufferError::DescriptorTableOutOfRange);
    }
    if header.pmu_buffer_size != 0 {
        let pmu_end = (header.pmu_buffer_offset as usize)
            .checked_add(header.pmu_buffer_size as usize)
            .ok_or(DmaBufferError::PmuRangeOutOfRange)?;
        if pmu_end > declared_size {
            return Err(DmaBufferError::PmuRangeOutOfRange);
        }
    }
    Ok(header)
}

/// Applies the physical-address relocation performed by the fixed CV181x
/// runtime after an offline CMDBUF-to-DMABUF conversion.
pub fn relocate_dma_buffer(
    physical_address: usize,
    bytes: &mut [u8],
) -> Result<(), DmaBufferError> {
    let descriptor_count = validate_dma_buffer(physical_address, bytes)?.cpu_descriptor_count;
    let descriptor_base = core::mem::size_of::<DmaBufferHeader>();

    for index in 0..descriptor_count as usize {
        let offset = descriptor_base + index * core::mem::size_of::<CpuSyncDescriptor>();
        // SAFETY: validation proved that the complete descriptor is contained
        // in `bytes`; use unaligned copies because the public slice need not be aligned.
        let descriptor_pointer =
            unsafe { bytes.as_mut_ptr().add(offset) as *mut CpuSyncDescriptor };
        let mut descriptor = unsafe { core::ptr::read_unaligned(descriptor_pointer) };

        if descriptor.tiu_descriptor_count & 0xffff != 0 {
            let original = descriptor.tiu_descriptor_offset;
            let relocated = physical_address
                .checked_add(original as usize)
                .ok_or(DmaBufferError::RelocatedAddressOutOfRange)?
                >> 8;
            descriptor.reserved[0] = original;
            descriptor.tiu_descriptor_offset =
                u32::try_from(relocated).map_err(|_| DmaBufferError::RelocatedAddressOutOfRange)?;
        }

        if descriptor.tdma_descriptor_count & 0xffff != 0 {
            let original = descriptor.tdma_descriptor_offset;
            let relocated = physical_address
                .checked_add(original as usize)
                .ok_or(DmaBufferError::RelocatedAddressOutOfRange)?
                >> 6;
            descriptor.reserved[1] = original;
            descriptor.tdma_descriptor_offset =
                u32::try_from(relocated).map_err(|_| DmaBufferError::RelocatedAddressOutOfRange)?;
        }
        // SAFETY: `descriptor_pointer` refers to the same validated descriptor range.
        unsafe { core::ptr::write_unaligned(descriptor_pointer, descriptor) };
    }
    Ok(())
}

/// Sets array base 0 (neuron memory) and base 1 (weight memory), matching
/// `cvi181x_arraybase_set` in the fixed runtime.
pub fn set_neuron_and_weight_bases(
    bytes: &mut [u8],
    neuron_physical_address: usize,
    weight_physical_address: usize,
) -> Result<(), DmaBufferError> {
    if bytes.len() < core::mem::size_of::<DmaBufferHeader>() {
        return Err(DmaBufferError::BufferTooSmall);
    }
    let header_pointer = bytes.as_mut_ptr() as *mut DmaBufferHeader;
    // SAFETY: the complete C-compatible header is present; use an unaligned copy.
    let mut header = unsafe { core::ptr::read_unaligned(header_pointer) };
    if header.magic_main != DMABUF_MAGIC_MAIN {
        return Err(DmaBufferError::InvalidMagic);
    }
    let neuron = u32::try_from(neuron_physical_address)
        .map_err(|_| DmaBufferError::RelocatedAddressOutOfRange)?;
    let weight = u32::try_from(weight_physical_address)
        .map_err(|_| DmaBufferError::RelocatedAddressOutOfRange)?;
    header.array_bases[0] = [neuron, 0];
    header.array_bases[1] = [weight, 0];
    // SAFETY: `header_pointer` still refers to the validated header range.
    unsafe { core::ptr::write_unaligned(header_pointer, header) };
    Ok(())
}

/// Stores all eight physical array bases in the DMABUF header.
pub fn set_array_bases(
    bytes: &mut [u8],
    physical_addresses: [usize; 8],
) -> Result<(), DmaBufferError> {
    if bytes.len() < core::mem::size_of::<DmaBufferHeader>() {
        return Err(DmaBufferError::BufferTooSmall);
    }
    let header_pointer = bytes.as_mut_ptr() as *mut DmaBufferHeader;
    // SAFETY: the complete C-compatible header is present; use an unaligned copy.
    let mut header = unsafe { core::ptr::read_unaligned(header_pointer) };
    if header.magic_main != DMABUF_MAGIC_MAIN {
        return Err(DmaBufferError::InvalidMagic);
    }
    for (index, address) in physical_addresses.into_iter().enumerate() {
        let low = u32::try_from(address).map_err(|_| DmaBufferError::RelocatedAddressOutOfRange)?;
        header.array_bases[index] = [low, 0];
    }
    // SAFETY: `header_pointer` still refers to the validated header range.
    unsafe { core::ptr::write_unaligned(header_pointer, header) };
    Ok(())
}

fn read_mmio_u32(address: usize) -> u32 {
    // SAFETY: callers use fixed 32-bit SG2002 TPU register addresses.
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

fn timeout_reached(start: u64) -> bool {
    (riscv::register::time::read() as u64).wrapping_sub(start) >= TPU_EXECUTION_TIMEOUT_TICKS
}

fn resynchronize_command_ids() {
    let tiu_control_3 = TIU_CONTROL_ADDRESS + 0x0c;
    let value = read_mmio_u32(tiu_control_3);
    write_mmio_u32(tiu_control_3, value | 1);
    write_mmio_u32(tiu_control_3, value & !1);

    let value = read_mmio_u32(TIU_CONTROL_ADDRESS);
    write_mmio_u32(TIU_CONTROL_ADDRESS, value & !((1 << 0) | (1 << 30)));
    let value = read_mmio_u32(TIU_CONTROL_ADDRESS);
    write_mmio_u32(TIU_CONTROL_ADDRESS, value | (1 << 1));

    write_mmio_u32(TDMA_CTRL_ADDRESS, 1 << 2);
    write_mmio_u32(TDMA_CTRL_ADDRESS, 0);
    write_mmio_u32(TDMA_INTERRUPT_MASK_ADDRESS, 0xffff_0000);
}

fn start_tiu(descriptor_offset: u32) {
    let descriptor_address = (descriptor_offset as u64) << 8;
    write_mmio_u32(TIU_CONTROL_ADDRESS + 0x04, descriptor_address as u32);
    let value = read_mmio_u32(TIU_CONTROL_ADDRESS + 0x08);
    write_mmio_u32(
        TIU_CONTROL_ADDRESS + 0x08,
        (value & 0xffff_ff00) | ((descriptor_address >> 32) as u32 & 0xff),
    );
    let value = read_mmio_u32(TIU_CONTROL_ADDRESS + 0x0c);
    write_mmio_u32(TIU_CONTROL_ADDRESS + 0x0c, value | (1 << 11));

    let value = read_mmio_u32(TIU_CONTROL_ADDRESS) & !0x3fc0_0000;
    write_mmio_u32(TIU_CONTROL_ADDRESS, value | (3 << 22));
    let value = read_mmio_u32(TIU_CONTROL_ADDRESS);
    write_mmio_u32(
        TIU_CONTROL_ADDRESS,
        value | (1 << 30) | (1 << 31) | (1 << 0),
    );
}

fn start_tdma(descriptor_offset: u32, descriptor_count: u32) {
    write_mmio_u32(TDMA_DESCRIPTOR_BASE_ADDRESS, descriptor_offset);
    write_mmio_u32(TDMA_DEBUG_MODE_ADDRESS, 0);
    write_mmio_u32(TDMA_DCM_DISABLE_ADDRESS, 0);
    write_mmio_u32(TDMA_INTERRUPT_MASK_ADDRESS, TDMA_MASK_INIT);
    write_mmio_u32(
        TDMA_CTRL_ADDRESS,
        (1 << 0)
            | (1 << 1)
            | (descriptor_count << 16)
            | (3 << 8)
            | (1 << 5)
            | (1 << 13)
            | (1 << 10),
    );
}

fn wait_tdma(expected_command_id: u32) -> Result<(), TpuExecutionError> {
    let start = riscv::register::time::read() as u64;
    loop {
        let interrupt_register = read_mmio_u32(TDMA_INTERRUPT_MASK_ADDRESS);
        let interrupt_status = (interrupt_register >> 16) & !TDMA_MASK_INIT;
        if interrupt_status != 0 {
            if interrupt_status != 0x1 && interrupt_status != 0x8000 {
                return Err(TpuExecutionError::TdmaInterruptError(interrupt_register));
            }
            let completed = read_mmio_u32(TDMA_SYNC_STATUS_ADDRESS) >> 16;
            write_mmio_u32(TDMA_INTERRUPT_MASK_ADDRESS, 0xffff_0000);
            if completed < expected_command_id {
                return Err(TpuExecutionError::TdmaCommandIdIncomplete {
                    completed,
                    expected: expected_command_id,
                });
            }
            return Ok(());
        }
        if timeout_reached(start) {
            return Err(TpuExecutionError::TdmaTimeout);
        }
        core::hint::spin_loop();
    }
}

fn wait_tiu(expected_command_id: u32) -> Result<(), TpuExecutionError> {
    let start = riscv::register::time::read() as u64;
    loop {
        let value = read_mmio_u32(TIU_CONTROL_ADDRESS);
        if ((value >> 6) & 0xffff) >= expected_command_id && value & (1 << 1) != 0 {
            write_mmio_u32(TIU_CONTROL_ADDRESS, value | (1 << 1));
            return Ok(());
        }
        if timeout_reached(start) {
            return Err(TpuExecutionError::TiuTimeout);
        }
        core::hint::spin_loop();
    }
}

/// Submits a relocated CV181x DMABUF and polls the same completion fields used
/// by the fixed Linux driver. PMU execution is intentionally not enabled.
pub fn execute_dma_buffer(physical_address: usize, bytes: &[u8]) -> Result<(), TpuExecutionError> {
    let header = validate_dma_buffer(physical_address, bytes)
        .map_err(TpuExecutionError::InvalidDmaBuffer)?;

    for index in 0..8 {
        write_mmio_u32(
            TDMA_ARRAY_BASE_LOW_ADDRESS + index * 4,
            header.array_bases[index][0],
        );
    }
    write_mmio_u32(TDMA_ARRAY_BASE_HIGH_ADDRESS, 0);
    write_mmio_u32(TDMA_ARRAY_BASE_HIGH_ADDRESS + 4, 0);

    let descriptor_base = core::mem::size_of::<DmaBufferHeader>();
    for index in 0..header.cpu_descriptor_count as usize {
        let offset = descriptor_base + index * core::mem::size_of::<CpuSyncDescriptor>();
        // SAFETY: validation proved the descriptor bounds; use an unaligned copy
        // because callers may provide an unaligned subslice.
        let descriptor = unsafe {
            core::ptr::read_unaligned(bytes.as_ptr().add(offset) as *const CpuSyncDescriptor)
        };
        let tiu_count = descriptor.tiu_descriptor_count & 0xffff;
        let tdma_count = descriptor.tdma_descriptor_count & 0xffff;

        resynchronize_command_ids();
        if tiu_count != 0 {
            start_tiu(descriptor.tiu_descriptor_offset);
        }
        if tdma_count != 0 {
            start_tdma(descriptor.tdma_descriptor_offset, tdma_count);
            wait_tdma(tdma_count)?;
        }
        if tiu_count != 0 {
            wait_tiu(tiu_count)?;
        }
    }
    Ok(())
}

const CACHE_LINE_SIZE: usize = 64;

fn cache_range(address: usize, size: usize, instruction: u32) {
    if size == 0 {
        return;
    }
    let mut current = address & !(CACHE_LINE_SIZE - 1);
    let end = address.saturating_add(size);
    while current < end {
        // SAFETY: these are the exact T-Head C906 cache operation encodings
        // used by the fixed SG2002 Linux kernel. `a0` carries the address.
        unsafe {
            match instruction {
                0x0295_000b => core::arch::asm!(
                    ".word 0x0295000b",
                    in("a0") current,
                    options(nostack, preserves_flags)
                ),
                0x02b5_000b => core::arch::asm!(
                    ".word 0x02b5000b",
                    in("a0") current,
                    options(nostack, preserves_flags)
                ),
                _ => unreachable!(),
            }
        }
        current += CACHE_LINE_SIZE;
    }
    // SAFETY: `sync.is` is the exact completion instruction used after the
    // cache loop by the fixed SG2002 Linux kernel.
    unsafe {
        core::arch::asm!(
            ".word 0x01b0000b",
            "fence iorw, iorw",
            options(nostack, preserves_flags)
        )
    }
}

/// Writes dirty cache lines back before the TPU reads memory.
pub fn cache_writeback_for_device(physical_address: usize, size: usize) {
    cache_range(physical_address, size, 0x0295_000b);
}

/// Writes back and invalidates cache lines before CPU reads TPU output.
pub fn cache_writeback_invalidate(physical_address: usize, size: usize) {
    cache_range(physical_address, size, 0x02b5_000b);
}

/// Reads `REG_CLK_EN_0` without changing any clock state.
pub fn read_clock_enable_0() -> u32 {
    // SAFETY: the fixed SG2002 SDK device tree maps the clock controller at
    // this address, and the CV181x clock driver accesses this register as u32.
    unsafe { core::ptr::read_volatile(CLOCK_ENABLE_0_ADDRESS as *const u32) }
}

/// Reads reset-controller bank 0 without changing any reset state.
pub fn read_reset_bank_0() -> u32 {
    // SAFETY: the fixed SG2002 SDK device tree maps the reset controller at
    // this address, and the CVitek reset driver accesses each bank as u32.
    unsafe { core::ptr::read_volatile(RESET_BANK_0_ADDRESS as *const u32) }
}

/// Returns whether the named bit is set in a 32-bit register value.
pub const fn bit_is_set(value: u32, bit: u32) -> bool {
    value & (1 << bit) != 0
}

fn write_mmio_u32(address: usize, value: u32) {
    // SAFETY: callers pass a 32-bit register address taken from the fixed
    // SG2002 SDK clock or reset controller implementation.
    unsafe {
        core::ptr::write_volatile(address as *mut u32, value);
        core::arch::asm!("fence iorw, iorw", options(nostack, preserves_flags));
    }
}

/// Applies the TPU clock and reset sequence used by the fixed SG2002 SDK.
///
/// This function only controls clocks and resets. It does not write TDMA/TIU
/// registers, submit a command buffer, or start a TPU operation.
pub fn initialize_clocks_and_resets() {
    let clock_mask = (1 << TPU_CLOCK_ENABLE_BIT) | (1 << TPU_FAB_CLOCK_ENABLE_BIT);
    write_mmio_u32(CLOCK_ENABLE_0_ADDRESS, read_clock_enable_0() | clock_mask);

    for bit in [TDMA_RESET_BIT, TPU_RESET_BIT, TPUSYS_RESET_BIT] {
        write_mmio_u32(RESET_BANK_0_ADDRESS, read_reset_bank_0() & !(1 << bit));
    }
    for bit in [TDMA_RESET_BIT, TPU_RESET_BIT, TPUSYS_RESET_BIT] {
        write_mmio_u32(RESET_BANK_0_ADDRESS, read_reset_bank_0() | (1 << bit));
    }
}

/// Reads `TDMA_STATUS` without changing TPU state.
///
/// The boot page table identity-maps the low MMIO region containing TDMA.
pub fn read_tdma_status() -> u32 {
    // SAFETY: TDMA_STATUS is a 32-bit MMIO register explicitly described by
    // the fixed SG2002 SDK device tree and CV181x TPU register header.
    unsafe { core::ptr::read_volatile(TDMA_STATUS_ADDRESS as *const u32) }
}
