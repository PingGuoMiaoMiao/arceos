use axplat::console::ConsoleIf;
use axplat::mem::{pa, phys_to_virt};

// Board DTS facts:
// compatible = "snps,dw-apb-uart"; reg = <0x0 0x04140000 0x0 0x1000>;
// reg-shift = <2>; reg-io-width = <4>; chosen.stdout-path = "serial0".
const UART0_PADDR: usize = 0x0414_0000;
const UART_REG_SHIFT: usize = 2;
const UART_RBR_THR: usize = 0;
const UART_LSR: usize = 5;
const UART_LSR_DATA_READY: u32 = 1 << 0;
const UART_LSR_THR_EMPTY: u32 = 1 << 5;

#[inline]
fn uart_reg(index: usize) -> *mut u32 {
    let base = phys_to_virt(pa!(UART0_PADDR)).as_usize();
    (base + (index << UART_REG_SHIFT)) as *mut u32
}

#[inline]
fn read_reg(index: usize) -> u32 {
    unsafe { uart_reg(index).read_volatile() }
}

#[inline]
fn write_reg(index: usize, value: u32) {
    unsafe { uart_reg(index).write_volatile(value) }
}

fn putchar(byte: u8) {
    while read_reg(UART_LSR) & UART_LSR_THR_EMPTY == 0 {
        core::hint::spin_loop();
    }
    write_reg(UART_RBR_THR, byte as u32);
}

struct ConsoleIfImpl;

#[impl_interface]
impl ConsoleIf for ConsoleIfImpl {
    fn write_bytes(bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                putchar(b'\r');
            }
            putchar(byte);
        }
    }

    fn read_bytes(bytes: &mut [u8]) -> usize {
        let mut read = 0;
        while read < bytes.len() && read_reg(UART_LSR) & UART_LSR_DATA_READY != 0 {
            bytes[read] = read_reg(UART_RBR_THR) as u8;
            read += 1;
        }
        read
    }
}
