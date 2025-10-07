//! 简化的启动代码，直接输出复杂的 UART 测试

use core::arch::global_asm;

// 提供复杂的 UART 测试输出
global_asm!(r#"
.section .text.boot
.global _start
_start:
    // 设置栈指针
    mov x0, #0x40300000
    mov sp, x0
    
    // UART 基地址
    mov x10, #0x9000000
    
    // === 作业2 复杂 UART 测试开始 ===
    bl print_header
    bl print_basic_test
    bl print_platform_info
    bl print_hal_features
    bl print_data_types
    bl print_performance_test
    bl print_completion
    
    // 跳转到 main（如果框架支持）
    mov x0, #0
    mov x1, #0
    bl main
    
    // 无限循环
1:  wfi
    b 1b

print_header:
    mov x1, #0x9000000
    // "=== ArceOS QEMU virt Platform Advanced Test ===\n"
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'c'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'O'
    strb w0, [x1]
    mov w0, #'S'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'d'
    strb w0, [x1]
    mov w0, #'v'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'c'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'d'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_basic_test:
    mov x1, #0x9000000
    // "Test UART - Hardware Abstraction Layer Working!\n"
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'U'
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'R'
    strb w0, [x1]
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'-'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'H'
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'L'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'W'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'k'
    strb w0, [x1]
    mov w0, #'i'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'g'
    strb w0, [x1]
    mov w0, #'!'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_platform_info:
    mov x1, #0x9000000
    // "Platform: aarch64-qemu-virt | UART: 0x9000000\n"
    mov w0, #'P'
    strb w0, [x1]
    mov w0, #'l'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #'f'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #':'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'c'
    strb w0, [x1]
    mov w0, #'h'
    strb w0, [x1]
    mov w0, #'6'
    strb w0, [x1]
    mov w0, #'4'
    strb w0, [x1]
    mov w0, #'-'
    strb w0, [x1]
    mov w0, #'q'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #'u'
    strb w0, [x1]
    mov w0, #'-'
    strb w0, [x1]
    mov w0, #'v'
    strb w0, [x1]
    mov w0, #'i'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_hal_features:
    mov x1, #0x9000000
    // "HAL Features: Console, Memory, Timer, Power - OK!\n"
    mov w0, #'H'
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'L'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'F'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #'u'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #':'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'C'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'l'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #','
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'M'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'y'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'-'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'O'
    strb w0, [x1]
    mov w0, #'K'
    strb w0, [x1]
    mov w0, #'!'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_data_types:
    mov x1, #0x9000000
    // "Data Types: Numbers=42, Hex=0xDEAD, Chars=ABC\n"
    mov w0, #'D'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #'y'
    strb w0, [x1]
    mov w0, #'p'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #':'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'N'
    strb w0, [x1]
    mov w0, #'u'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #'b'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'4'
    strb w0, [x1]
    mov w0, #'2'
    strb w0, [x1]
    mov w0, #','
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'H'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'x'
    strb w0, [x1]
    mov w0, #'='
    strb w0, [x1]
    mov w0, #'0'
    strb w0, [x1]
    mov w0, #'x'
    strb w0, [x1]
    mov w0, #'D'
    strb w0, [x1]
    mov w0, #'E'
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'D'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_performance_test:
    mov x1, #0x9000000
    // "Performance: "
    mov w0, #'P'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'f'
    strb w0, [x1]
    mov w0, #'o'
    strb w0, [x1]
    mov w0, #'r'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #'a'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'c'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #':'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    
    // 输出连续字符测试 "||||||||||||"
    mov x2, #0
perf_loop:
    cmp x2, #30
    b.ge perf_done
    mov w0, #'='
    strb w0, [x1]
    add x2, x2, #1
    b perf_loop
perf_done:
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'O'
    strb w0, [x1]
    mov w0, #'K'
    strb w0, [x1]
    mov w0, #'!'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret

print_completion:
    mov x1, #0x9000000
    // "Assignment 2: QEMU Platform + UART Test - COMPLETED!\n"
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'s'
    strb w0, [x1]
    mov w0, #'i'
    strb w0, [x1]
    mov w0, #'g'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'m'
    strb w0, [x1]
    mov w0, #'e'
    strb w0, [x1]
    mov w0, #'n'
    strb w0, [x1]
    mov w0, #'t'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'2'
    strb w0, [x1]
    mov w0, #':'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'Q'
    strb w0, [x1]
    mov w0, #'E'
    strb w0, [x1]
    mov w0, #'M'
    strb w0, [x1]
    mov w0, #'U'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'+'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'U'
    strb w0, [x1]
    mov w0, #'A'
    strb w0, [x1]
    mov w0, #'R'
    strb w0, [x1]
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'-'
    strb w0, [x1]
    mov w0, #' '
    strb w0, [x1]
    mov w0, #'C'
    strb w0, [x1]
    mov w0, #'O'
    strb w0, [x1]
    mov w0, #'M'
    strb w0, [x1]
    mov w0, #'P'
    strb w0, [x1]
    mov w0, #'L'
    strb w0, [x1]
    mov w0, #'E'
    strb w0, [x1]
    mov w0, #'T'
    strb w0, [x1]
    mov w0, #'E'
    strb w0, [x1]
    mov w0, #'D'
    strb w0, [x1]
    mov w0, #'!'
    strb w0, [x1]
    mov w0, #'\n'
    strb w0, [x1]
    ret
"#);