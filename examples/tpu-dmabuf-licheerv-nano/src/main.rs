#![no_std]
#![no_main]

use axalloc::GlobalPage;
use axhal::mem::virt_to_phys;
use axplat_riscv64_licheerv_nano::tpu::{
    cache_writeback_for_device, relocate_dma_buffer, validate_dma_buffer,
};
use axstd::println;

const PAGE_SIZE: usize = 4096;
const MODEL_DMABUF: &[u8] = include_bytes!(
    "../../tpu-execute-licheerv-nano/mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin"
);

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 mushroom DMABUF load/relocation probe");
    println!("embedded bytes            = {}", MODEL_DMABUF.len());

    let page_count = MODEL_DMABUF.len().div_ceil(PAGE_SIZE);
    let mut pages = GlobalPage::alloc_contiguous(page_count, PAGE_SIZE)
        .expect("failed to allocate contiguous DMABUF pages");
    pages.as_slice_mut()[..MODEL_DMABUF.len()].copy_from_slice(MODEL_DMABUF);

    let physical_address = pages.start_paddr(virt_to_phys).as_usize();
    let before = validate_dma_buffer(physical_address, pages.as_slice())
        .expect("offline DMABUF validation failed");
    println!("physical address          = {physical_address:#x}");
    println!(
        "magic                     = {:#06x}/{:#06x}",
        before.magic_main, before.magic_sub
    );
    println!(
        "cpu/tiu/tdma descriptors = {}/{}/{}",
        before.cpu_descriptor_count, before.tiu_descriptor_count, before.tdma_descriptor_count
    );

    relocate_dma_buffer(physical_address, pages.as_slice_mut()).expect("DMABUF relocation failed");
    cache_writeback_for_device(physical_address, MODEL_DMABUF.len());
    println!("relocation/cache writeback completed");
    println!("No TIU/TDMA command was submitted.");
}
