use crate::patch_table::{
    AicPatchApplyError, AicPatchMemory, AicPatchTable, AicPatchTableError,
    D80_SDIO_BT_DEFAULT_PATCH_SETTINGS, apply_d80_bt_patch_table, parse_d80_patch_info,
};

pub const D80_CHIP_VERSION_ADDRESS: u32 = 0x4050_0000;
pub const D80_WIFI_FIRMWARE_ADDRESS: u32 = 0x0012_0000;
pub const D80_HOST_START_APP_AUTO: u32 = 1;

const D80_PATCH_POINTER_ADDRESS: u32 = D80_WIFI_FIRMWARE_ADDRESS + 0x0198;
const D80_PATCH_STRUCTURE_POINTER_ADDRESS: u32 = D80_PATCH_POINTER_ADDRESS + 8;
const D80_PATCH_BUFFER_POINTER_ADDRESS: u32 = D80_PATCH_POINTER_ADDRESS + 12;
const D80_FIRMWARE_VERSION_ADDRESS: u32 = D80_WIFI_FIRMWARE_ADDRESS + 0x001c;
const D80_NEW_PATCH_BUFFER_VERSION: u32 = 0x0609_0100;
const D80_LEGACY_PATCH_BUFFER_ADDRESS: u32 = 0x0016_f800;
const AIC_PATCH_MAGIC: u32 = 0x4843_5450;
const AIC_PATCH_MAGIC_2: u32 = 0x5054_4348;
const AIC_PATCH_MAGIC_OFFSET: u32 = 0;
const AIC_PATCH_PAIR_START_OFFSET: u32 = 4;
const AIC_PATCH_MAGIC_2_OFFSET: u32 = 8;
const AIC_PATCH_PAIR_COUNT_OFFSET: u32 = 12;
const AIC_PATCH_BLOCK_SIZE_OFFSET: u32 = 48;
const D80_WIFI_PATCH: [(u32, u32); 3] = [
    (0x00b4, 0xf301_0000),
    (0x0170, 0x0100_000a),
    (0x0188, 0x0000_0003),
];

pub trait D80FirmwareIo: AicPatchMemory {
    fn read_word(&mut self, address: u32) -> Result<u32, Self::Error>;
    fn upload_image(&mut self, address: u32, image: &[u8]) -> Result<(), Self::Error>;
    fn start_app(&mut self, address: u32, boot_type: u32) -> Result<u32, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D80ExtPatchImage<'a> {
    pub id: u32,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum D80FirmwareOperation {
    UploadAdid,
    UploadBluetoothPatch,
    UploadBluetoothExtension,
    UploadWifi,
    ReadConfigBase,
    ReadPatchStructure,
    ReadFirmwareVersion,
    ReadPatchBuffer,
    WritePatchStructure,
    WritePatchPair,
    WritePatchBlockSize,
    StartWifi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum D80FirmwareError<E> {
    PatchTable(AicPatchTableError),
    ExtensionCount {
        required: usize,
        available: usize,
    },
    ExtensionId {
        index: usize,
        required: u32,
        actual: u32,
    },
    AddressOverflow,
    Operation {
        operation: D80FirmwareOperation,
        source: E,
    },
    PatchApply(AicPatchApplyError<E>),
}

pub fn load_d80_bluetooth<M: D80FirmwareIo>(
    io: &mut M,
    patch_table: &AicPatchTable<'_>,
    adid: &[u8],
    patch: &[u8],
    extension_images: &[D80ExtPatchImage<'_>],
) -> Result<(), D80FirmwareError<M::Error>> {
    let info = parse_d80_patch_info(patch_table).map_err(D80FirmwareError::PatchTable)?;
    let required_extensions = info.ext_patch_count() as usize;
    if extension_images.len() != required_extensions {
        return Err(D80FirmwareError::ExtensionCount {
            required: required_extensions,
            available: extension_images.len(),
        });
    }

    io.upload_image(info.adid_address, adid)
        .map_err(|source| D80FirmwareError::Operation {
            operation: D80FirmwareOperation::UploadAdid,
            source,
        })?;
    io.upload_image(info.patch_address, patch)
        .map_err(|source| D80FirmwareError::Operation {
            operation: D80FirmwareOperation::UploadBluetoothPatch,
            source,
        })?;

    for (index, ((required_id, address), image)) in
        info.ext_patches().zip(extension_images.iter()).enumerate()
    {
        if image.id != required_id {
            return Err(D80FirmwareError::ExtensionId {
                index,
                required: required_id,
                actual: image.id,
            });
        }
        io.upload_image(address, image.bytes)
            .map_err(|source| D80FirmwareError::Operation {
                operation: D80FirmwareOperation::UploadBluetoothExtension,
                source,
            })?;
    }

    apply_d80_bt_patch_table(io, patch_table, D80_SDIO_BT_DEFAULT_PATCH_SETTINGS)
        .map_err(D80FirmwareError::PatchApply)
}

pub fn load_d80_wifi_and_start<M: D80FirmwareIo>(
    io: &mut M,
    wifi_firmware: &[u8],
) -> Result<u32, D80FirmwareError<M::Error>> {
    io.upload_image(D80_WIFI_FIRMWARE_ADDRESS, wifi_firmware)
        .map_err(|source| D80FirmwareError::Operation {
            operation: D80FirmwareOperation::UploadWifi,
            source,
        })?;
    configure_d80_wifi_patch(io)?;
    io.start_app(D80_WIFI_FIRMWARE_ADDRESS, D80_HOST_START_APP_AUTO)
        .map_err(|source| D80FirmwareError::Operation {
            operation: D80FirmwareOperation::StartWifi,
            source,
        })
}

pub fn configure_d80_wifi_patch<M: D80FirmwareIo>(
    io: &mut M,
) -> Result<(), D80FirmwareError<M::Error>> {
    let config_base = read_for(
        io,
        D80_PATCH_POINTER_ADDRESS,
        D80FirmwareOperation::ReadConfigBase,
    )?;
    let patch_structure = read_for(
        io,
        D80_PATCH_STRUCTURE_POINTER_ADDRESS,
        D80FirmwareOperation::ReadPatchStructure,
    )?;
    let firmware_version = read_for(
        io,
        D80_FIRMWARE_VERSION_ADDRESS,
        D80FirmwareOperation::ReadFirmwareVersion,
    )?;
    let patch_buffer = if firmware_version > D80_NEW_PATCH_BUFFER_VERSION {
        read_for(
            io,
            D80_PATCH_BUFFER_POINTER_ADDRESS,
            D80FirmwareOperation::ReadPatchBuffer,
        )?
    } else {
        D80_LEGACY_PATCH_BUFFER_ADDRESS
    };

    write_for(
        io,
        add(patch_structure, AIC_PATCH_MAGIC_OFFSET)?,
        AIC_PATCH_MAGIC,
        D80FirmwareOperation::WritePatchStructure,
    )?;
    write_for(
        io,
        add(patch_structure, AIC_PATCH_MAGIC_2_OFFSET)?,
        AIC_PATCH_MAGIC_2,
        D80FirmwareOperation::WritePatchStructure,
    )?;
    write_for(
        io,
        add(patch_structure, AIC_PATCH_PAIR_START_OFFSET)?,
        patch_buffer,
        D80FirmwareOperation::WritePatchStructure,
    )?;
    write_for(
        io,
        add(patch_structure, AIC_PATCH_PAIR_COUNT_OFFSET)?,
        D80_WIFI_PATCH.len() as u32,
        D80FirmwareOperation::WritePatchStructure,
    )?;

    for (index, &(config_offset, value)) in D80_WIFI_PATCH.iter().enumerate() {
        let pair_address = add(patch_buffer, (index as u32) * 8)?;
        write_for(
            io,
            pair_address,
            add(config_base, config_offset)?,
            D80FirmwareOperation::WritePatchPair,
        )?;
        write_for(
            io,
            add(pair_address, 4)?,
            value,
            D80FirmwareOperation::WritePatchPair,
        )?;
    }

    for index in 0..4_u32 {
        write_for(
            io,
            add(patch_structure, AIC_PATCH_BLOCK_SIZE_OFFSET + index * 4)?,
            0,
            D80FirmwareOperation::WritePatchBlockSize,
        )?;
    }
    Ok(())
}

fn read_for<M: D80FirmwareIo>(
    io: &mut M,
    address: u32,
    operation: D80FirmwareOperation,
) -> Result<u32, D80FirmwareError<M::Error>> {
    io.read_word(address)
        .map_err(|source| D80FirmwareError::Operation { operation, source })
}

fn write_for<M: D80FirmwareIo>(
    io: &mut M,
    address: u32,
    value: u32,
    operation: D80FirmwareOperation,
) -> Result<(), D80FirmwareError<M::Error>> {
    io.write_word(address, value)
        .map_err(|source| D80FirmwareError::Operation { operation, source })
}

fn add<E>(base: u32, offset: u32) -> Result<u32, D80FirmwareError<E>> {
    base.checked_add(offset)
        .ok_or(D80FirmwareError::AddressOverflow)
}
