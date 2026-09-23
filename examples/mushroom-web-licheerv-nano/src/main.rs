#![cfg_attr(feature = "hardware", no_std)]
#![cfg_attr(feature = "hardware", no_main)]

#[cfg(feature = "hardware")]
extern crate axplat_riscv64_licheerv_nano;

#[cfg(feature = "hardware")]
#[unsafe(no_mangle)]
fn main() {
    arceos_mushroom_web_licheerv_nano::hardware::run();
}

#[cfg(not(feature = "hardware"))]
fn main() {}
