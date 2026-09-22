#[path = "../src/plic_controller.rs"]
mod plic_controller;
#[path = "../src/plic_layout.rs"]
mod plic_layout;

use plic_controller::{PlicController, RegisterIo};
use plic_layout::PlicLayout;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const SG2002_PLIC: PlicLayout = PlicLayout::new(0x7000_0000, 1, 101);

#[test]
fn irq_38_uses_supervisor_context_one_registers() {
    assert_eq!(SG2002_PLIC.priority_address(38), Some(0x7000_0098));
    assert_eq!(SG2002_PLIC.enable_word_address(38), Some(0x7000_2084));
    assert_eq!(SG2002_PLIC.enable_bit(38), Some(1 << 6));
    assert_eq!(SG2002_PLIC.threshold_address(), 0x7020_1000);
    assert_eq!(SG2002_PLIC.claim_complete_address(), 0x7020_1004);
}

#[test]
fn only_device_interrupt_ids_one_through_101_are_valid() {
    assert!(!SG2002_PLIC.is_valid_irq(0));
    assert!(SG2002_PLIC.is_valid_irq(1));
    assert!(SG2002_PLIC.is_valid_irq(101));
    assert!(!SG2002_PLIC.is_valid_irq(102));

    assert_eq!(SG2002_PLIC.priority_address(0), None);
    assert_eq!(SG2002_PLIC.enable_word_address(102), None);
    assert_eq!(SG2002_PLIC.enable_bit(102), None);
}

#[derive(Clone, Default)]
struct FakeIo {
    registers: Rc<RefCell<BTreeMap<usize, u32>>>,
    writes: Rc<RefCell<Vec<(usize, u32)>>>,
}

impl FakeIo {
    fn set(&self, address: usize, value: u32) {
        self.registers.borrow_mut().insert(address, value);
    }

    fn get(&self, address: usize) -> u32 {
        self.registers.borrow().get(&address).copied().unwrap_or(0)
    }
}

impl RegisterIo for FakeIo {
    fn read_u32(&self, address: usize) -> u32 {
        self.get(address)
    }

    fn write_u32(&self, address: usize, value: u32) {
        self.registers.borrow_mut().insert(address, value);
        self.writes.borrow_mut().push((address, value));
    }
}

#[test]
fn initialization_clears_all_supervisor_enable_words_and_threshold() {
    let io = FakeIo::default();
    let mut plic = PlicController::new(SG2002_PLIC, io.clone());

    plic.initialize();

    assert_eq!(
        io.writes.borrow().as_slice(),
        &[
            (0x7000_2080, 0),
            (0x7000_2084, 0),
            (0x7000_2088, 0),
            (0x7000_208c, 0),
            (0x7020_1000, 0),
        ]
    );
}

#[test]
fn enabling_irq_38_preserves_other_bits_and_sets_priority_one() {
    let io = FakeIo::default();
    io.set(0x7000_2084, 0x0000_0002);
    let mut plic = PlicController::new(SG2002_PLIC, io.clone());

    assert!(plic.set_enabled(38, true));

    assert_eq!(io.get(0x7000_2084), 0x0000_0042);
    assert_eq!(io.get(0x7000_0098), 1);
}

#[test]
fn disabling_irq_38_clears_only_its_bit_and_priority() {
    let io = FakeIo::default();
    io.set(0x7000_2084, 0x0000_0042);
    io.set(0x7000_0098, 1);
    let mut plic = PlicController::new(SG2002_PLIC, io.clone());

    assert!(plic.set_enabled(38, false));

    assert_eq!(io.get(0x7000_2084), 0x0000_0002);
    assert_eq!(io.get(0x7000_0098), 0);
}

#[test]
fn invalid_irq_is_rejected_without_register_writes() {
    let io = FakeIo::default();
    let mut plic = PlicController::new(SG2002_PLIC, io.clone());

    assert!(!plic.set_enabled(0, true));
    assert!(!plic.set_enabled(102, true));

    assert!(io.writes.borrow().is_empty());
}

#[test]
fn claim_and_complete_use_the_same_supervisor_context_register() {
    let io = FakeIo::default();
    io.set(0x7020_1004, 38);
    let mut plic = PlicController::new(SG2002_PLIC, io.clone());

    assert_eq!(plic.claim(), 38);
    assert!(plic.complete(38));

    assert_eq!(io.writes.borrow().last(), Some(&(0x7020_1004, 38)));
}
