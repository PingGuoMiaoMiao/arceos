#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::firmware::{
    DBG_MEM_READ_CONFIRM, DBG_MEM_READ_REQUEST, DEBUG_TASK_ID, DRIVER_TASK_ID,
    decode_debug_memory_read_confirmation, encode_debug_memory_read_parameters,
};
use axdriver_aic8800::sdio::initialize_sdio_functions;
use axdriver_aic8800::sg2002::Sg2002SdioIo;
use axdriver_aic8800::transaction::execute_config_command;
use axdriver_sg2002_sdio::enumeration::enumerate;
use axdriver_sg2002_sdio::sg2002::{host, register_interrupt};
use axstd::println;

const IDENTIFICATION_CLOCK_HZ: u32 = 400_000;
const FUNCTION_ENABLE_TIMEOUT_MS: u32 = 20;
const COMMAND_RESPONSE_TIMEOUT_MS: u32 = 6000;
const CHIP_VERSION_ADDRESS: u32 = 0x4050_0000;
const EXPECTED_CHIP_VERSION_WORD: u32 = 0xf3c7_8820;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 AIC8800D80 command response test");

    if !register_interrupt() {
        println!("AIC8800_COMMAND_FAILED irq-register");
        return;
    }
    let controller = host();
    if let Err(error) = controller.initialize(IDENTIFICATION_CLOCK_HZ) {
        println!("AIC8800_COMMAND_FAILED host-init {error:?}");
        return;
    }
    let card = match enumerate(&controller) {
        Ok(card) => card,
        Err(error) => {
            println!("AIC8800_COMMAND_FAILED card-enumeration {error:?}");
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
            println!("AIC8800_COMMAND_FAILED function-1-missing");
            return;
        }
    };
    let product = match Aic8800Product::from_sdio_id(function1.vendor, function1.device) {
        Some(Aic8800Product::Aic8800D80) => Aic8800Product::Aic8800D80,
        value => {
            println!("AIC8800_COMMAND_FAILED product {value:?}");
            return;
        }
    };

    let mut io = match Sg2002SdioIo::new(&controller, FUNCTION_ENABLE_TIMEOUT_MS) {
        Ok(io) => io,
        Err(error) => {
            println!("AIC8800_COMMAND_FAILED adapter {error:?}");
            return;
        }
    };
    if let Err(error) = initialize_sdio_functions(&mut io, product) {
        println!("AIC8800_COMMAND_FAILED function-init {error:?}");
        return;
    }

    let mut parameter = [0_u8; 4];
    if let Err(error) = encode_debug_memory_read_parameters(&mut parameter, CHIP_VERSION_ADDRESS) {
        println!("AIC8800_COMMAND_FAILED encode {error:?}");
        return;
    }
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; 512];
    let response = match execute_config_command(
        &mut io,
        product,
        DBG_MEM_READ_REQUEST,
        DBG_MEM_READ_CONFIRM,
        DEBUG_TASK_ID,
        DRIVER_TASK_ID,
        &parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    ) {
        Ok(response) => response,
        Err(error) => {
            println!("AIC8800_COMMAND_FAILED transaction {error:?}");
            return;
        }
    };
    let version_word =
        match decode_debug_memory_read_confirmation(response.parameter, CHIP_VERSION_ADDRESS) {
            Ok(value) => value,
            Err(error) => {
                println!("AIC8800_COMMAND_FAILED decode {error:?}");
                return;
            }
        };

    println!("chip version address = {CHIP_VERSION_ADDRESS:#010x}");
    println!("chip version word    = {version_word:#010x}");
    if version_word != EXPECTED_CHIP_VERSION_WORD {
        println!(
            "AIC8800_COMMAND_FAILED version expected={EXPECTED_CHIP_VERSION_WORD:#010x} actual={version_word:#010x}"
        );
        return;
    }
    println!("AIC8800_COMMAND_PASS");
}
