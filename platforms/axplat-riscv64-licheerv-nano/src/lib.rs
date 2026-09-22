#![no_std]

#[macro_use]
extern crate axplat;

mod boot;
mod boot_page_table;
mod console;
mod init;
#[cfg(feature = "irq")]
mod irq;
mod mem;
#[cfg(feature = "irq")]
mod plic_controller;
#[cfg(feature = "irq")]
mod plic_layout;
mod power;
mod time;
pub mod tpu;

pub mod config {
    axconfig_macros::include_configs!(path_env = "AX_CONFIG_PATH", fallback = "axconfig.toml");
    assert_str_eq!(
        PACKAGE,
        env!("CARGO_PKG_NAME"),
        "platform configuration package does not match the linked platform crate"
    );
}
