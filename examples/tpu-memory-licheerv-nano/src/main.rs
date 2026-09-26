#![no_std]
#![no_main]

use axalloc::GlobalPage;
use axhal::mem::virt_to_phys;
use axplat_riscv64_licheerv_nano::tpu::{
    CpuSyncDescriptor, DMABUF_MAGIC_MAIN, DmaBufferHeader, cache_writeback_for_device,
    cache_writeback_invalidate, validate_dma_buffer,
};
use axstd::println;

const DMA_BUFFER_SIZE: usize = 20 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 TPU contiguous-memory/cache probe");
    println!(
        "DmaBufferHeader size     = {}",
        size_of::<DmaBufferHeader>()
    );
    println!(
        "CpuSyncDescriptor size   = {}",
        size_of::<CpuSyncDescriptor>()
    );

    let mut pages = GlobalPage::alloc_contiguous(DMA_BUFFER_SIZE / PAGE_SIZE, PAGE_SIZE)
        .expect("failed to allocate 20 MiB contiguous pages");
    let virtual_address = pages.start_vaddr();
    let physical_address = pages.start_paddr(virt_to_phys);
    println!("virtual address          = {virtual_address:#x}");
    println!("physical address         = {physical_address:#x}");
    println!("allocated bytes          = {}", pages.size());
    println!(
        "physical 4K alignment    = {}",
        physical_address.as_usize() & (PAGE_SIZE - 1) == 0
    );

    pages.as_slice_mut()[..PAGE_SIZE].fill(0x5a);
    cache_writeback_for_device(physical_address.as_usize(), PAGE_SIZE);
    cache_writeback_invalidate(physical_address.as_usize(), PAGE_SIZE);
    println!("cache writeback/invalidate completed");

    let header = DmaBufferHeader {
        magic_main: DMABUF_MAGIC_MAIN,
        magic_sub: 0,
        buffer_size: size_of::<DmaBufferHeader>() as u32,
        cpu_descriptor_count: 0,
        tiu_descriptor_count: 0,
        tdma_descriptor_count: 0,
        tpu_clock_rate: 0,
        pmu_buffer_size: 0,
        pmu_buffer_offset: 0,
        array_bases: [[0; 2]; 8],
        reserved: [0; 8],
    };
    // SAFETY: the destination is page-aligned and large enough for the exact
    // 128-byte C-compatible header.
    unsafe { (pages.as_mut_ptr() as *mut DmaBufferHeader).write(header) };
    let parsed = validate_dma_buffer(physical_address.as_usize(), pages.as_slice())
        .expect("generated dmabuf header did not validate");
    println!("dmabuf magic             = {:#06x}", parsed.magic_main);
    println!("dmabuf validation completed");
    println!("No TIU/TDMA command was submitted.");
}
