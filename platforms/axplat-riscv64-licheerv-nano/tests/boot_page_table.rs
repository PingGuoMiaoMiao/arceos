#[path = "../src/boot_page_table.rs"]
mod boot_page_table;

use boot_page_table::map_gigapage;

const PHYS_VIRT_OFFSET: usize = 0xffff_ffc0_0000_0000;
const DEVICE_FLAGS: u64 = (1 << 60) | (1 << 63) | 0xef;

#[test]
fn plic_gigapage_is_mapped_identity_and_in_the_high_half() {
    let mut root = [0_u64; 512];

    map_gigapage(&mut root, 0x4000_0000, 0x4000_0000, DEVICE_FLAGS);
    map_gigapage(
        &mut root,
        PHYS_VIRT_OFFSET + 0x4000_0000,
        0x4000_0000,
        DEVICE_FLAGS,
    );

    let expected = (0x40000_u64 << 10) | DEVICE_FLAGS;
    assert_eq!(root[1], expected);
    assert_eq!(root[0x101], expected);
    assert_eq!(root[0], 0);
    assert_eq!(root[2], 0);
}

#[test]
#[should_panic(expected = "gigapage virtual address must be 1 GiB aligned")]
fn unaligned_virtual_address_is_rejected() {
    let mut root = [0_u64; 512];
    map_gigapage(&mut root, 0x7000_0000, 0x4000_0000, DEVICE_FLAGS);
}

#[test]
#[should_panic(expected = "gigapage physical address must be 1 GiB aligned")]
fn unaligned_physical_address_is_rejected() {
    let mut root = [0_u64; 512];
    map_gigapage(&mut root, 0x4000_0000, 0x7000_0000, DEVICE_FLAGS);
}
