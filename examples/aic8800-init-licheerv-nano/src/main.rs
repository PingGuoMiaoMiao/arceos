#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::sdio::{AicSdioIo, initialize_sdio_functions, poll_command_flow_control};
use axdriver_aic8800::sg2002::Sg2002SdioIo;
use axdriver_sg2002_sdio::enumeration::enumerate;
use axdriver_sg2002_sdio::sg2002::{host, register_interrupt};
use axstd::println;

const IDENTIFICATION_CLOCK_HZ: u32 = 400_000;
const FUNCTION_ENABLE_TIMEOUT_MS: u32 = 20;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 AIC8800D80 initialization test");

    if !register_interrupt() {
        println!("AIC8800_INIT_FAILED irq-register");
        return;
    }

    let controller = host();
    if let Err(error) = controller.initialize(IDENTIFICATION_CLOCK_HZ) {
        println!("AIC8800_INIT_FAILED host-init {error:?}");
        return;
    }

    let card = match enumerate(&controller) {
        Ok(card) => card,
        Err(error) => {
            println!("AIC8800_INIT_FAILED card-enumeration {error:?}");
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
            println!("AIC8800_INIT_FAILED function-1-missing");
            return;
        }
    };
    let product = match Aic8800Product::from_sdio_id(function1.vendor, function1.device) {
        Some(Aic8800Product::Aic8800D80) => Aic8800Product::Aic8800D80,
        value => {
            println!("AIC8800_INIT_FAILED product {value:?}");
            return;
        }
    };
    println!(
        "function 1 vendor={:#06x} device={:#06x} product={product:?}",
        function1.vendor, function1.device
    );

    let mut io = match Sg2002SdioIo::new(&controller, FUNCTION_ENABLE_TIMEOUT_MS) {
        Ok(io) => io,
        Err(error) => {
            println!("AIC8800_INIT_FAILED adapter {error:?}");
            return;
        }
    };
    if let Err(error) = initialize_sdio_functions(&mut io, product) {
        println!("AIC8800_INIT_FAILED function-init {error:?}");
        return;
    }

    let function_zero_interrupt = match AicSdioIo::read_register(&mut io, 0, 0x04) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_INIT_FAILED read-f0-interrupt {error:?}");
            return;
        }
    };
    let function_one_interrupt = match AicSdioIo::read_register(&mut io, 1, 0x00) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_INIT_FAILED read-f1-interrupt {error:?}");
            return;
        }
    };
    let byte_mode = match AicSdioIo::read_register(&mut io, 1, 0x07) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_INIT_FAILED read-byte-mode {error:?}");
            return;
        }
    };
    let flow_control = match poll_command_flow_control(&mut io, product) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_INIT_FAILED flow-control {error:?}");
            return;
        }
    };

    println!("function 0 interrupt = {function_zero_interrupt:#04x}");
    println!("function 1 interrupt = {function_one_interrupt:#04x}");
    println!("byte mode            = {byte_mode:#04x}");
    println!("flow control         = {flow_control}");
    if function_zero_interrupt != 0x07
        || function_one_interrupt != 0x07
        || byte_mode != 0x01
        || flow_control == 0
    {
        println!("AIC8800_INIT_FAILED readback");
        return;
    }

    println!("AIC8800_INIT_PASS");
}
