use axplat::irq::{HandlerTable, IpiTarget, IrqHandler, IrqIf};
use axplat::mem::{pa, phys_to_virt};
use core::sync::atomic::{AtomicPtr, Ordering};
use log::warn;
use riscv::register::sie;
use sbi_rt::HartMask;

use crate::plic_controller::{PlicController, RegisterIo};
use crate::plic_layout::PlicLayout;

/// `Interrupt` bit in `scause`.
const INTC_IRQ_BASE: usize = 1 << (usize::BITS - 1);
/// Supervisor software interrupt in `scause`.
const S_SOFT: usize = INTC_IRQ_BASE + 1;
/// Supervisor timer interrupt in `scause`.
const S_TIMER: usize = INTC_IRQ_BASE + 5;
/// Supervisor external interrupt in `scause`.
const S_EXT: usize = INTC_IRQ_BASE + 9;

// Fixed platform data from cv181x_base.dtsi. OpenSBI selects PLIC context 1
// for hart 0's supervisor external interrupt context.
const PLIC_PADDR: usize = 0x7000_0000;
const PLIC_S_CONTEXT: usize = 1;
const PLIC_MAX_IRQ: usize = 101;
const MAX_IRQ_COUNT: usize = PLIC_MAX_IRQ + 1;

static TIMER_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static IPI_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();

struct VolatileMmio;

impl RegisterIo for VolatileMmio {
    fn read_u32(&self, address: usize) -> u32 {
        // SAFETY: Every address is produced by PlicLayout from the mapped PLIC
        // MMIO range, is 32-bit aligned, and is accessed with volatile semantics.
        unsafe { (address as *const u32).read_volatile() }
    }

    fn write_u32(&self, address: usize, value: u32) {
        // SAFETY: Every address is produced by PlicLayout from the mapped PLIC
        // MMIO range, is 32-bit aligned, and is accessed with volatile semantics.
        unsafe { (address as *mut u32).write_volatile(value) }
    }
}

fn plic_controller() -> PlicController<VolatileMmio> {
    let virtual_base = phys_to_virt(pa!(PLIC_PADDR)).as_usize();
    PlicController::new(
        PlicLayout::new(virtual_base, PLIC_S_CONTEXT, PLIC_MAX_IRQ),
        VolatileMmio,
    )
}

macro_rules! with_cause {
    ($cause:expr, @S_TIMER => $timer_op:expr, @S_SOFT => $ipi_op:expr, @S_EXT => $ext_op:expr, @EX_IRQ => $plic_op:expr $(,)?) => {
        match $cause {
            S_TIMER => $timer_op,
            S_SOFT => $ipi_op,
            S_EXT => $ext_op,
            other => {
                if other & INTC_IRQ_BASE == 0 {
                    $plic_op
                } else {
                    panic!("Unknown IRQ cause: {}", other);
                }
            }
        }
    };
}

pub(super) fn init_percpu() {
    plic_controller().initialize();

    // SAFETY: Platform initialization intentionally enables supervisor
    // software, timer, and external interrupt sources for this hart.
    unsafe {
        sie::set_ssoft();
        sie::set_stimer();
        sie::set_sext();
    }
}

struct IrqIfImpl;

#[impl_interface]
impl IrqIf for IrqIfImpl {
    fn set_enable(irq: usize, enabled: bool) {
        if !plic_controller().set_enabled(irq, enabled) {
            warn!("invalid PLIC IRQ {}", irq);
        }
    }

    fn register(irq: usize, handler: IrqHandler) -> bool {
        with_cause!(
            irq,
            @S_TIMER => TIMER_HANDLER
                .compare_exchange(
                    core::ptr::null_mut(),
                    handler as *mut _,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok(),
            @S_SOFT => IPI_HANDLER
                .compare_exchange(
                    core::ptr::null_mut(),
                    handler as *mut _,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok(),
            @S_EXT => {
                warn!("external IRQ must be claimed from PLIC");
                false
            },
            @EX_IRQ => {
                if IRQ_HANDLER_TABLE.register_handler(irq, handler) {
                    Self::set_enable(irq, true);
                    true
                } else {
                    warn!("register handler for PLIC IRQ {} failed", irq);
                    false
                }
            },
        )
    }

    fn unregister(irq: usize) -> Option<IrqHandler> {
        with_cause!(
            irq,
            @S_TIMER => take_atomic_handler(&TIMER_HANDLER),
            @S_SOFT => take_atomic_handler(&IPI_HANDLER),
            @S_EXT => {
                warn!("external IRQ must be claimed from PLIC");
                None
            },
            @EX_IRQ => {
                let handler = IRQ_HANDLER_TABLE.unregister_handler(irq);
                if handler.is_some() {
                    Self::set_enable(irq, false);
                }
                handler
            },
        )
    }

    fn handle(irq: usize) {
        with_cause!(
            irq,
            @S_TIMER => call_atomic_handler(&TIMER_HANDLER),
            @S_SOFT => {
                call_atomic_handler(&IPI_HANDLER);
                // SAFETY: Clearing the supervisor software-pending bit is the
                // required acknowledgement for a supervisor software IRQ.
                unsafe { riscv::register::sip::clear_ssoft() };
            },
            @S_EXT => {
                let mut plic = plic_controller();
                let claimed = plic.claim();
                if claimed == 0 {
                    warn!("PLIC external interrupt had no pending source");
                } else {
                    if !IRQ_HANDLER_TABLE.handle(claimed) {
                        warn!("unhandled PLIC IRQ {}", claimed);
                    }
                    if !plic.complete(claimed) {
                        warn!("PLIC returned out-of-range IRQ {}", claimed);
                    }
                }
            },
            @EX_IRQ => unreachable!("device IRQs arrive through supervisor external IRQ"),
        )
    }

    fn send_ipi(_irq_num: usize, target: IpiTarget) {
        match target {
            IpiTarget::Current { cpu_id } | IpiTarget::Other { cpu_id } => {
                send_ipi_to(cpu_id);
            }
            IpiTarget::AllExceptCurrent { cpu_id, cpu_num } => {
                for target_cpu in 0..cpu_num {
                    if target_cpu != cpu_id {
                        send_ipi_to(target_cpu);
                    }
                }
            }
        }
    }
}

fn take_atomic_handler(slot: &AtomicPtr<()>) -> Option<IrqHandler> {
    let handler = slot.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if handler.is_null() {
        None
    } else {
        // SAFETY: Only IrqHandler function pointers are stored in this slot.
        Some(unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler) })
    }
}

fn call_atomic_handler(slot: &AtomicPtr<()>) {
    let handler = slot.load(Ordering::Acquire);
    if !handler.is_null() {
        // SAFETY: Only IrqHandler function pointers are stored in this slot.
        unsafe { core::mem::transmute::<*mut (), IrqHandler>(handler)() };
    }
}

fn send_ipi_to(cpu_id: usize) {
    let result = sbi_rt::send_ipi(HartMask::from_mask_base(1 << cpu_id, 0));
    if result.is_err() {
        warn!("send_ipi failed: {:?}", result);
    }
}
