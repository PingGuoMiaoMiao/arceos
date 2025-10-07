//! Power management for myqemu platform

/// System shutdown using PSCI (Power State Coordination Interface)
pub fn system_off() -> ! {
    // 使用 PSCI SYSTEM_OFF 调用
    // 在 QEMU 中，这会关闭虚拟机
    const PSCI_SYSTEM_OFF: u32 = 0x84000008;
    unsafe {
        core::arch::asm!(
            "mov w0, {0:w}",
            "hvc #0",              // 调用 hypervisor
            in(reg) PSCI_SYSTEM_OFF,
            options(noreturn)
        );
    }
}