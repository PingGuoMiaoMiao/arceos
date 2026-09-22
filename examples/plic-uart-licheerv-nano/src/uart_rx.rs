const UART_RBR: usize = 0;
const UART_IER: usize = 1;
const UART_LSR: usize = 5;

const UART_IER_RDI: u32 = 0x01;
const UART_LSR_DATA_READY: u32 = 0x01;

pub trait RegisterIo {
    fn read_u32(&self, address: usize) -> u32;
    fn write_u32(&self, address: usize, value: u32);
}

pub struct UartRx<I> {
    base: usize,
    register_shift: usize,
    io: I,
}

impl<I: RegisterIo> UartRx<I> {
    pub const fn new(base: usize, register_shift: usize, io: I) -> Self {
        Self {
            base,
            register_shift,
            io,
        }
    }

    #[cfg(test)]
    pub const fn io(&self) -> &I {
        &self.io
    }

    fn register_address(&self, register: usize) -> usize {
        self.base + (register << self.register_shift)
    }

    pub fn enable_receive_interrupt(&self) {
        let ier_address = self.register_address(UART_IER);
        let ier = self.io.read_u32(ier_address);
        self.io.write_u32(ier_address, ier | UART_IER_RDI);
    }

    pub fn disable_receive_interrupt(&self) {
        let ier_address = self.register_address(UART_IER);
        let ier = self.io.read_u32(ier_address);
        self.io.write_u32(ier_address, ier & !UART_IER_RDI);
    }

    pub fn drain_receive_fifo(&self, mut consume: impl FnMut(u8)) {
        while self.io.read_u32(self.register_address(UART_LSR)) & UART_LSR_DATA_READY != 0 {
            consume(self.io.read_u32(self.register_address(UART_RBR)) as u8);
        }
    }
}
