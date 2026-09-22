#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use axdriver_sg2002_sdio::enumeration::enumerate;
use axdriver_sg2002_sdio::sg2002::{SDIO1_IRQ, SDIO1_PADDR, host, register_interrupt};
use axstd::println;

const IDENTIFICATION_CLOCK_HZ: u32 = 400_000;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 SDIO1 enumeration test");
    println!("SDIO1 base = {SDIO1_PADDR:#010x}");
    println!("SDIO1 IRQ  = {SDIO1_IRQ}");

    if !register_interrupt() {
        println!("SDIO_ENUMERATION_FAILED irq-register");
        return;
    }

    let controller = host();
    let identification_clock = match controller.initialize(IDENTIFICATION_CLOCK_HZ) {
        Ok(actual_hz) => actual_hz,
        Err(error) => {
            println!("SDIO_ENUMERATION_FAILED host-init {error:?}");
            return;
        }
    };
    println!("identification clock = {identification_clock} Hz");

    let card = match enumerate(&controller) {
        Ok(card) => card,
        Err(error) => {
            println!("SDIO_ENUMERATION_FAILED card-enumeration {error:?}");
            return;
        }
    };

    println!("function count = {}", card.function_count);
    println!("OCR            = {:#010x}", card.ocr);
    println!("RCA            = {:#06x}", card.rca);
    println!("CCCR revision  = {}", card.cccr_revision);
    println!("SDIO revision  = {}", card.sdio_revision);
    println!("capabilities   = {:#04x}", card.capabilities);
    println!("speed          = {:#04x}", card.speed);
    println!("run clock      = {} Hz", card.actual_clock_hz);
    for function in card.functions.iter().flatten() {
        println!(
            "function {} class={:#04x} vendor={:#06x} device={:#06x} max_block_size={}",
            function.number,
            function.class,
            function.vendor,
            function.device,
            function.max_block_size,
        );
    }
    println!("SDIO_ENUMERATION_PASS");
}
