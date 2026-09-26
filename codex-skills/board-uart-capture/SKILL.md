---
name: board-uart-capture
description: Use when a Windows host must identify a USB-TTL adapter such as CH340, CP210x, or FTDI and capture or diagnose a development-board UART boot log around a manual power-on or reset.
---

# Board UART Capture

## Overview

建立只读串口证据链：先核对板卡资料和接线，再确认准确 COM 口，看到 `SERIAL_READY` 后才通知用户上电或复位，最后依据原始日志分类结果。不得把“收到字节”当成“正常启动”。

## Workflow

1. 从官方板卡资料、原理图或用户照片提取准确的 `GND/TX/RX`、逻辑电平和波特率。资料没有给出的内容不得猜测，必须向用户索取证据。
2. 指导断电接线：共地，适配器 TX 接板卡 RX，适配器 RX 接板卡 TX。板卡独立供电时，不连接适配器电源脚。
3. 在 Windows 中读取现有串口设备及其完整名称。多个串口并存时必须显式指定目标端口。
4. 找到已安装 `pyserial` 的 Python 解释器，再运行 `python scripts/uart_capture.py --list-ports`。不得因为默认 `python` 缺少模块就安装或覆盖环境。
5. 使用明确端口、已核实波特率和唯一的新日志路径启动捕获。只有工具打印 `SERIAL_READY` 后，才通知用户上电或按一次 RESET。
6. 结束捕获后保留原始日志，并报告字节数与分类：`NO_DATA`、`TEXT` 或 `UNREADABLE`。
7. `UNREADABLE` 表示链路收到数据但当前设置无法可靠解码。**先跑 `--diagnose-log` 取信号指纹再动手**，按下面的诊断顺序处理，每次只改变一个变量。

## UNREADABLE 诊断顺序

不要靠逐个试波特率去“碰”出文本。先用指纹判断问题属于哪一类：

1. 运行 `--diagnose-log <原始日志>`，读取 `msb_ratio` 与 `bit_profile`。
2. 真实 ASCII 文本的 `b7`（第 8 位）恒为 0，因此 `msb_ratio` 明显大于 `0.02` 就**不可能是波特率错配的文本**。
3. 若 `line_state=NON_TEXT_SIGNAL`，停止调波特率，改为逐项核对：共地、TX/RX 是否接反、逻辑电平（TTL 与 RS-232 是否混用）、以及板卡是否真的进入了会打印的阶段（供电、复位、启动介质、启动模式跳线）。
4. 用 `--compare-logs <已知可读日志> <可疑日志>` 对比指纹。`verdict=DIFFERENT_SIGNAL` 说明两者不是同一条信号，不能继续沿用旧接线假设；`verdict=SAME_SIGNAL` 说明是同一条稳定信号，可跨采集复现。
5. 用 `--scan-logs <日志目录>` 找出**最后一次可读**的采集，从而框定回归窗口：问题是在哪两次采集之间出现的。

指纹是稳定量：同一条信号在不同时间采集的两次快照，其 `bit_profile` 之间 L1 距离通常低于 `0.05`，而可读文本与乱码流之间会超过 `0.5`。

### 区分“接线问题”与“板子启动问题”

当按 RESET 前后信号指纹相同、且全文最长连续 ASCII 只有个位数时，该信号**不是 SoC 的 UART 输出**（复位至少会带来某种变化）。此时唯一的分支判据是：

- **只拔掉适配器的 RX 信号线**（USB 与 GND 保持不动），再采一次；
- 信号仍在 → 引脚悬浮拾取干扰，查接线、共地、针脚；
- 信号消失 → 板子确实在驱动该线，查启动介质与启动模式。

## Commands

在 Skill 根目录执行：

```powershell
py -3.12 scripts/uart_capture.py --list-ports
py -3.12 scripts/uart_capture.py --port COM3 --baud 115200 --log C:\path\board-reset.raw.log
py -3.12 scripts/uart_capture.py --analyze-log C:\path\board-reset.raw.log
py -3.12 scripts/uart_capture.py --diagnose-log C:\path\board-reset.raw.log
py -3.12 scripts/uart_capture.py --compare-logs C:\path\known-good.raw.log C:\path\board-reset.raw.log
py -3.12 scripts/uart_capture.py --scan-logs C:\path\to\logs
```

上述分析命令（`--analyze-log`、`--diagnose-log`、`--compare-logs`、`--scan-logs`）全部只读，不会打开或写入串口。`py -3.12` 只是当前主机已经验证的解释器。其他主机先运行 `py -0p`，再逐个使用输出中的准确解释器路径验证 `import serial`。

## Quick Reference

| 结果 | 含义 | 下一步 |
|---|---|---|
| `NO_DATA` | 没收到字节 | 核对 COM 口、GND、TX/RX、供电和复位动作 |
| `TEXT` | 文本可可靠解码 | 从日志识别 BootROM、OpenSBI、U-Boot、内核或应用阶段 |
| `UNREADABLE` / `NON_TEXT_SIGNAL` | 收到帧但不是 UART 文本 | 保存证据，核对电平、接线、共地和启动阶段；不要扫波特率 |

## Safety Boundaries

- 默认只读，不向串口发送字符或命令。
- 不自动控制供电或复位；这些动作必须由用户执行。
- 不自动尝试多个波特率，不用试错掩盖接线或电平问题。
- 不覆盖已有日志；每次捕获使用新文件。
- 不在没有权威依据时给出板卡物理针脚位置或逻辑电平。

## Common Mistakes

- 在监听器尚未打印 `SERIAL_READY` 时让用户上电，丢失最早启动日志。
- 只看到若干字节便宣称系统启动成功。
- 忽略蓝牙虚拟串口，自动选择了错误 COM 口。
- 把 TTL 串口与 RS-232 电平混用，或在板卡独立供电时同时连接 VCC。
- 把“收到连续字节流”当成“波特率没对上”，于是反复换波特率，而不去查共地、接线和启动状态。
- 只根据最新一份日志下结论，不去用 `--scan-logs` 确认最后一次可读采集发生在什么时候。
