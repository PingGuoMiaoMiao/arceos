use axplat::init::InitIf;

struct InitIfImpl;

#[impl_interface]
impl InitIf for InitIfImpl {
    fn init_early(_cpu_id: usize, _dtb: usize) {
        // The first milestone reuses UART0 setup left by the firmware/U-Boot.
        // Clock, reset, pinmux, and baud-rate programming require a separate,
        // source-backed initialization sequence before they are changed here.
        axcpu::init::init_trap();
    }

    fn init_later(_cpu_id: usize, _dtb: usize) {
        #[cfg(feature = "irq")]
        crate::irq::init_percpu();
        crate::time::init_percpu();
    }
}
