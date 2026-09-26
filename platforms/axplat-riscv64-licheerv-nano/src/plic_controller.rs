use crate::plic_layout::PlicLayout;

pub trait RegisterIo {
    fn read_u32(&self, address: usize) -> u32;
    fn write_u32(&self, address: usize, value: u32);
}

pub struct PlicController<I> {
    layout: PlicLayout,
    io: I,
}

impl<I: RegisterIo> PlicController<I> {
    pub const fn new(layout: PlicLayout, io: I) -> Self {
        Self { layout, io }
    }

    pub fn initialize(&mut self) {
        for word_index in 0..self.layout.enable_word_count() {
            let address = self
                .layout
                .enable_word_address_at(word_index)
                .expect("enable word index is in range");
            self.io.write_u32(address, 0);
        }
        self.io.write_u32(self.layout.threshold_address(), 0);
    }

    pub fn set_enabled(&mut self, irq: usize, enabled: bool) -> bool {
        let Some(priority_address) = self.layout.priority_address(irq) else {
            return false;
        };
        let Some(enable_word_address) = self.layout.enable_word_address(irq) else {
            return false;
        };
        let Some(enable_bit) = self.layout.enable_bit(irq) else {
            return false;
        };

        let old_enable_word = self.io.read_u32(enable_word_address);
        let new_enable_word = if enabled {
            old_enable_word | enable_bit
        } else {
            old_enable_word & !enable_bit
        };
        self.io.write_u32(enable_word_address, new_enable_word);
        self.io
            .write_u32(priority_address, if enabled { 1 } else { 0 });
        true
    }

    pub fn claim(&mut self) -> usize {
        self.io.read_u32(self.layout.claim_complete_address()) as usize
    }

    pub fn complete(&mut self, irq: usize) -> bool {
        if !self.layout.is_valid_irq(irq) {
            return false;
        }
        self.io
            .write_u32(self.layout.claim_complete_address(), irq as u32);
        true
    }
}
