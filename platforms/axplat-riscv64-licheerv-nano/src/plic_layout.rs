const PRIORITY_BASE: usize = 0x0;
const ENABLE_BASE: usize = 0x2000;
const ENABLE_CONTEXT_STRIDE: usize = 0x80;
const CONTEXT_BASE: usize = 0x20_0000;
const CONTEXT_STRIDE: usize = 0x1000;
const CONTEXT_THRESHOLD: usize = 0x0;
const CONTEXT_CLAIM_COMPLETE: usize = 0x4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlicLayout {
    base: usize,
    context: usize,
    max_irq: usize,
}

impl PlicLayout {
    pub const fn new(base: usize, context: usize, max_irq: usize) -> Self {
        Self {
            base,
            context,
            max_irq,
        }
    }

    pub const fn is_valid_irq(self, irq: usize) -> bool {
        irq > 0 && irq <= self.max_irq
    }

    pub const fn enable_word_count(self) -> usize {
        self.max_irq / u32::BITS as usize + 1
    }

    pub const fn enable_word_address_at(self, word_index: usize) -> Option<usize> {
        if word_index < self.enable_word_count() {
            Some(
                self.base
                    + ENABLE_BASE
                    + self.context * ENABLE_CONTEXT_STRIDE
                    + word_index * size_of::<u32>(),
            )
        } else {
            None
        }
    }

    pub const fn priority_address(self, irq: usize) -> Option<usize> {
        if self.is_valid_irq(irq) {
            Some(self.base + PRIORITY_BASE + irq * size_of::<u32>())
        } else {
            None
        }
    }

    pub const fn enable_word_address(self, irq: usize) -> Option<usize> {
        if self.is_valid_irq(irq) {
            self.enable_word_address_at(irq / u32::BITS as usize)
        } else {
            None
        }
    }

    pub const fn enable_bit(self, irq: usize) -> Option<u32> {
        if self.is_valid_irq(irq) {
            Some(1_u32 << (irq % u32::BITS as usize))
        } else {
            None
        }
    }

    pub const fn threshold_address(self) -> usize {
        self.base + CONTEXT_BASE + self.context * CONTEXT_STRIDE + CONTEXT_THRESHOLD
    }

    pub const fn claim_complete_address(self) -> usize {
        self.base + CONTEXT_BASE + self.context * CONTEXT_STRIDE + CONTEXT_CLAIM_COMPLETE
    }
}
