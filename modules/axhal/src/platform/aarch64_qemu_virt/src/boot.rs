//! Boot code for QEMU virt aarch64
#![no_std]

#[no_mangle]
pub unsafe fn platform_init() {
    // Initialize console
    super::console::init();
    
    // Output test message - 作业要求的输出！
    super::console::put_str("Test UART - QEMU virt aarch64\n");
    super::console::put_str("HAL abstraction working!\n");
}

