//! Memory management for myqemu platform

extern crate alloc;
use alloc::vec::Vec;

/// Memory region flags
pub use axplat::mem::MemRegionFlags;

/// Physical memory region
pub use axplat::mem::PhysMemRegion;

/// Physical address type
pub use axplat::mem::PhysAddr;

/// Virtual address type
pub use axplat::mem::VirtAddr;

/// Check if sorted ranges overlap  
pub fn check_sorted_ranges_overlap<T: PartialOrd + Clone>(ranges: impl Iterator<Item = (T, T)>) -> Result<(), (T, T)> {
    let ranges: Vec<_> = ranges.collect();
    for i in 1..ranges.len() {
        if ranges[i - 1].1 > ranges[i].0 {
            return Err((ranges[i - 1].1.clone(), ranges[i].0.clone()));
        }
    }
    Ok(())
}

/// Calculate differences between two range sets
pub fn ranges_difference<T: PartialOrd + Clone, F>(
    minuend: &[(T, T)],
    subtrahend: &[(T, T)],
    mut callback: F,
) -> Result<(), (T, T)>
where
    F: FnMut((T, T)),
{
    // 简化实现：仅处理不重叠的原始范围
    for range in minuend {
        let mut current_start = range.0.clone();
        let current_end = range.1.clone();
        
        let mut overlapped = false;
        for sub_range in subtrahend {
            if sub_range.0 < current_end && sub_range.1 > current_start {
                overlapped = true;
                break;
            }
        }
        
        if !overlapped {
            callback((current_start, current_end));
        }
    }
    Ok(())
}

/// Memory layout constants for QEMU virt machine
pub const PHYS_MEMORY_BASE: usize = 0x4000_0000;
pub const PHYS_MEMORY_SIZE: usize = 0x800_0000; // 128MB

/// Get memory regions for this platform
pub fn memory_regions() -> &'static [PhysMemRegion] {
    use axplat::mem::PhysAddr;
    
    static REGIONS: &[PhysMemRegion] = &[
        PhysMemRegion {
            paddr: PhysAddr::from_usize(PHYS_MEMORY_BASE),
            size: PHYS_MEMORY_SIZE,
            flags: MemRegionFlags::FREE,
            name: "free memory",
        },
    ];
    REGIONS
}

/// Get MMIO regions
pub fn mmio_ranges() -> &'static [(usize, usize)] {
    static MMIO: &[(usize, usize)] = &[
        (0x0900_0000, 0x1000), // UART
    ];
    MMIO
}

/// Get physical RAM ranges
pub fn phys_ram_ranges() -> &'static [(usize, usize)] {
    static RAM: &[(usize, usize)] = &[
        (PHYS_MEMORY_BASE, PHYS_MEMORY_SIZE),
    ];
    RAM
}

/// Get reserved physical RAM ranges
pub fn reserved_phys_ram_ranges() -> &'static [(usize, usize)] {
    static RESERVED: &[(usize, usize)] = &[
        // 内核镜像区域
        (0x4020_0000, 0x100_0000), // 假设内核大小为 16MB
    ];
    RESERVED
}

/// Get total RAM size
pub fn total_ram_size() -> usize {
    PHYS_MEMORY_SIZE
}

/// Convert physical address to virtual address
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    // 简化的线性映射
    // 物理地址 + 虚拟地址偏移 = 虚拟地址
    const PHYS_VIRT_OFFSET: usize = 0xffff_0000_0000_0000;
    VirtAddr::from_usize(paddr.as_usize() + PHYS_VIRT_OFFSET)
}

/// Convert virtual address to physical address
pub fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    // 简化的线性映射的逆操作
    const PHYS_VIRT_OFFSET: usize = 0xffff_0000_0000_0000;
    PhysAddr::from_usize(vaddr.as_usize() - PHYS_VIRT_OFFSET)
}