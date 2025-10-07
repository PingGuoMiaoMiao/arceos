//! Implementation of `axplat` for the custom myqemu platform
//! 
//! This is a simplified QEMU AArch64 virtual machine platform implementation
//! to demonstrate how to create a custom platform in ArceOS.

#![no_std]

// Re-export the main macro from axplat
pub use axplat::main;

use axplat::impl_plat_interface;
use axplat::console::ConsoleIf;
use axplat::init::InitIf;
#[cfg(feature = "irq")]
use axplat::irq::{IpiTarget, IrqHandler, IrqIf};
use axplat::mem::{MemIf, RawRange};
use axplat::power::PowerIf;
use axplat::time::TimeIf;

pub mod console;
pub mod time;
pub mod misc;
pub mod mem;
pub mod power;
pub mod init;
mod boot;

/// UART base address for QEMU virt machine
const UART_BASE: usize = 0x0900_0000;

/// Physical memory base address
const PHYS_MEMORY_BASE: usize = 0x4000_0000;
/// Physical memory size (128MB)
const PHYS_MEMORY_SIZE: usize = 0x800_0000;
/// Kernel load address
const KERNEL_BASE_PADDR: usize = 0x4020_0000;

struct MyQemuInit;
struct MyQemuConsole;
struct MyQemuMem;
struct MyQemuTime;
struct MyQemuPower;
#[cfg(feature = "irq")]
struct MyQemuIrq;

#[impl_plat_interface]
impl InitIf for MyQemuInit {
    fn init_early(_cpu_id: usize, _arg: usize) {
        // 确保在早期初始化阶段就初始化控制台
        console::init();
        // 使用简单的方式输出初始化消息，避免使用 axlog（它可能依赖其他未初始化的组件）
        console::write_bytes(b"MyQemu platform early init\n");
    }

    #[cfg(feature = "smp")]
    fn init_early_secondary(_cpu_id: usize) {}

    fn init_later(_cpu_id: usize, _arg: usize) {
        console::write_bytes(b"MyQemu platform late init\n");
    }

    #[cfg(feature = "smp")]
    fn init_later_secondary(_cpu_id: usize) {}
}

#[impl_plat_interface]
impl ConsoleIf for MyQemuConsole {
    fn write_bytes(bytes: &[u8]) {
        console::write_bytes(bytes);
    }

    fn read_bytes(_bytes: &mut [u8]) -> usize {
        // Not implemented for now
        0
    }
}

#[impl_plat_interface]
impl MemIf for MyQemuMem {
    fn phys_ram_ranges() -> &'static [RawRange] {
        static RANGES: &[RawRange] = &[(
            PHYS_MEMORY_BASE,
            PHYS_MEMORY_BASE + PHYS_MEMORY_SIZE,
        )];
        RANGES
    }

    fn reserved_phys_ram_ranges() -> &'static [RawRange] {
        &[]
    }

    fn mmio_ranges() -> &'static [RawRange] {
        static RANGES: &[RawRange] = &[(0x0900_0000, 0x0900_1000)]; // UART
        RANGES
    }

    fn phys_to_virt(paddr: memory_addr::PhysAddr) -> memory_addr::VirtAddr {
        memory_addr::VirtAddr::from(paddr.as_usize())
    }

    fn virt_to_phys(vaddr: memory_addr::VirtAddr) -> memory_addr::PhysAddr {
        memory_addr::PhysAddr::from(vaddr.as_usize())
    }
}

#[impl_plat_interface]
impl TimeIf for MyQemuTime {
    fn current_ticks() -> u64 {
        time::current_time_nanos()
    }

    fn ticks_to_nanos(ticks: u64) -> u64 {
        ticks  // Our ticks are already in nanoseconds
    }

    fn nanos_to_ticks(nanos: u64) -> u64 {
        nanos  // Our ticks are already in nanoseconds
    }

    fn epochoffset_nanos() -> u64 {
        0  // No epoch offset for now
    }

    fn set_oneshot_timer(_deadline_ns: u64) {
        // Not implemented for now
    }
}

#[impl_plat_interface]
impl PowerIf for MyQemuPower {
    #[cfg(feature = "smp")]
    fn cpu_boot(_cpu_id: usize, _stack_top_paddr: usize) {
        // Not implemented for now
    }

    fn system_off() -> ! {
        misc::system_off()
    }
}

#[cfg(feature = "irq")]
#[impl_plat_interface]
impl IrqIf for MyQemuIrq {
    fn set_enable(_irq: usize, _enabled: bool) {
        // Not implemented for now
    }

    fn register(_irq: usize, _handler: IrqHandler) -> bool {
        false  // Not implemented for now
    }

    fn unregister(_irq: usize) -> Option<IrqHandler> {
        None  // Not implemented for now
    }

    fn handle(_irq: usize) {
        // Not implemented for now
    }

    fn send_ipi(_irq: usize, _target: IpiTarget) {
        // Not implemented for now
    }
}