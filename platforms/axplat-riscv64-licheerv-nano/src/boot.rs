use crate::boot_page_table::map_gigapage;
use crate::config::plat::{BOOT_STACK_SIZE, PHYS_VIRT_OFFSET};
use axplat::mem::{Aligned4K, pa};

#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

#[unsafe(link_section = ".data")]
static BOOT_PT_SV39: Aligned4K<[u64; 512]> = build_boot_page_table();

// SG2002's C906 enables the T-Head MAEE page attributes. These bit positions
// match arch/riscv/include/asm/pgtable-bits.h in Sophgo's Linux 5.10 tree.
const C906_PAGE_SHARE: u64 = 1 << 60;
const C906_PAGE_BUF: u64 = 1 << 61;
const C906_PAGE_CACHE: u64 = 1 << 62;
const C906_PAGE_SO: u64 = 1 << 63;
const C906_NORMAL_MEMORY: u64 = C906_PAGE_SHARE | C906_PAGE_BUF | C906_PAGE_CACHE;
const C906_DEVICE_MEMORY: u64 = C906_PAGE_SHARE | C906_PAGE_SO;

const PTE_RWX_GAD: u64 = 0xef;

const fn build_boot_page_table() -> Aligned4K<[u64; 512]> {
    let mut root = [0_u64; 512];

    // Low MMIO (0x0000_0000..0x3fff_ffff).
    map_gigapage(
        &mut root,
        0x0000_0000,
        0x0000_0000,
        PTE_RWX_GAD | C906_DEVICE_MEMORY,
    );
    map_gigapage(
        &mut root,
        PHYS_VIRT_OFFSET,
        0x0000_0000,
        PTE_RWX_GAD | C906_DEVICE_MEMORY,
    );

    // PLIC lies at 0x7000_0000, inside this 1 GiB device window.
    map_gigapage(
        &mut root,
        0x4000_0000,
        0x4000_0000,
        PTE_RWX_GAD | C906_DEVICE_MEMORY,
    );
    map_gigapage(
        &mut root,
        PHYS_VIRT_OFFSET + 0x4000_0000,
        0x4000_0000,
        PTE_RWX_GAD | C906_DEVICE_MEMORY,
    );

    // DDR starts at 0x8000_0000.
    map_gigapage(
        &mut root,
        0x8000_0000,
        0x8000_0000,
        PTE_RWX_GAD | C906_NORMAL_MEMORY,
    );
    map_gigapage(
        &mut root,
        PHYS_VIRT_OFFSET + 0x8000_0000,
        0x8000_0000,
        PTE_RWX_GAD | C906_NORMAL_MEMORY,
    );

    Aligned4K::new(root)
}

unsafe fn init_mmu() {
    unsafe {
        axcpu::asm::write_kernel_page_table(pa!(&raw const BOOT_PT_SV39 as usize));
        axcpu::asm::flush_tlb(None);
    }
}

/// Earliest entry for the C906B core.
///
/// The board SDK fixes the payload entry at physical address 0x8020_0000.
/// The RISC-V firmware convention passes the hart ID in a0 and DTB address in a1.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        mv      s0, a0
        mv      s1, a1
        la      sp, {boot_stack}
        li      t0, {boot_stack_size}
        add     sp, sp, t0

        call    {init_mmu}

        li      s2, {phys_virt_offset}
        add     sp, sp, s2

        mv      a0, s0
        mv      a1, s1
        la      a2, {entry}
        add     a2, a2, s2
        jalr    a2
        j       .
        ",
        phys_virt_offset = const PHYS_VIRT_OFFSET,
        boot_stack_size = const BOOT_STACK_SIZE,
        boot_stack = sym BOOT_STACK,
        init_mmu = sym init_mmu,
        entry = sym axplat::call_main,
    )
}
