//! Initialization for myqemu platform

/// Early platform initialization
pub fn init_early(cpu_id: usize, arg: usize) {
    let _ = (cpu_id, arg); // 避免未使用参数警告
    
    // 早期平台初始化
    // 在真实实现中，这里会：
    // 1. 设置内存映射
    // 2. 初始化中断控制器
    // 3. 设置时钟
    axlog::debug!("MyQemu platform early init complete");
}

/// Later platform initialization
pub fn init_later(cpu_id: usize, arg: usize) {
    let _ = (cpu_id, arg); // 避免未使用参数警告
    // 后期平台初始化
    // 在真实实现中，这里会：
    // 1. 初始化设备驱动
    // 2. 设置更复杂的硬件特性
    axlog::debug!("MyQemu platform late init complete");
}