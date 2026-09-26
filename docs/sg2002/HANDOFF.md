# LicheeRV Nano WE / SG2002 ArceOS 详细交接文档

## 0. 后续 AI 从这里开始

本文件是项目主入口。先读取本文件，再按顺序读取：

1. `docs/sg2002/project-status.md`：阶段状态和验收门禁；
2. `docs/sg2002/session-2026-09-25.md`：环境恢复与本次 Session 证据；
3. `docs/sg2002/uart-rediagnosis-2026-09-25.md`：UART 物理层复诊；
4. `docs/sg2002/operation-log.md`：按时间追加的操作记录；
5. `docs/superpowers/plans/2026-09-23-sg2002-sta-product-closure.md`：实现与真板门禁计划。

不要先增加新功能。

> **更新（2026-09-25 04:50）：UART 物理门禁已通过。**
> 现场把 CH340 TXD/RXD 接正后，RESET 取得 `23,107` 字节 `UART_TEXT` 启动日志，证据见 §5.5。
> 下一步是直接执行 §6.3 的产品启动器；**不需要恢复 SD 卡根文件系统**（原因见 §5.6）。

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

真板 U-Boot 已确认可工作，并已进入出厂 Linux（证据见 §5.5）；但下列各项仍缺本轮 ArceOS 证据：

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

### 5.3 排查过程（历史记录，已于 04:45 结束）

- CH340 由 Windows 识别为 `COM3`；
- 04:20 只拔 RXD（USB 与 GND 不动）→ 采集 0 字节，`NO_DATA`；
- 04:25 接回后非文本信号返回；
- 04:30 USB 重新插拔 → 采集 0 字节；
- 04:33 全部接回后再次为 `NON_TEXT_SIGNAL`（1,485 字节）。

这一段的判别价值在于：**非文本信号随接线动作出现或消失**，说明它由接线状态产生，不是 SoC 的 UART 输出。当时记录的“当前接线仍需现场确认”已于 04:45 结束，见 §5.5。

### 5.4 正确的判定流程

1. 板子断电或保持安全状态后确认：`GND→GND`；
2. `CH340 TXD→板子 RX`；
3. `CH340 RXD→板子 TX`；
4. VCC 不接，开发板使用自己的供电；
5. 板子上电，做 15 秒空闲采集；
6. 若为 `NO_DATA`，不要立刻判成功或失败；保持接线，按一次 RESET，再采 30 秒；
7. 只有 `line_state=UART_TEXT` 才进入产品启动器。

> 该流程已于 2026-09-25 04:45 走通。

### 5.5 UART 链路已恢复（2026-09-25 04:45，门禁通过）

现场按 §5.4 把 CH340 的 TXD/RXD 接正后，用户按一次 RESET 取得完整启动日志：

| 项 | 值 |
| --- | --- |
| 原始日志 | `artifacts/uart/licheerv-nano-restored-boot-20260925-044448.raw.log` |
| 大小 | `23,107` 字节 |
| SHA-256 | `7c94ecc5d2d57987f875e2afeab5a56271744bae1c472a8637b17cdf67ea88f4` |
| 分类 | `line_state=UART_TEXT`、`msb_ratio=0.05`、`printable_ratio=0.94` |
| 内容标记 | `U-Boot 2021.10`、`Loading Environment`、`Starting kernel`、`Linux version 5.10.4-tag-` |
| 时间线 | `--scan-logs` 的 `LAST_READABLE_TEXT` 由 `2026-09-14 23:48:44` 推进到 `2026-09-25 04:45:41` |

**UART 物理门禁通过。** 回归窗口闭合：09-14 之后 TX/RX 被接反，09-25 起该线一直是悬空干扰；接回正确后立即恢复可读文本。

### 5.6 缺 SD 卡根文件系统不阻塞本门禁

本次启动日志同时暴露一个新情况：TF 卡只有 `mmcblk0p1`（FAT32 启动分区），没有 `mmcblk0p2` 根文件系统，出厂 Linux 因此进入 USB 大容量存储恢复循环。

**这与 ArceOS 产品门禁无关。** 已核实 `send_arceos_xmodem.py` 的引导路径：

```text
LOAD_ADDRESS = 0x8020_0000
U-Boot 提示符 → XMODEM 把镜像送进内存 → go 0x80200000
```

ArceOS 产品镜像与 AIC8800 固件全部经串口送入内存，不读 SD 卡第二分区，也不启动 Linux。

因此：**不要为了本门禁重新写入整卡镜像。** `mmcblk0p1` 与 U-Boot 当前可用，写入整卡镜像有覆盖它们的风险。缺根文件系统只影响板端 Linux 基线（GC4653 摄像头等，见 §1.2），不属于当前门禁。

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

该启动器不依赖 SD 卡根文件系统（§5.6）：ArceOS 镜像与 AIC8800 固件都经串口送入内存。

**传输耗时预估（重要）**：产品镜像 `8,212,544` 字节，XMODEM-1K 需要 `8,021` 个包，
线上约 `8,253,609` 字节，115200 8N1 理论 716 秒；加上逐包握手与 ACK 往返，
**实际约 14–18 分钟**。这段时间里终端只会有缓慢滚动的 XMODEM 进度，**不要当作卡死而中断**。

对照：09-12 板级验证成功的那次只传输了 `114,752` 字节（约 10 秒），因此大镜像的耗时是本次新增的变量。

**`go` 之后的时间预算（重要）**：跳转后 sender 逐个握手发送 5 个 AIC8800 固件文件——
板子先打印 `READY AIC_FIRMWARE <名字> <长度>`，sender 才发送该文件与 CRC32；
随后依次等待各阶段标记。各步骤超时为：

| 步骤 | 超时 |
| --- | --- |
| 每个固件文件的 `READY AIC_FIRMWARE` 等待 | 60 秒 × 5 |
| `verify_aic8800_stack` | 30 秒 |
| `verify_aic8800_rf_and_mac` | 60 秒 |
| `verify_aic8800_management` / `_me` / `_sta_interface` | 各 30 秒 |
| `verify_aic8800_scan` | 60 秒 |
| `verify_aic8800_link_up`（输凭据之后） | 30 秒 |
| `verify_aic8800_dhcp` | 150 秒 |

加上固件本体约 380 KB（约 33 秒）与凭据输入的人工等待，**`go` 之后还要数分钟**。
因此整轮从按 RESET 到出现 `MUSHROOM_WEB_URL`，预期总时长约 **20–28 分钟**。


### 6.4 产品 URL 出现后运行门禁

```powershell
py -3.12 "\\wsl.localhost\Ubuntu\home\chen\arceos-worktrees\sg2002-phone-tpu\tools\sg2002\verify_mushroom_web.py" `
  --url "http://<UART 中打印的实际地址>/" `
  --input-rgb "<精确 RGB_CHW_UINT8 文件路径>" `
  --input-meta "<精确元数据 JSON 路径>" `
  --requests 20
```

`<...>` 只能用实际日志或实际文件路径替换，不能自行填写。校验器打印 `SG2002_STA_PRODUCT_GATE_PASS` 才表示 STA 产品门禁通过。

### 6.5 SD 卡 fatload 加载方式（推荐）

**为什么换**：启动器默认走 XMODEM，而 sender 用的是**普通 XMODEM（128 字节/包）**，
不是 XMODEM-1K。产品镜像 `8,212,544` 字节需要 **64,161 个停等往返**。
检索全部历史日志中的 `## Total Size`，XMODEM 在本项目**只成功完成过两次**：
`114,752` 与 `118,848` 字节——产品镜像这个体积从未跑通。
2026-09-25 05:10 的首次尝试在 `XMODEM CRC handshake detected` 之后**没有任何进度输出即中止**。

而 U-Boot 读同一张卡的速度是 **10.7 MiB/s**：`11,757,220` 字节的 `boot.sd` 用 `1043 ms`。
所以把镜像放到卡的 FAT 分区，用 `fatload` 加载。

**步骤**

1. **把镜像放进 TF 卡的 FAT 分区**（`mmcblk0p1`）。两种做法：

   | 做法 | 说明 |
   | --- | --- |
   | 取出 TF 卡 + 读卡器 | **推荐**。不牵涉板子挂载，无冲突风险 |
   | 板子 USB 大容量存储导出 | 出厂 Linux 的 init 会把**整张卡**导出（`echo /dev/mmcblk0 > functions/mass_storage.disk0/lun.0/file`）。但 Linux 侧可能仍以只读挂载着 `mmcblk0p1`，从 Windows 写入有冲突风险 |

   建议用**短文件名**（如 `arceos.bin`），减少 U-Boot FAT 驱动处理长文件名的风险。

2. 安全弹出后把卡插回板子。

3. 以 fatload 方式运行启动器（注意 `-FatloadName` 只写卡上的文件名，不是本地路径）：

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File `
     "\wsl.localhost\Ubuntu\home\chen\arceos-worktrees\sg2002-phone-tpu\tools\sg2002\run_arceos_licheerv_nano.ps1" `
     -FatloadName arceos.bin
   ```

4. 提示后**按一次 RESET**。sender 会执行
   `fatload mmc 0 0x80200000 arceos.bin`，并**校验 U-Boot 回报的字节数等于本地镜像大小**。

5. 之后的序列与 §6.3 完全相同：`go 0x80200000` → AIC8800 固件逐个握手 →
   Wi-Fi 凭据隐藏提示 → `MUSHROOM_WEB_URL`。

**失败判读**

| 报错 | 含义 |
| --- | --- |
| `U-Boot could not read <name> from the SD card` | 文件名不对，或没放进 FAT 分区 |
| `U-Boot did not report reading <n> bytes` | 卡上文件被截断或大小与本地镜像不符 |
| 其它 | 判读同 §6.3 |

启动器仍会传入本地镜像路径，用于计算期望大小并做上述校验，因此本地构建产物必须存在。
**不要**为了本门禁写整卡镜像恢复根文件系统（见 §5.6）。

## 7. 当前验证记录

2026-09-25 本文档整理前重新执行：

| 验证 | 结果 |
| --- | --- |
| `python3 tests/test_board_uart_capture_skill.py` | 9 项通过 |
| `python3 tools/sg2002/test_verify_mushroom_web.py` | 16 项通过 |
| `test_run_arceos_licheerv_nano.ps1` | `SG2002 launcher validation PASS` |
| `git diff --check`（Skill 提示修复提交前） | 通过 |
| `--scan-logs artifacts/uart` | 7 份文件；`LAST_READABLE_TEXT = 2026-09-25 04:45:41`（`UART_TEXT`，23,107 字节） |
| `--diagnose-log` 恢复日志 | `line_state=UART_TEXT`，`msb_ratio=0.05`，`printable_ratio=0.94` |
| `git ls-remote personal` | 远端与本地一致，无未推送提交 |
| `test_send_arceos_xmodem.py` | 28 项通过（含 3 项 fatload 用例：成功、大小不符、文件不存在） |
| 启动器前置步骤预演 | 构建（`bash -lic`）exit 0；`test -s` exit 0；`import paramiko,serial,xmodem` exit 0；CH340 提取端口名 `COM3` |
| sender 的 U-Boot 提示符假设 | `soph#` 由 09-12 真板日志证实存在，见下 |
| 09-12 板级成功流程 | `aic8800-management-board-20260912.log`、`aic8800-rf-mac-board-20260912.log` 含完整链路：`soph#` → `loadx 0x80200000` → `XMODEM CRC handshake detected` → `## Total Size = 0x0001c040 = 114752 Bytes` → `go 0x80200000` → `## Starting application at 0x80200000 ...` |

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

- 新 UART 原始日志为 `UART_TEXT`；**已满足**（§5.5）
- 产品启动日志包含 AIC8800 固件、WPA2、DHCP 和 `MUSHROOM_WEB_URL`；
- `/health` 验证通过；
- 有效推理返回 JSON；
- 错误 CRC 被拒绝；
- 第二次请求成功；
- 20 次请求门禁打印 `SG2002_STA_PRODUCT_GATE_PASS`；
- 证据文件、哈希和提交号写入文档；
- Git 工作树干净，提交已推送到 `personal/codex/sg2002-phone-tpu`。

## 11. AI 操作手册（环境陷阱与可复现命令）

本节记录本次 Session 实际踩到并已解决的问题。其他 AI 按本节操作，可以避免重复摸索。
每条都对应一次真实失败，不是推测。

### 11.1 从 Windows 调用 WSL 时，单引号保护不了 `$VAR`

外层是 Windows shell，**`$VAR` 和 `$(...)` 会先在最外层展开**，`bash -lc '...'` 的单引号挡不住：

```bash
# 错误：$R 在进入 WSL 之前就被展开成空串，cp 报 cannot stat ''
wsl.exe -d Ubuntu -- bash -lc 'R=/home/chen/x; cp $R/file /dest'

# 正确：只用字面路径
wsl.exe -d Ubuntu -- bash -lc 'cp /home/chen/x/file /dest'
```

**在 `wsl.exe ... bash -lc '...'` 里不要用 shell 变量和命令替换。**

### 11.2 怎么改 WSL 工作树里的文件

文件编辑工具在 `\\wsl.localhost\...` 路径上会报 `GetFileSecurityW EIO`。两种可用方式：

1. 写到 Windows 临时目录，再拷进 WSL：
   把内容写到 `C:\Users\chen\AppData\Local\Temp\new.ext`，然后
   `wsl.exe -d Ubuntu -- bash -lc 'cp /mnt/c/Users/chen/AppData/Local/Temp/new.ext <目标>'`
2. **推荐**：写一个 Python 补丁脚本，只做精确字符串替换，并断言锚点唯一，再放进 WSL 执行。
   锚点不唯一时它会直接报错退出，不会静默改错位置。

写文件时保持 LF：`Path.write_text(text, encoding="utf-8", newline="")`。

### 11.3 Git：身份、提交、推送

WSL 工作树**没有配置 git 身份**。本次沿用该分支既有作者：

```bash
export GIT_AUTHOR_NAME=chenyongqi GIT_AUTHOR_EMAIL=3226742838@qq.com
export GIT_COMMITTER_NAME=chenyongqi GIT_COMMITTER_EMAIL=3226742838@qq.com
```

推送：**WSL 内的非交互推送会被拒绝**（`remote: No anonymous write access`）。
可用的是 Windows 凭据管理器，在 Git Bash 里执行：

```bash
cd "//wsl.localhost/Ubuntu/home/chen/arceos-worktrees/sg2002-phone-tpu"
git -c safe.directory='*' -c credential.helper=manager push personal codex/sg2002-phone-tpu
```

会打印一条 `D:\github-cli\gh.exe` 路径被反斜杠吃掉的告警，但凭据助手仍然生效，推送会成功。

提交后必须核对两项：

```bash
git status --short                              # 必须为空
git ls-remote personal codex/sg2002-phone-tpu   # 必须与本机 HEAD 一致
```

### 11.4 采集日志到底存在哪

两个目录并存，采集时必须显式指定路径并记住位置：

| 目录 | 内容 |
| --- | --- |
| `C:\Users\chen\Documents\ArcOS移植sg2002\logs` | 早期采集与历史日志 |
| `C:\Users\chen\Documents\ArcOS移植sg2002\artifacts\uart` | 04:18 之后的真板采集 |

**教训**：本次曾只扫 `logs/`，据此得出“恢复日志不存在”的错误结论，实际文件在 `artifacts/uart/`。
排查前先用 `--scan-logs` 把两个目录都扫一遍。

### 11.5 构建顺序：`defconfig` 必须在前

```
make A=<example> MYPLAT=axplat-riscv64-licheerv-nano defconfig
make A=<example> MYPLAT=axplat-riscv64-licheerv-nano APP_FEATURES=hardware build
```

漏掉 `defconfig` 直接 `build` 会报
`"ARCH" or "MYPLAT" has been changed, please run "make defconfig" again`。
删掉 `.bin` 之后重建也必须按这个顺序。

### 11.6 Python 与测试注意事项

- 已验证解释器：`py -3.12`。
- Windows 上跑 pytest 必须禁用插件自动加载，否则 `langsmith` 插件会因缺 `requests_toolbelt` 直接报错：
  `set PYTEST_DISABLE_PLUGIN_AUTOLOAD=1`
- WSL 里没有 pytest，直接用 unittest：
  ```bash
  python3 tests/test_board_uart_capture_skill.py
  python3 -m unittest discover -s tools/sg2002 -p "test_*.py"
  ```
- **WSL 的 `python3` 没有 `xmodem` 和 `paramiko`。** 所以上面这条 discover 会在
  `test_send_arceos_xmodem.py` 上报 `ModuleNotFoundError: No module named 'xmodem'`，
  结果是 **16 项通过 + 1 项 error**。这是环境差异，不是代码缺陷——不要为此改 sender 的 import。
  sender 测试必须用 Windows 的解释器跑：

  ```powershell
  set PYTEST_DISABLE_PLUGIN_AUTOLOAD=1
  py -3.12 -m pytest tools\sg2002\test_send_arceos_xmodem.py -q   # 25 项通过
  ```
- PowerShell 执行策略会拦截 `.ps1`：加 `-ExecutionPolicy Bypass`，或先
  `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`。

### 11.7 采集与诊断命令速查

```powershell
# 串口枚举（自动选择在多串口时会拒绝，必须显式 --port）
py -3.12 C:\Users\chen\.codex\skills\board-uart-capture\scripts\uart_capture.py --list-ports

# 只读采集，不向串口写任何字节
py -3.12 C:\Users\chen\.codex\skills\board-uart-capture\scripts\uart_capture.py `
  --port COM3 --baud 115200 --log <新日志路径> --duration 30

# 单份信号诊断
py -3.12 ...\uart_capture.py --diagnose-log <日志>

# 两份对比：判定是否同一条信号
py -3.12 ...\uart_capture.py --compare-logs <A> <B>

# 目录时间线：找最后一次可读采集，框定回归窗口
py -3.12 ...\uart_capture.py --scan-logs <目录>
```

判读要点：

- 真实 ASCII 文本第 8 位恒为 0，所以 `msb_ratio > 0.02` 就**不可能**是波特率错配的文本；
- 同一条信号的两次快照，`bit_profile` 的 L1 距离通常 `< 0.05`；可读文本与非文本流之间 `> 0.5`；
- `NO_DATA`（0 字节）**不等于故障**：正确接线的空闲 UART TX 也是 0 字节，必须再按一次 RESET 采集才算判据完整。

### 11.8 每次收尾的验证清单

在 WSL 里：

```bash
cd /home/chen/arceos-worktrees/sg2002-phone-tpu
python3 tests/test_board_uart_capture_skill.py                       # 9 项
python3 -m unittest discover -s tools/sg2002 -p "test_verify_*.py"   # 16 项
git diff --check                                                     # 空白检查
git status --short                                                   # 必须为空
git ls-remote personal codex/sg2002-phone-tpu                        # 与 HEAD 一致
```

在 Windows 里（WSL 缺依赖，见 §11.6）：

```powershell
set PYTEST_DISABLE_PLUGIN_AUTOLOAD=1
py -3.12 -m pytest tools\sg2002\test_send_arceos_xmodem.py -q      # 25 项
```

真板侧另外还要跑 `test_run_arceos_licheerv_nano.ps1`，预期输出 `SG2002 launcher validation PASS`。
