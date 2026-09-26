#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::firmware::{
    DBG_MEM_READ_CONFIRM, DBG_MEM_READ_REQUEST, DEBUG_TASK_ID, DRIVER_TASK_ID,
    decode_debug_memory_read_confirmation, encode_debug_memory_read_parameters,
    upload_firmware_image,
};
use axdriver_aic8800::sdio::initialize_sdio_functions;
use axdriver_aic8800::sg2002::Sg2002SdioIo;
use axdriver_aic8800::transaction::execute_config_command;
use axdriver_sg2002_sdio::enumeration::enumerate;
use axdriver_sg2002_sdio::function::SdioTransferBus;
use axdriver_sg2002_sdio::sg2002::{host, register_interrupt};
use axstd::println;

const IDENTIFICATION_CLOCK_HZ: u32 = 400_000;
const FUNCTION_ENABLE_TIMEOUT_MS: u32 = 20;
const COMMAND_RESPONSE_TIMEOUT_MS: u32 = 6000;
const CHIP_VERSION_ADDRESS: u32 = 0x4050_0000;
const EXPECTED_CHIP_VERSION_WORD: u32 = 0xf3c7_8820;
const WIFI_FIRMWARE_ADDRESS: u32 = 0x0012_0000;
const FIRMWARE_PREFIX_LENGTH: usize = 1024;

fn receive_exact(bytes: &mut [u8]) {
    let mut received = 0;
    while received < bytes.len() {
        let count = axhal::console::read_bytes(&mut bytes[received..]);
        if count == 0 {
            core::hint::spin_loop();
        } else {
            received += count;
        }
    }
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn read_debug_word<B: SdioTransferBus>(
    io: &mut Sg2002SdioIo<'_, B>,
    product: Aic8800Product,
    address: u32,
    transmit: &mut [u8],
    receive: &mut [u8],
) -> Result<u32, &'static str> {
    let mut parameter = [0_u8; 4];
    encode_debug_memory_read_parameters(&mut parameter, address).map_err(|_| "encode")?;
    let response = execute_config_command(
        io,
        product,
        DBG_MEM_READ_REQUEST,
        DBG_MEM_READ_CONFIRM,
        DEBUG_TASK_ID,
        DRIVER_TASK_ID,
        &parameter,
        transmit,
        receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    )
    .map_err(|_| "transaction")?;
    decode_debug_memory_read_confirmation(response.parameter, address).map_err(|_| "decode")
}

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 AIC8800D80 firmware block test");

    if !register_interrupt() {
        println!("AIC8800_FIRMWARE_BLOCK_FAILED irq-register");
        return;
    }
    let controller = host();
    if let Err(error) = controller.initialize(IDENTIFICATION_CLOCK_HZ) {
        println!("AIC8800_FIRMWARE_BLOCK_FAILED host-init {error:?}");
        return;
    }
    let card = match enumerate(&controller) {
        Ok(card) => card,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED card-enumeration {error:?}");
            return;
        }
    };
    let function1 = match card
        .functions
        .iter()
        .flatten()
        .find(|function| function.number == 1)
    {
        Some(function) => function,
        None => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED function-1-missing");
            return;
        }
    };
    let product = match Aic8800Product::from_sdio_id(function1.vendor, function1.device) {
        Some(Aic8800Product::Aic8800D80) => Aic8800Product::Aic8800D80,
        value => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED product {value:?}");
            return;
        }
    };
    let mut io = match Sg2002SdioIo::new(&controller, FUNCTION_ENABLE_TIMEOUT_MS) {
        Ok(io) => io,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED adapter {error:?}");
            return;
        }
    };
    if let Err(error) = initialize_sdio_functions(&mut io, product) {
        println!("AIC8800_FIRMWARE_BLOCK_FAILED function-init {error:?}");
        return;
    }

    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let version_word = match read_debug_word(
        &mut io,
        product,
        CHIP_VERSION_ADDRESS,
        &mut transmit,
        &mut receive,
    ) {
        Ok(value) => value,
        Err(stage) => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED version-{stage}");
            return;
        }
    };
    if version_word != EXPECTED_CHIP_VERSION_WORD {
        println!(
            "AIC8800_FIRMWARE_BLOCK_FAILED version expected={EXPECTED_CHIP_VERSION_WORD:#010x} actual={version_word:#010x}"
        );
        return;
    }

    println!("READY AIC_FIRMWARE_PREFIX {FIRMWARE_PREFIX_LENGTH}");
    let mut firmware_prefix = [0_u8; FIRMWARE_PREFIX_LENGTH];
    receive_exact(&mut firmware_prefix);
    let mut transmitted_crc_bytes = [0_u8; 4];
    receive_exact(&mut transmitted_crc_bytes);
    let transmitted_crc = u32::from_le_bytes(transmitted_crc_bytes);
    let received_crc = crc32_ieee(&firmware_prefix);
    if received_crc != transmitted_crc {
        println!(
            "AIC8800_FIRMWARE_BLOCK_FAILED crc expected={transmitted_crc:08x} actual={received_crc:08x}"
        );
        return;
    }

    let expected_first_word = u32::from_le_bytes(firmware_prefix[..4].try_into().unwrap());
    let mut parameter = [0_u8; FIRMWARE_PREFIX_LENGTH + 8];
    if let Err(error) = upload_firmware_image(
        &mut io,
        product,
        WIFI_FIRMWARE_ADDRESS,
        &firmware_prefix,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    ) {
        println!("AIC8800_FIRMWARE_BLOCK_FAILED upload {error:?}");
        return;
    }
    let actual_first_word = match read_debug_word(
        &mut io,
        product,
        WIFI_FIRMWARE_ADDRESS,
        &mut transmit,
        &mut receive,
    ) {
        Ok(value) => value,
        Err(stage) => {
            println!("AIC8800_FIRMWARE_BLOCK_FAILED readback-{stage}");
            return;
        }
    };

    println!("firmware prefix crc32 = {received_crc:08x}");
    println!("write address         = {WIFI_FIRMWARE_ADDRESS:#010x}");
    println!("expected first word   = {expected_first_word:#010x}");
    println!("readback first word   = {actual_first_word:#010x}");
    if actual_first_word != expected_first_word {
        println!("AIC8800_FIRMWARE_BLOCK_FAILED readback-mismatch");
        return;
    }
    println!("AIC8800_FIRMWARE_BLOCK_PASS");
}
