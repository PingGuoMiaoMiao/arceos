use axplat::power::PowerIf;

struct PowerIfImpl;

#[impl_interface]
impl PowerIf for PowerIfImpl {
    fn system_off() -> ! {
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
        loop {
            axcpu::asm::halt();
        }
    }

    fn cpu_num() -> usize {
        crate::config::plat::MAX_CPU_NUM
    }
}
