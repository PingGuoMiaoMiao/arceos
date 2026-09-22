const GIGAPAGE_SIZE: usize = 1 << 30;
const SV39_ROOT_INDEX_MASK: usize = 0x1ff;

pub const fn map_gigapage(
    root: &mut [u64; 512],
    virtual_address: usize,
    physical_address: usize,
    flags: u64,
) {
    assert!(
        virtual_address & (GIGAPAGE_SIZE - 1) == 0,
        "gigapage virtual address must be 1 GiB aligned"
    );
    assert!(
        physical_address & (GIGAPAGE_SIZE - 1) == 0,
        "gigapage physical address must be 1 GiB aligned"
    );

    let root_index = (virtual_address >> 30) & SV39_ROOT_INDEX_MASK;
    root[root_index] = (((physical_address >> 12) as u64) << 10) | flags;
}
