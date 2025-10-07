//! Console implementation using ARM PL011 UART
//! 
//! This provides a simple UART driver for console output

use kspin::SpinNoIrq;
use lazyinit::LazyInit;

/// UART base address for QEMU virt machine
const UART_BASE: usize = 0x0900_0000;

/// Simple PL011 UART driver
struct Pl011Uart {
    base_addr: usize,
}

impl Pl011Uart {
    const fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    fn write_byte(&self, byte: u8) {
        unsafe {
            let data_reg = self.base_addr as *mut u8;
            data_reg.write_volatile(byte);
        }
    }

    fn write_bytes(&self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_byte(byte);
        }
    }

    fn read_byte(&self) -> Option<u8> {
        // 简化实现：假设没有输入数据
        // 在真实实现中，这里应该读取 UART 的数据寄存器和状态寄存器
        None
    }
}

static UART: LazyInit<SpinNoIrq<Pl011Uart>> = LazyInit::new();

/// Initialize the console
pub fn init() {
    UART.init_once(SpinNoIrq::new(Pl011Uart::new(UART_BASE)));
    // 使用简单的字节输出，避免使用可能未初始化的 axlog
    write_bytes(b"MyQemu UART console initialized\n");
}

/// Write bytes to console
pub fn write_bytes(bytes: &[u8]) {
    UART.lock().write_bytes(bytes);
}

/// Read bytes from console
pub fn read_bytes(bytes: &mut [u8]) -> usize {
    // 简化实现：返回 0（没有读取到数据）
    // 在真实实现中，这里应该从 UART 读取数据
    let _ = bytes; // 避免未使用参数警告
    0
}