//! Miscellaneous platform functions (power management, etc.)

/// System shutdown
pub fn system_off() -> ! {
    axlog::info!("MyQemu platform: shutting down");
    // Use PSCI function call to power off
    unsafe {
        core::arch::asm!(
            "ldr x0, ={psci_fn}",   // Load PSCI_POWER_OFF
            "hvc #0",               // Hypervisor call
            psci_fn = const 0x84000008u64,
            options(noreturn)
        );
    }
}

/// System reset
pub fn system_reset() -> ! {
    axlog::info!("MyQemu platform: resetting");
    // Use PSCI function call to reset
    unsafe {
        core::arch::asm!(
            "ldr x0, ={psci_fn}",   // Load PSCI_SYSTEM_RESET
            "hvc #0",               // Hypervisor call
            psci_fn = const 0x84000009u64,
            options(noreturn)
        );
    }
}

/// CPU idle
pub fn cpu_idle() {
    unsafe {
        core::arch::asm!("wfi");  // Wait for interrupt
    }
}