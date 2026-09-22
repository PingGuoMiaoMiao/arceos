#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

use axstd::println;

#[unsafe(no_mangle)]
fn main() {
    println!("Hello from ArceOS on LicheeRV Nano!");
}
