#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

mod uart_rx;

use axhal::mem::{pa, phys_to_virt};
use axstd::println;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use uart_rx::{RegisterIo, UartRx};

// soph_base.dtsi and soph_base_riscv.dtsi define these values.
const UART0_PADDR: usize = 0x0414_0000;
const UART0_REGISTER_SHIFT: usize = 2;
const UART0_IRQ: usize = 44;

static RX_COUNT: AtomicUsize = AtomicUsize::new(0);
static LAST_RX_BYTE: AtomicU8 = AtomicU8::new(0);

struct VolatileMmio;

impl RegisterIo for VolatileMmio {
    fn read_u32(&self, address: usize) -> u32 {
        // SAFETY: UartRx only produces aligned 32-bit addresses inside UART0's
        // mapped 4 KiB MMIO range.
        unsafe { (address as *const u32).read_volatile() }
    }

    fn write_u32(&self, address: usize, value: u32) {
        // SAFETY: UartRx only produces aligned 32-bit addresses inside UART0's
        // mapped 4 KiB MMIO range.
        unsafe { (address as *mut u32).write_volatile(value) }
    }
}

fn uart0() -> UartRx<VolatileMmio> {
    UartRx::new(
        phys_to_virt(pa!(UART0_PADDR)).as_usize(),
        UART0_REGISTER_SHIFT,
        VolatileMmio,
    )
}

fn uart0_receive_handler() {
    uart0().drain_receive_fifo(|byte| {
        LAST_RX_BYTE.store(byte, Ordering::Release);
        RX_COUNT.fetch_add(1, Ordering::AcqRel);
    });
}

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 PLIC + UART0 RX interrupt test");
    println!("UART0 IRQ = {UART0_IRQ}");

    let uart = uart0();
    uart.disable_receive_interrupt();
    uart.drain_receive_fifo(|_| {});

    if !axhal::irq::register(UART0_IRQ, uart0_receive_handler) {
        println!("UART0_RX_IRQ_REGISTER_FAILED");
        return;
    }

    uart.enable_receive_interrupt();
    println!("UART0_RX_IRQ_READY: send one character over the serial terminal");

    while RX_COUNT.load(Ordering::Acquire) == 0 {
        core::hint::spin_loop();
    }

    uart.disable_receive_interrupt();
    let count = RX_COUNT.load(Ordering::Acquire);
    let byte = LAST_RX_BYTE.load(Ordering::Acquire);
    println!("UART0_RX_IRQ_PASS count={count} last_byte={byte:#04x}");
}
