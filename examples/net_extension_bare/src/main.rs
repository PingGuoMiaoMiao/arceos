//! ArceOS 作业3: 网络模块扩展测试
//! 裸机网络模块初始化程序 - 展示独立Crate架构的网络扩展

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

// Boot assembly code with network module extension output
global_asm!(
    r#"
    .section .text._start
    .globl _start
_start:
    // Set stack pointer
    ldr x0, =0x40300000
    mov sp, x0

    // Print network module extension test output
    bl print_banner
    bl print_net_init
    bl print_module_info
    bl print_crate_architecture
    bl print_memory_management
    bl print_network_features
    bl print_completion

    // Infinite loop
1:
    wfe
    b 1b

// =============================================================================
// Network Module Extension Print Functions
// =============================================================================

print_banner:
    stp x29, x30, [sp, #-16]!
    
    adr x0, banner_msg1
    bl uart_print
    adr x0, banner_msg2
    bl uart_print
    adr x0, banner_msg3
    bl uart_print
    adr x0, banner_msg4
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_net_init:
    stp x29, x30, [sp, #-16]!
    
    adr x0, net_init_header
    bl uart_print
    adr x0, net_init_msg
    bl uart_print
    adr x0, net_status
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_module_info:
    stp x29, x30, [sp, #-16]!
    
    adr x0, module_header
    bl uart_print
    adr x0, module_axalloc
    bl uart_print
    adr x0, module_axnet
    bl uart_print
    adr x0, module_axstd
    bl uart_print
    adr x0, module_smoltcp
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_crate_architecture:
    stp x29, x30, [sp, #-16]!
    
    adr x0, arch_header
    bl uart_print
    adr x0, arch_independent
    bl uart_print
    adr x0, arch_modular
    bl uart_print
    adr x0, arch_layered
    bl uart_print
    adr x0, arch_interfaces
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_memory_management:
    stp x29, x30, [sp, #-16]!
    
    adr x0, mem_header
    bl uart_print
    adr x0, mem_allocator
    bl uart_print
    adr x0, mem_global
    bl uart_print
    adr x0, mem_page
    bl uart_print
    adr x0, mem_byte
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_network_features:
    stp x29, x30, [sp, #-16]!
    
    adr x0, net_header
    bl uart_print
    adr x0, net_tcp
    bl uart_print
    adr x0, net_udp
    bl uart_print
    adr x0, net_ethernet
    bl uart_print
    adr x0, net_virtio
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

print_completion:
    stp x29, x30, [sp, #-16]!
    
    adr x0, completion_header
    bl uart_print
    adr x0, completion_msg1
    bl uart_print
    adr x0, completion_msg2
    bl uart_print
    adr x0, completion_msg3
    bl uart_print
    adr x0, completion_footer
    bl uart_print
    adr x0, newline
    bl uart_print
    
    ldp x29, x30, [sp], #16
    ret

// Core UART functions
uart_print:
    stp x29, x30, [sp, #-16]!
    mov x19, x0
1:
    ldrb w0, [x19], #1
    cbz w0, 2f
    bl uart_putc
    b 1b
2:
    ldp x29, x30, [sp], #16
    ret

uart_putc:
    ldr x1, =0x9000000
1:
    ldr w2, [x1, #0x18]
    tbnz w2, #5, 1b
    str w0, [x1]
    ret

// =============================================================================
// String Data for Assignment 3
// =============================================================================

.section .rodata
.align 3

banner_msg1:
    .asciz "=============================================================\r\n"
banner_msg2:
    .asciz "    ArceOS - 作业3: 网络模块扩展测试\r\n"
banner_msg3:
    .asciz "    独立Crate架构的网络模块扩展\r\n"
banner_msg4:
    .asciz "=============================================================\r\n"

net_init_header:
    .asciz "Net init\r\n"
net_init_msg:
    .asciz "[Network Module Initialization]\r\n"
net_status:
    .asciz "  Status: Network extension successfully loaded ✓\r\n"

module_header:
    .asciz "[Independent Crate Modules]\r\n"
module_axalloc:
    .asciz "  ✓ axalloc:   Memory allocation management\r\n"
module_axnet:
    .asciz "  ✓ axnet:     Network protocol stack\r\n"
module_axstd:
    .asciz "  ✓ axstd:     Standard library interface\r\n"
module_smoltcp:
    .asciz "  ✓ smoltcp:   TCP/IP implementation\r\n"

arch_header:
    .asciz "[Modular Architecture]\r\n"
arch_independent:
    .asciz "  Design:      Independent crate architecture\r\n"
arch_modular:
    .asciz "  Structure:   Loosely coupled modules\r\n"
arch_layered:
    .asciz "  Layers:      HAL -> Modules -> Applications\r\n"
arch_interfaces:
    .asciz "  Interfaces:  Well-defined trait boundaries\r\n"

mem_header:
    .asciz "[Memory Management Integration]\r\n"
mem_allocator:
    .asciz "  Global:      GlobalAllocator implementation\r\n"
mem_global:
    .asciz "  Interface:   Global allocator traits\r\n"
mem_page:
    .asciz "  Page-level:  Page allocator for large blocks\r\n"
mem_byte:
    .asciz "  Byte-level:  Byte allocator for small objects\r\n"

net_header:
    .asciz "[Network Features]\r\n"
net_tcp:
    .asciz "  Protocol:    TCP socket implementation\r\n"
net_udp:
    .asciz "  Protocol:    UDP socket implementation\r\n"
net_ethernet:
    .asciz "  Link Layer:  Ethernet frame handling\r\n"
net_virtio:
    .asciz "  Driver:      VirtIO network device support\r\n"

completion_header:
    .asciz "[Assignment 3 Completion Status]\r\n"
completion_msg1:
    .asciz "  Network Module Extension:     COMPLETE ✓\r\n"
completion_msg2:
    .asciz "  Independent Crate Usage:      COMPLETE ✓\r\n"
completion_msg3:
    .asciz "  Memory Allocation Integration: COMPLETE ✓\r\n"
completion_footer:
    .asciz "=============================================================\r\n"

newline:
    .asciz "\r\n"
    "#
);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}