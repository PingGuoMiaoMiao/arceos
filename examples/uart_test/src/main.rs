//! ArceOS UART 测试程序
//! 作业 2: QEMU 平台适配与 UART 测试

#![no_std]
#![no_main]

use axstd::println;

/// 主测试函数
#[no_mangle]
fn main() {
    // 作业要求的基本 UART 测试
    println!("=== ArceOS UART Test ===");
    println!("Test UART");
    println!("Platform: QEMU virt aarch64");
    println!("HAL abstraction: Working!");
    
    // 测试格式化输出
    for i in 1..=3 {
        println!("UART Test Message {}", i);
    }
    
    println!("=== UART Test Complete ===");
}