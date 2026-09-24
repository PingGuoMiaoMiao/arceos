# LicheeRV Nano WE / SG2002 ArceOS 项目进度

> **2026-09-25 复盘更正（本注释为追加，未改动下方原文）**
> 第 5、6 节对本轮 UART 阻塞点的定性已被实测推翻：该信号不是 UART 文本，
> 也非波特率错配或极性反接，且按 RESET 不产生任何影响。因此重跑采集与启动器
> **不会**恢复可读文本。先读同目录 `findings-2026-09-25-uart-rediagnosis.md`，
> 按其中"决定性判据"（只拔适配器 RX 线再采一次）区分物理接线问题与板子启动问题。

## 1. 最终目标

实现下面这条真板链路：

```text
手机浏览器上传图片
    → AIC8800D80 Wi-Fi（STA）
    → ArceOS HTTP 服务
    → PhoneImageEnvelopeV1 校验
    → SG2002 TPU 蘑菇检测
    → JSON 结果返回手机网页
```

目标模型固定为 `mushroom_yolov5s_cv181x_int8_sym`。板端不运行 ONNX 或 TPU-MLIR；板端加载已经转换好的 DMABUF 和 WEIGHT 资产。

## 2. 当前所处位置

当前处于“代码与主机侧验证已完成，等待真板 STA/HTTP/TPU 闭环验收”的位置。

| 阶段 | 当前状态 | 已有证据 | 仍缺证据 |
| --- | --- | --- | --- |
| 开发环境恢复 | 已完成 | Ubuntu WSL2 可启动；权威工作树可用；固定 Rust/QEMU 工具链可用 | 无 |
| LicheeRV Nano 平台与 UART | 代码已实现，物理链路未恢复 | 平台代码、构建产物、09-14 前可读真板日志 | 09-25 最新线路为 `NON_TEXT_SIGNAL`，需重新确认 RXD→板子 TX 后取得可读输出 |
| TPU MMIO 与执行路径 | 已实现到固定模型运行代码 | TPU 平台代码、DMABUF/WEIGHT 资产、多个 TPU 示例产物 | 本轮真板端推理响应证据 |
| SG2002 SDIO 与 AIC8800 | 已实现 | SDIO、芯片识别、初始化、固件加载与启动示例及构建产物 | 本轮固件启动和 WPA2 受控端口打开日志 |
| DHCP 与 STA 网络 | 已实现 | DHCP 会话接口和主机侧测试 | 本轮真板 DHCP 地址 |
| 手机上传 HTTP 服务 | 已实现 | `GET /`、`GET /health`、`POST /api/infer`；解析、超时、分段收发和服务测试 | 本轮网页访问和上传响应 |
| 自动化部署与校验 | 已实现工具 | PowerShell 启动器、Python 门禁校验器及测试 | 真板校验器通过输出 |
| STA 产品门禁 | 未通过 | 尚无本轮完整闭环证据 | URL、健康检查、有效推理、错误 CRC、复用、20 次稳定性 |
| AP-only 热点 | 未开始 | 已明确作为 STA 闭环之后的阶段 | AP 驱动、DHCP 服务和手机直连证据 |
| 蓝牙传文件 | 不在当前门禁 | 无 | 需另立规范和硬件协议证据 |
| GC4653 摄像头 | 不在当前门禁 | Linux 基线与 ArceOS 驱动仍需分别验证 | 官方 Linux 采集证据、传感器/CSI/ISP 驱动移植 |

“已实现”表示源码、测试或构建产物已存在；只有“门禁通过”才表示真板功能已由本轮日志证明。

## 3. 已落地的主要 Git 提交

按实施顺序记录：

| 提交 | 内容 |
| --- | --- |
| `babc5a2` | LicheeRV Nano 平台支持 |
| `345c209` | SG2002 SDIO 主机驱动 |
| `4a5389d` | AIC8800 WPA2 与 DHCP 路径 |
| `20ae15c` | SG2002 TPU 蘑菇推理路径 |
| `1a20e09` | 手机图片上传协议 |
| `d21ff9d` | 注册 SG2002 crates 与 examples |
| `00c83c9` | 加固 SG2002 网络与 TPU 路径 |
| `0b8db0a` | 暴露 Wi-Fi DHCP 会话 |
| `772a142` | HTTP 单连接服务 |
| `03af7cd` | 轮询 TCP 流 |
| `d901ad7` | AIC8800 上运行网页 TPU 推理服务 |
| `c76492e` | 真板部署启动器 |
| `22218ba` | 网页推理证据输出 |
| `8a1cb5f` | STA 产品回归测试 |
| `32bf10d` | 网页请求证据采集工具 |
| `f9ed830` | 自动化 SG2002 网页推理门禁 |
| `76a3290` | 可复用开发板 UART 采集 Skill |
| `627cdc1` | 混合 UART 启动日志识别修复 |
| `63358a5` | 区分非文本线路信号并给出物理层诊断 |
| `f1ca771` | 启动器按原始字节保存 UART 证据 |
| `7da9d54` | 证明 STA 校验器能拒绝错误真板证据 |
| `738757f` | 建立 UART 日志回归时间线 |
| `068c789` | 明确空闲 UART 的 `NO_DATA` 不能单独判故障 |

## 4. 现有关键文件

### 4.1 产品与平台

- `examples/mushroom-web-licheerv-nano/`
- `examples/aic8800-firmware-boot-licheerv-nano/`
- `examples/tpu-execute-licheerv-nano/`
- `platforms/axplat-riscv64-licheerv-nano/src/tpu.rs`

### 4.2 固定模型资产

- `examples/tpu-execute-licheerv-nano/mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin`
- `examples/tpu-execute-licheerv-nano/mushroom_yolov5s_weight.bin`

### 4.3 部署与验收

- `tools/sg2002/run_arceos_licheerv_nano.ps1`
- `tools/sg2002/test_run_arceos_licheerv_nano.ps1`
- `tools/sg2002/verify_mushroom_web.py`
- `tools/sg2002/test_verify_mushroom_web.py`
- `docs/superpowers/plans/2026-09-23-sg2002-sta-product-closure.md`

### 4.4 UART 复用能力

- `codex-skills/board-uart-capture/`
- 本机安装位置：`C:\Users\chen\.codex\skills\board-uart-capture`

## 5. 当前失败点

`licheerv-nano-full-boot-20260925-021046.raw.log` 收到 `23,326` 字节，升级后的诊断结果是 `NON_TEXT_SIGNAL`。项目复诊 Session 进一步确认：用户曾把 CH340 TXD/RXD 接反，使 CH340 RXD 落在板子 RX 输入脚上并拾取干扰。改线后曾出现 0 字节空闲状态，但重新插回 CH340 后，最新 `check-033212.raw.log` 又收到 `1,503` 字节相同的非文本信号。

当前不能确认：

- U-Boot 是否进入预期加载流程；
- ArceOS 是否被跳转执行；
- AIC8800 固件是否启动；
- WPA2 是否打开受控端口；
- DHCP 是否取得地址；
- HTTP 服务是否打印 `MUSHROOM_WEB_URL`。

因此，当前阻塞点不是继续编写 TPU 功能，而是把 CH340 RXD 确实接到板子 TX，取得 RESET 后的 `UART_TEXT`，再执行已有真板门禁。COM3 当前在线不等于信号线连接正确。

## 6. 下一验收门禁

### 6.1 先恢复 UART

先确认三线交叉连接：`GND→GND`、`CH340 TXD→板子 RX`、`CH340 RXD→板子 TX`，VCC 不接。空闲采集出现 0 字节只能说明没有接收到起始位，随后必须按一次 RESET 再采集。只有得到 `line_state=UART_TEXT` 并保存原始日志，才能认为链路恢复；在此之前不对启动成功作结论。

### 6.2 启动 ArceOS 产品镜像

启动器固定构建：

```text
examples/mushroom-web-licheerv-nano
```

平台固定为：

```text
axplat-riscv64-licheerv-nano
```

硬件特性固定为：

```text
APP_FEATURES=hardware
```

Wi-Fi SSID 和密码必须由启动器在运行时交互读取，不写入文档或 Git。

### 6.3 收集真板日志

日志必须包含以下逐项证据：

1. AIC8800 固件启动；
2. WPA2 controlled-port 打开；
3. DHCP 地址；
4. `MUSHROOM_WEB_URL http://<实际地址>/`。

### 6.4 执行 HTTP/TPU 校验

使用 `tools/sg2002/verify_mushroom_web.py` 验证：

1. `GET /health` 返回 Wi-Fi `DhcpBound`、TPU `Ready` 和固定模型名；
2. 一次正确图片上传返回推理 JSON；
3. 一次错误 CRC 在调用 TPU 前被拒绝；
4. 第二次正确上传成功，证明监听器可以复用；
5. 最终执行 20 次正确请求，要求 request ID 连续、输入 CRC 不变、每次都有响应且服务不崩溃。

校验器只有打印精确标记 `SG2002_STA_PRODUCT_GATE_PASS` 时，STA 产品门禁才算通过。

## 7. 完成 STA 后的顺序

1. 记录完整 UART 与 HTTP 证据、哈希和提交号。
2. 更新产品状态，明确 STA 门禁通过。
3. 再开始 AP-only：由开发板创建热点，让手机直连。
4. 蓝牙文件传输与 GC4653 摄像头分别建立独立规范，不与 STA 门禁混合。

## 8. 数据恢复与仓库保护

- 当前开发工作继续使用 `/home/chen/arceos-worktrees/sg2002-phone-tpu`。
- 不把 `D:\Recovered_WSL\f831978898.vhdx` 直接注册为当前开发环境，也不删除它；先只读核验内容、生成哈希并建立第二份备份。
- 不在外层脏工作区批量提交；每次只从权威分支提交本任务明确修改的文件。
- 每个任务结束执行验证、创建 Git 提交并推送到 `personal/codex/sg2002-phone-tpu`。
