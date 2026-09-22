#![no_std]
#![no_main]

use axplat_riscv64_licheerv_nano::tpu::{
    TDMA_RESET_BIT, TPU_CLOCK_ENABLE_BIT, TPU_FAB_CLOCK_ENABLE_BIT, TPU_RESET_BIT,
    TPUSYS_RESET_BIT, bit_is_set, initialize_clocks_and_resets, read_clock_enable_0,
    read_reset_bank_0, read_tdma_status,
};
use axstd::println;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 TPU clock/reset initialization probe");
    println!("No TDMA/TIU command register will be written.");

    let clock_before = read_clock_enable_0();
    let reset_before = read_reset_bank_0();
    let status_before = read_tdma_status();
    println!("clock before = {clock_before:#010x}");
    println!("reset before = {reset_before:#010x}");
    println!("status before = {status_before:#010x}");

    initialize_clocks_and_resets();

    let clock_after = read_clock_enable_0();
    let reset_after = read_reset_bank_0();
    let status_after = read_tdma_status();
    println!("clock after  = {clock_after:#010x}");
    println!("reset after  = {reset_after:#010x}");
    println!("status after = {status_after:#010x}");
    println!(
        "clk_tpu enabled       = {}",
        bit_is_set(clock_after, TPU_CLOCK_ENABLE_BIT)
    );
    println!(
        "clk_tpu_fab enabled   = {}",
        bit_is_set(clock_after, TPU_FAB_CLOCK_ENABLE_BIT)
    );
    println!(
        "RST_TDMA deasserted   = {}",
        bit_is_set(reset_after, TDMA_RESET_BIT)
    );
    println!(
        "RST_TPU deasserted    = {}",
        bit_is_set(reset_after, TPU_RESET_BIT)
    );
    println!(
        "RST_TPUSYS deasserted = {}",
        bit_is_set(reset_after, TPUSYS_RESET_BIT)
    );
    println!("TPU clock/reset initialization probe completed.");
}
