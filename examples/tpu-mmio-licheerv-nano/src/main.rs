#![no_std]
#![no_main]

use axplat_riscv64_licheerv_nano::tpu::{
    CLOCK_ENABLE_0_ADDRESS, RESET_BANK_0_ADDRESS, TDMA_BASE, TDMA_RESET_BIT, TDMA_STATUS_ADDRESS,
    TDMA_STATUS_OFFSET, TIU_BASE, TPU_CLOCK_ENABLE_BIT, TPU_FAB_CLOCK_ENABLE_BIT, TPU_RESET_BIT,
    TPUSYS_RESET_BIT, bit_is_set, read_clock_enable_0, read_reset_bank_0, read_tdma_status,
};
use axstd::println;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 TPU read-only MMIO probe");
    println!("TDMA base          = {TDMA_BASE:#010x}");
    println!("TIU base           = {TIU_BASE:#010x}");
    println!("TDMA_STATUS offset = {TDMA_STATUS_OFFSET:#05x}");
    println!("TDMA_STATUS addr   = {TDMA_STATUS_ADDRESS:#010x}");
    println!("No TPU register will be written.");

    let clock_enable_0 = read_clock_enable_0();
    let reset_bank_0 = read_reset_bank_0();
    println!("REG_CLK_EN_0 addr  = {CLOCK_ENABLE_0_ADDRESS:#010x}");
    println!("REG_CLK_EN_0 value = {clock_enable_0:#010x}");
    println!(
        "clk_tpu enabled     = {}",
        bit_is_set(clock_enable_0, TPU_CLOCK_ENABLE_BIT)
    );
    println!(
        "clk_tpu_fab enabled = {}",
        bit_is_set(clock_enable_0, TPU_FAB_CLOCK_ENABLE_BIT)
    );
    println!("RESET bank 0 addr  = {RESET_BANK_0_ADDRESS:#010x}");
    println!("RESET bank 0 value = {reset_bank_0:#010x}");
    println!(
        "RST_TDMA deasserted = {}",
        bit_is_set(reset_bank_0, TDMA_RESET_BIT)
    );
    println!(
        "RST_TPU deasserted  = {}",
        bit_is_set(reset_bank_0, TPU_RESET_BIT)
    );
    println!(
        "RST_TPUSYS deasserted = {}",
        bit_is_set(reset_bank_0, TPUSYS_RESET_BIT)
    );

    let status = read_tdma_status();
    println!("TDMA_STATUS value  = {status:#010x}");
    println!("TPU read-only MMIO probe completed.");
}
