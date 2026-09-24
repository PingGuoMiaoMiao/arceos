# LicheeRV Nano WE / SG2002 ArceOS 详细交接文档

## 0. 后续 AI 从这里开始

本文件是项目主入口。先读取本文件，再按顺序读取：

1. `docs/sg2002/project-status.md`：阶段状态和验收门禁；
2. `docs/sg2002/session-2026-09-25.md`：环境恢复与本次 Session 证据；
3. `docs/sg2002/uart-rediagnosis-2026-09-25.md`：UART 物理层复诊；
4. `docs/sg2002/operation-log.md`：按时间追加的操作记录；
5. `docs/superpowers/plans/2026-09-23-sg2002-sta-product-closure.md`：实现与真板门禁计划。

不要先增加新功能。当前唯一前置门禁是恢复 UART 可读文本，然后执行已经存在的 STA/HTTP/TPU 真板验收。

## 1. 项目目标与范围

### 1.1 当前产品目标

```text
手机浏览器上传图片
  → AIC8800D80 连接 WPA2 Wi-Fi
  → DHCP 获得 IPv4 地址
  → ArceOS TCP/HTTP 服务
  → 校验 PhoneImageEnvelopeV1
  → SG2002 TPU 执行固定蘑菇模型
  → 返回 JSON 检测结果
```

固定模型名为 `mushroom_yolov5s_cv181x_int8_sym`。板端只加载已转换的 DMABUF 和 WEIGHT，不在板端运行 ONNX 或 TPU-MLIR。

### 1.2 当前不混入的工作

- AP-only 热点在 STA 真板闭环后开始；
- 蓝牙文件传输另立规范；
- GC4653 摄像头先在官方 Linux 验证，再单独移植；
- 不把网络、摄像头、蓝牙问题混入当前 UART 物理门禁。

## 2. 权威位置与仓库规则

| 项目 | 精确值 |
| --- | --- |
| WSL 发行版 | `Ubuntu`，WSL2 |
| 权威工作树 | `/home/chen/arceos-worktrees/sg2002-phone-tpu` |
| Windows 访问路径 | `\\wsl.localhost\Ubuntu\home\chen\arceos-worktrees\sg2002-phone-tpu` |
| Git 分支 | `codex/sg2002-phone-tpu` |
| 推送远端 | `personal` |
| GitHub | `https://github.com/PingGuoMiaoMiao/arceos.git` |
| 外层证据目录 | `C:\Users\chen\Documents\ArcOS移植sg2002` |
| UART 日志目录 | `C:\Users\chen\Documents\ArcOS移植sg2002\logs` |

规则：

- 权威源码只从 WSL 工作树提交；Windows 外层目录保存启动工具、日志和临时证据，当前不是干净的权威源码仓库。
- 每个任务都要有新验证、Git 提交和推送。
- 不批量暂存无关文件。
- Wi-Fi SSID 和密码只允许运行时交互输入，不写入源码、命令行、日志或 Git。
- 构建通过不是上板通过；真板状态只由 UART 与 HTTP 证据确认。

## 3. 当前实现结构

### 3.1 板级平台

- `platforms/axplat-riscv64-licheerv-nano/`
- 负责 SG2002 内存布局、UART、TPU MMIO 与板级启动边界。

### 3.2 网络链路

- `modules/axdriver_sg2002_sdio/`：SG2002 SDIO 主机侧；
- `modules/axdriver_aic8800/`：AIC8800D80 固件、WPA2 与数据通路；
- `examples/aic8800-firmware-boot-licheerv-nano/`：固件启动和 DHCP 会话；
- `examples/mushroom-web-licheerv-nano/`：最终 HTTP 产品应用。

### 3.3 TPU 链路

- `platforms/axplat-riscv64-licheerv-nano/src/tpu.rs`；
- `examples/tpu-execute-licheerv-nano/`；
- `modules/axmodel_mushroom_yolov5/`；
- `mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin`；
- `mushroom_yolov5s_weight.bin`。

### 3.4 部署与门禁

- `tools/sg2002/run_arceos_licheerv_nano.ps1`：构建、传输、启动、运行时输入 Wi-Fi 凭据、保存 UART 证据；
- `tools/sg2002/verify_mushroom_web.py`：检查健康状态、有效推理、错误 CRC 拒绝和连续请求；
- `tools/sg2002/test_run_arceos_licheerv_nano.ps1`；
- `tools/sg2002/test_verify_mushroom_web.py`。

HTTP 契约是：

- `GET /`
- `GET /health`
- `POST /api/infer`
- 内容类型：`application/vnd.arceos.rgb-u8`
- 请求总长度：`1,228,840` 字节，其中头部 `40` 字节，RGB_U8 CHW 载荷 `1,228,800` 字节。

## 4. 已实现与未证明的边界

### 4.1 代码与主机侧已实现

- LicheeRV Nano 平台与示例注册；
- TPU MMIO、内存布局、DMABUF/WEIGHT 资产与固定模型执行接口；
- SG2002 SDIO 与 AIC8800 初始化、固件、WPA2、DHCP 代码路径；
- 顺序 TCP 连接、HTTP 解析、上传协议、超时和部分收发处理；
- `/health`、推理响应和错误 CRC 处理；
- PowerShell 部署启动器与 Python 门禁校验器；
- 可复用 `board-uart-capture` Skill。

### 4.2 当前仍未由真板证明

- 本轮 ArceOS 镜像确实由 U-Boot 跳转；
- AIC8800 固件启动；
- WPA2 controlled-port 打开；
- DHCP 地址；
- `MUSHROOM_WEB_URL`；
- `/health` 返回 TPU Ready；
- 有效图片返回推理 JSON；
- 错误 CRC 在 TPU 前被拒绝；
- 第二次请求和 20 次连续请求稳定性。

## 5. UART 阻塞点的完整结论

### 5.1 回归窗口

- 最后一份可读真板日志：`aic8800-link-up-board-20260914.log`，时间 2026-09-14 23:48。
- 2026-09-25 01:46 起，多份采集变为稳定 `NON_TEXT_SIGNAL`。
- 09-25 02:15 的 23,326 字节日志不是可读 UART 文本。

### 5.2 已确认的历史原因

用户在项目复诊 Session 中确认 CH340 的 TXD/RXD 原来接反。此时 CH340 RXD 落在板子 RX 输入脚上，接收端没有发送源，采到的是稳定可复现的干扰，而不是 SoC UART 输出。

### 5.3 最新物理状态

- CH340 当前由 Windows 识别为 `COM3`；
- 改线后曾连续出现三份 0 字节日志；
- 重新插回适配器后，噪声重新出现；
- `check-033212.raw.log` 为 1,503 字节 `NON_TEXT_SIGNAL`；
- 2026-09-25 04:05 的最新空闲采集 `idle-20260925-040507.raw.log` 为 800 字节，`msb_ratio=0.624`，仍是 `NON_TEXT_SIGNAL`。

所以不能把“历史接反原因已找到”写成“当前链路已恢复”。当前接线仍需要现场重新确认。

### 5.4 正确的判定流程

1. 板子断电或保持安全状态后确认：`GND→GND`；
2. `CH340 TXD→板子 RX`；
3. `CH340 RXD→板子 TX`；
4. VCC 不接，开发板使用自己的供电；
5. 板子上电，做 15 秒空闲采集；
6. 若为 `NO_DATA`，不要立刻判成功或失败；保持接线，按一次 RESET，再采 30 秒；
7. 只有 `line_state=UART_TEXT` 才进入产品启动器。

## 6. 精确执行命令

### 6.1 枚举串口

```powershell
py -3.12 C:\Users\chen\.codex\skills\board-uart-capture\scripts\uart_capture.py --list-ports
```

### 6.2 采集 RESET 日志

```powershell
py -3.12 C:\Users\chen\.codex\skills\board-uart-capture\scripts\uart_capture.py `
  --port COM3 `
  --baud 115200 `
  --log C:\Users\chen\Documents\ArcOS移植sg2002\logs\uart-reset-new.raw.log `
  --duration 30
```

启动命令后，在 30 秒窗口内只按一次 RESET。

### 6.3 UART 恢复后启动产品镜像

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File `
  "\\wsl.localhost\Ubuntu\home\chen\arceos-worktrees\sg2002-phone-tpu\tools\sg2002\run_arceos_licheerv_nano.ps1"
```

启动器提示 `Press RESET once` 后只按一次 RESET。Wi-Fi 凭据在隐藏提示中输入。

### 6.4 产品 URL 出现后运行门禁

```powershell
py -3.12 "\\wsl.localhost\Ubuntu\home\chen\arceos-worktrees\sg2002-phone-tpu\tools\sg2002\verify_mushroom_web.py" `
  --url "http://<UART 中打印的实际地址>/" `
  --input-rgb "<精确 RGB_CHW_UINT8 文件路径>" `
  --input-meta "<精确元数据 JSON 路径>" `
  --requests 20
```

`<...>` 只能用实际日志或实际文件路径替换，不能自行填写。校验器打印 `SG2002_STA_PRODUCT_GATE_PASS` 才表示 STA 产品门禁通过。

## 7. 当前验证记录

2026-09-25 本文档整理前重新执行：

| 验证 | 结果 |
| --- | --- |
| `python3 tests/test_board_uart_capture_skill.py` | 9 项通过 |
| `python3 tools/sg2002/test_verify_mushroom_web.py` | 16 项通过 |
| `test_run_arceos_licheerv_nano.ps1` | `SG2002 launcher validation PASS` |
| `git diff --check`（Skill 提示修复提交前） | 通过 |

项目复诊 Session 还记录了 179 项 Cargo 测试、15 个构建目标和完整产品回归通过；这些属于该 Session 的既有证据，不替代下一次改代码后的重新验证，也不替代真板门禁。

## 8. 数据恢复状态

- 当前 Ubuntu 已正常工作，注册位置在 C 盘应用包目录；
- `D:\Recovered_WSL\f831978898.vhdx` 存在，大小 `5,955,824,640` 字节；
- `E:\WSL\Ubuntu-22.04` 当前不存在；
- 不删除或注册恢复出的 VHDX，直到完成只读内容核验、哈希记录和第二份备份。

## 9. 原始 Session 证据

| 文件 | SHA-256 | 用途 |
| --- | --- | --- |
| `C:\Users\chen\Downloads\dsh-session-session-2aadc18f-8888-44df-a803-a12c9bcce2b4.zip` | `C33BD9289C5FEB361468C3FA35AF99276F2A717DA2AAEAE1C4ED6A5352821320` | Session 记录结构参考 |
| `C:\Users\chen\Downloads\dsh-session-session-c7598b32-8049-4c8f-8c03-656055a82b4f.zip` | `D30539F5391D26BE2393E23D2EB1E7E8FD309423D415D1B0B0E55BE76D19049A` | 本项目 UART 复诊、测试与本地提交过程 |

保留 ZIP 原件；交接文档只摘录与项目状态有关的可验证事实。

## 10. 完成定义

当前阶段只有同时得到以下证据才能结束：

- 新 UART 原始日志为 `UART_TEXT`；
- 产品启动日志包含 AIC8800 固件、WPA2、DHCP 和 `MUSHROOM_WEB_URL`；
- `/health` 验证通过；
- 有效推理返回 JSON；
- 错误 CRC 被拒绝；
- 第二次请求成功；
- 20 次请求门禁打印 `SG2002_STA_PRODUCT_GATE_PASS`；
- 证据文件、哈希和提交号写入文档；
- Git 工作树干净，提交已推送到 `personal/codex/sg2002-phone-tpu`。
