# SG2002 项目操作日志

本文件按时间追加，不覆盖旧记录。每条记录使用“观察 → 判断依据 → 动作 → 结果 → 下一步”。不记录隐藏的逐字思维过程，不记录 Wi-Fi 密码或其他凭据。

## 记录模板

```text
### YYYY-MM-DD HH:mm:ss +08:00 — 标题
- 观察：原始状态或输入。
- 判断依据：采用的日志、文件、命令输出或规范。
- 动作：实际执行的精确操作。
- 结果：退出码、计数、哈希、提交号或失败文本。
- 下一步：唯一的后续动作或明确门禁。
```

## 2026-09-25

### 02:09:52 +08:00 — UART Skill 混合日志修复

- 观察：历史启动日志同时含非文本前缀和可读 U-Boot/Linux 文本。
- 判断依据：只看文件开头会把整份可用日志误判为不可读。
- 动作：修复混合日志识别并提交。
- 结果：提交 `627cdc1 fix(skill): recognize mixed UART boot logs`。
- 下一步：重新采集当前板子的完整启动日志。

### 02:10–02:15 +08:00 — 当前全启动采集

- 观察：COM3 持续收到字节。
- 判断依据：原始文件 `licheerv-nano-full-boot-20260925-021046.raw.log`。
- 动作：以 115200 波特率采集完整窗口。
- 结果：23,326 字节；SHA-256 `4A97C0438A8965A0C0265D8B6EC323E5A303D6433E646DD0B1547AFD9F49AF7C`；没有可读 U-Boot/Linux/ArceOS 标记。
- 下一步：分析信号本身，不能把收到字节等同于启动成功。

### 03:06:20 +08:00 — UART 非文本信号诊断与原始字节保存

- 观察：多份 09-25 日志表现为稳定但不可读的字节流。
- 判断依据：位指纹、可打印比例、RESET 前后对比；启动脚本的 `ReadExisting()` 会损坏大于等于 `0x80` 的原始字节。
- 动作：增加非文本信号诊断；把启动器证据保存改成原始字节。
- 结果：提交 `63358a5` 和 `f1ca771`。
- 下一步：强化 STA 门禁的失败路径并建立回归时间线。

### 03:19:19 +08:00 — STA 门禁负向验证

- 观察：通过路径测试不能证明错误真板证据一定被拒绝。
- 判断依据：门禁必须对缺失或错误证据失败。
- 动作：增加负向测试。
- 结果：提交 `7da9d54 test(tools): prove the STA gate actually rejects a bad board`。
- 下一步：定位 UART 从可读到不可读的时间窗口。

### 03:25:41 +08:00 — UART 回归时间线

- 观察：09-14 有可读日志，09-25 变成非文本信号。
- 判断依据：扫描 `logs/` 后最后可读真板日志为 2026-09-14 23:48。
- 动作：为 Skill 增加日志时间线与最后可读点输出。
- 结果：提交 `738757f feat(skill): frame the regression window with a log timeline`。
- 下一步：通过物理接线变化区分板端输出与悬空干扰。

### 03:28–03:30 +08:00 — TX/RX 接反得到确认

- 观察：用户确认 CH340 TXD/RXD 原来接反；改线后采集从稳定噪声变为 0 字节。
- 判断依据：CH340 RXD 原来落在板子 RX 输入脚，没有发送源；0 字节与正常 UART TX 空闲高电平一致。
- 动作：记录接线原因，并在保持接线时要求按一次 RESET 采集。
- 结果：三份空闲或 RESET 采集为 0 字节；0 字节不能单独证明链路成功。
- 下一步：保持正确接线并取得 RESET 后的 `UART_TEXT`。

### 03:31–03:32 +08:00 — 重新插回后非文本信号返回

- 观察：CH340 重新枚举为 COM3 后，非文本字节流重新出现。
- 判断依据：`replugged-033115.raw.log` 为 3,744 字节；`check-033212.raw.log` 为 1,503 字节，信号指纹与此前噪声一致。
- 动作：停止继续尝试波特率，重新把问题收敛到 RXD→板子 TX 的现场接线。
- 结果：当前 UART 门禁仍未通过。
- 下一步：重新确认三线交叉连接；空闲采集后只按一次 RESET，再采 30 秒。

### 03:55:43 +08:00 — Session 证据合并与当前状态复核

- 观察：收到第二个 Session ZIP；权威工作树相对远端领先 4 个提交，另有 Skill 提示修复和文档未提交；CH340 当前为 COM3。
- 判断依据：解析 `session-c7598b32-8049-4c8f-8c03-656055a82b4f` 的 7,388 条事件；检查 `git status`、Git 日志、Windows PnP 串口和最新日志目录。
- 动作：重新运行 Skill、门禁校验器和启动器验证；修正 `NO_DATA` 提示；生成详细交接、项目状态、Session 与操作日志。
- 结果：Skill 测试 9 项通过；校验器测试 16 项通过；启动器验证输出 `SG2002 launcher validation PASS`；Skill 修复提交为 `068c789`。
- 下一步：校验文档、提交并推送；真板继续等待正确接线后的 `UART_TEXT`。

### 04:03:31 +08:00 — 交接文档发布

- 观察：五份文档已生成，但提交前必须验证内部路径、敏感信息和远端状态。
- 判断依据：逐项检查文档引用文件存在；搜索已知明文凭据和禁止词；读取远端分支引用。
- 动作：执行 UTF-8/空字节检查、`git diff --cached --check`，提交五份文档并通过 Windows GitHub 凭据推送。
- 结果：提交 `4377112 docs: add SG2002 session and project handoff` 已推送到 `personal/codex/sg2002-phone-tpu`；远端在推送后指向该提交。
- 下一步：重新运行最终验证并确认工作树与远端一致；随后等待正确接线后的 RESET 操作。

### 04:05:07 +08:00 — 当前线路空闲采集

- 观察：Windows 枚举 `USB-SERIAL CH340 (COM3)`，需要确认重新插线后的实际信号状态。
- 判断依据：空闲采集不会向串口写数据，可在不触发启动流程的情况下判断线路是否仍有非文本信号。
- 动作：在 COM3、115200 波特率下只读采集 15 秒，保存为 `logs/idle-20260925-040507.raw.log`。
- 结果：收到 800 字节；`line_state=NON_TEXT_SIGNAL`；`printable_ratio=0.326`；`msb_ratio=0.624`；`ascii_run_max=4`；`bit_profile=b0=0.604 b1=0.405 b2=0.422 b3=0.391 b4=0.464 b5=0.519 b6=0.455 b7=0.624`。与此前悬空干扰特征一致，当前不能运行产品启动器。
- 下一步：只拔掉 CH340 RXD 信号线，USB 与 GND 保持不动，再采一次；根据信号是否消失区分接收端悬空与板端驱动。

### 04:06:42 +08:00 — RXD 分支动作复查

- 观察：尚未收到用户确认，先只读复查线路是否已经发生物理变化。
- 判断依据：如果 CH340 RXD 已从当前信号线上拔下，采集结果应与 04:05 的稳定非文本流产生明确差异。
- 动作：在 COM3、115200 波特率下只读采集 10 秒，保存为 `logs/rx-branch-probe-20260925-040642.raw.log`。
- 结果：收到 753 字节；`line_state=NON_TEXT_SIGNAL`；`printable_ratio=0.325`；`msb_ratio=0.612`；`ascii_run_max=4`；`bit_profile=b0=0.618 b1=0.421 b2=0.410 b3=0.405 b4=0.474 b5=0.499 b6=0.433 b7=0.612`。与 04:05 的信号一致，所需物理分支动作尚未发生。
- 下一步：等待用户只拔掉 CH340 RXD 信号线并明确回复；USB 与 GND 保持不动，不按 RESET。

### 04:07:59 +08:00 — RXD 分支最终自动复核

- 观察：连续两轮仍未收到用户完成物理动作的确认；执行最后一次短时只读复核，避免把旧状态误作当前状态。
- 判断依据：没有拔线后的明确对照，就不能把字节速率变化解释为接线恢复。
- 动作：在 COM3、115200 波特率下只读采集 5 秒，保存为 `logs/rx-branch-final-20260925-040759.raw.log`。
- 结果：收到 19 字节；`line_state=NON_TEXT_SIGNAL`；`printable_ratio=0.211`；`msb_ratio=0.789`；`ascii_run_max=1`。连续三轮都未得到所需物理分支证据。
- 停止条件：停止自动重复采集。项目等待用户只拔掉 CH340 RXD 信号线并回复“已拔 RXD”；USB 与 GND 保持不动，不按 RESET。收到确认后再进行一次对照采集。

### 04:18–04:45 +08:00 — UART 物理门禁通过

- 观察：CH340 重新枚举为 `COM3` 后线路仍为非文本信号，需要现场把 RXD 接回板子 TX。
- 判断依据：`artifacts/uart/` 下逐次采集的对照；非文本信号随接线动作出现或消失。
- 动作：按判定流程现场改线（`GND→GND`、`TXD→板子 RX`、`RXD→板子 TX`、VCC 不接），随后按一次 RESET 采集。
- 结果：`artifacts/uart/licheerv-nano-restored-boot-20260925-044448.raw.log` 收到 `23,107` 字节，
  `line_state=UART_TEXT`、`msb_ratio=0.05`、`printable_ratio=0.94`，含 `U-Boot 2021.10`、`Loading Environment`、
  `Starting kernel`、`Linux version 5.10.4-tag-`。`--scan-logs` 的 `LAST_READABLE_TEXT` 由 `2026-09-14 23:48:44`
  推进到 `2026-09-25 04:45:41`；日志 SHA-256
  `7c94ecc5d2d57987f875e2afeab5a56271744bae1c472a8637b17cdf67ea88f4`。UART 物理门禁通过。
- 中间对照：04:20 只拔 RXD → 0 字节 `NO_DATA`；04:25 接回 → 非文本信号返回；04:30 USB 重新插拔 → 0 字节；
  04:33 全部接回 → `NON_TEXT_SIGNAL`（1,485 字节）。
- 下一步：执行 `tools/sg2002/run_arceos_licheerv_nano.ps1` 启动产品镜像。

### 04:45–04:50 +08:00 — 缺根文件系统与门禁范围界定

- 观察：上述启动日志显示 TF 卡只有 `mmcblk0p1`，缺 `mmcblk0p2`；出厂 Linux 进入 USB 大容量存储恢复循环。
  现场曾准备下载 Sipeed 官方整卡镜像（发布标签 `20260114`，提交 `d4003f`）用于恢复第 2 分区。
- 判断依据：核对 `send_arceos_xmodem.py`：`LOAD_ADDRESS = 0x8020_0000`，流程为等待 U-Boot 提示符 →
  XMODEM 把镜像写入内存 → `go 0x80200000`；AIC8800 固件同样由启动器经串口送入。
- 动作：确认产品门禁不依赖 SD 卡根文件系统，暂停整卡写入，保留已下载的镜像包备用。
- 结果：认定缺根文件系统只影响板端 Linux 基线（GC4653 摄像头等，见 `HANDOFF.md` §1.2），
  不属于 STA 产品门禁；不覆盖当前可用的 `mmcblk0p1` 与 U-Boot。
- 下一步：直接执行产品启动器；取得 `MUSHROOM_WEB_URL` 后运行 `verify_mushroom_web.py`。

### 05:05 +08:00 — 交接文档补全与实时记录约定

- 观察：用户要求交接文档必须让其他 AI 直接看懂，并要求实时记录思考依据与对应操作。
  用户同时贴出一份项目摘要，其中称“当前阻塞在 UART 物理链路、最新 `check-033212.raw.log` 仍为
  1,503 字节 `NON_TEXT_SIGNAL`、最新提交 `b81e17c`”。
- 判断依据：核对 `git rev-parse --short HEAD`、`git log`、`git ls-remote personal` 与
  `--scan-logs artifacts/uart`。该摘要落后 4 个提交（实际 HEAD 为 `bfd2c25`），
  且 UART 已于 04:45 恢复（`licheerv-nano-restored-boot-20260925-044448.raw.log`，23,107 字节 `UART_TEXT`）。
- 动作：在 `HANDOFF.md` 增补 §11「AI 操作手册」，把本次实际踩过的环境问题写成可复现条目
  （WSL 变量展开陷阱、WSL 文件编辑方式、git 身份与凭据推送、日志目录约定、`defconfig` 顺序、
  Python/pytest 注意事项、采集诊断命令速查、收尾验证清单）；修正 §7 中过时的远端提交号。
- 结果：文档更新与本条同批提交；`git status` 干净，已推送到 `personal/codex/sg2002-phone-tpu`。
- 下一步：执行 `tools/sg2002/run_arceos_licheerv_nano.ps1` 启动产品镜像；出现 `MUSHROOM_WEB_URL`
  后运行 `verify_mushroom_web.py`。**在此之前不要写整卡镜像恢复 `mmcblk0p2`。**

### 05:12 +08:00 — 修复启动器依赖缺失（sender 从未进入权威仓库）

- 观察：用户在 PowerShell 执行文档给出的启动器命令，立即报错
  `XMODEM sender was not found. Pass its exact path with -SenderPath: ...\tools\sg2002\send_arceos_xmodem.py`。
- 判断依据：`ls tools/sg2002/` 与 `git log --all -- "*send_arceos_xmodem.py"` 确认该文件**从未被纳入权威仓库**。
  它只存在于外层工作区 `C:\Users\chen\Documents\ArcOS移植sg2002\send_arceos_xmodem.py`，
  而外层目录是一个**没有配置任何 remote 的本地 `master` 仓库**，因此权威工作树取不到这个工具。
  另外 `send_arceos_xmodem.py` 的默认固件目录是 `<sender 同级>/.local-firmware`，仓库内同样没有该目录。
- 动作：
  1. 把 `send_arceos_xmodem.py`（23,761 字节）与其单元测试 `test_send_arceos_xmodem.py`（11,689 字节）
     从外层复刻进 `tools/sg2002/`，保持 LF 换行；
  2. 把 AIC8800 固件 5 个文件复刻到 `tools/sg2002/.local-firmware/`。该目录已被
     `.gitignore:17 '`.local-firmware/`'` 忽略，符合仓库“绝不提交本地专有固件”的既定策略；
  3. 复刻启动器自身的固件清单校验，并运行 sender 单元测试。
- 结果：启动器前置校验（sender + 固件）**全部通过**——5 个固件文件长度与 SHA-256 全部匹配；
  `test_send_arceos_xmodem.py` **25 项通过**。`git status` 只显示两个新增源码文件，固件未进入版本控制。
  提交 `166c108 tools: add the XMODEM sender the launcher already depends on`。
- 下一步：由用户在**自己的 PowerShell** 里执行启动器命令。Wi-Fi 凭据是隐藏交互输入，
  只能在用户终端里输入，无法由 Agent 代跑；提示 `Press RESET once` 时按一次 RESET。

### 05:15 +08:00 — 修复启动器不跑 defconfig 的自洽性缺陷

- 观察：修好 sender 缺失后继续预检，发现 `.axconfig.toml` 仍指向 `riscv64-qemu-virt`
  （此前跑产品回归构建了 qemu 示例留下），而启动器的构建命令只有 `make build`。
- 判断依据：直接执行启动器用的那条命令，得到
  `Makefile:181: *** "ARCH" or "MYPLAT" has been changed, please run "make defconfig" again.  Stop.`
  说明启动器依赖工作树"碰巧"已配置成目标平台，不自洽。
- 动作：把启动器的构建命令改为先 `defconfig` 再 `build`，顺序与
  `tools/sg2002/test_product_regression.sh` 既有做法一致；并在
  `test_run_arceos_licheerv_nano.ps1` 增加"必须包含 defconfig"的回归守卫。
- 结果：修复后的完整命令构建成功，产出 8,212,544 字节产品镜像；
  启动器测试仍为 `SG2002 launcher validation PASS`。
  提交 `a998957 fix(tools): configure the worktree before the launcher builds it`。
- 下一步：用户在自己终端执行启动器；提示 `Press RESET once` 时按一次 RESET，隐藏提示里输入 Wi-Fi 凭据。

### 05:20 +08:00 — 启动器全链路预检（含 U-Boot 提示符与大镜像耗时）

- 观察：sender 与 defconfig 两个缺陷修复后，需要在用户运行前把剩余风险提前打掉。
- 判断依据：
  1. 逐步复演启动器的四条前置步骤；
  2. sender 的 U-Boot 检测是 `read_until(b"U-Boot 2021.10")` → 发空格打断 autoboot →
     `read_until(b"soph#", 15)`，因此提示符字符串必须与真板一致；
  3. 产品镜像是 `8,212,544` 字节，而 09-12 板级成功的那次只传了 `114,752` 字节。
- 动作：预演四条前置步骤；在全部历史日志中检索 `soph#`；计算 XMODEM-1K 的理论与实际耗时。
- 结果：
  - 前置步骤全部通过：构建（`bash -lic`）exit 0；`test -s` exit 0；
    `import paramiko,serial,xmodem` exit 0；CH340 提取端口名 `COM3`。
  - `soph#` 在 `logs/aic8800-management-board-20260912.log` 与
    `logs/aic8800-rf-mac-board-20260912.log` 中真实存在，且这两份日志完整记录了成功流程：
    `Hit any key to stop autoboot` → `soph# U-Boot prompt detected` → `loadx 0x80200000` →
    `XMODEM CRC handshake detected` → `## Total Size = 0x0001c040 = 114752 Bytes` →
    `go 0x80200000` → `## Starting application at 0x80200000 ...`。
    即 sender 的提示符假设与 XMODEM 加载路径**已有板级证据**。
  - 大镜像耗时：8,021 包 × 1,029 字节 = 8,253,609 字节，115200 8N1 理论 716 秒，
    加握手/ACK 往返实际约 **14–18 分钟**。
- 下一步：用户在自己终端执行启动器；传镜像期间不要中断。出现 `MUSHROOM_WEB_URL` 后跑
  `verify_mushroom_web.py`。若 `read_until(b"soph#")` 超时，说明空格没有及时打断 autoboot，
  应把打断动作提前或增大 autoboot 延时——这是当前唯一未在本轮真板复现的环节。

### 05:10:29 +08:00 — 用户首次把启动器推进到 XMODEM 传输

- 观察：用户在 PowerShell 执行启动器，产出
  `tools/sg2002/logs/mushroom-web-sta-board-20260925-051029.log`（24,656 字节，UTF-16LE 编码）。
- 判断依据：解码该日志得到完整阶段序列——
  `sending ...\mushroom-web-licheerv-nano_riscv64-licheerv-nano.bin (8212544 bytes) to 0x80200000`
  → `waiting for U-Boot; reset the board now`（此时板子仍在出厂 Linux 恢复循环里输出 busybox iostat）
  → 第 590 行 `U-Boot 2021.10 (Jan 22 2026 - 14:08:22 +0800) soph`（用户按了 RESET）
  → 第 616 行 `soph# U-Boot prompt detected`
  → 第 617 行 `starting U-Boot loadx`
  → 第 618 行 `XMODEM CRC handshake detected`。
- 动作：**不触碰 COM3**（sender 正持有端口）。改用非侵入方式判定活性：进程 CPU 计数变化、
  以及尝试打开 COM3 看是否被占用。
- 结果：
  - PID 40712（`python`，sender）CPU 由 `2.28s` 增至 `2.50s`（间隔 8 秒采样）；
  - PID 42124（`py` 包装进程）CPU 静止在 `0.02s`，在等待；
  - 打开 `COM3` 抛 `Access to the port 'COM3' is denied`，确认端口被 sender 持有。
  → 判定 **XMODEM 传输正在进行中**。日志在 05:10:48 之后不再增长，是 Python stdout 经
  `Tee-Object` 管道时采用块缓冲所致，**不是卡死**。
  按 §6.3 的估算（8,021 个包），预计 `05:25`–`05:29` 完成传输。
- 下一步：等待传输完成。完成后 sender 会先送 AIC8800 固件，再在隐藏提示里要求输入
  Wi-Fi SSID 与密码——**必须在该终端内输入**。之后等待 `MUSHROOM_WEB_URL`。
  传输期间不要拔线、不要按 RESET、不要另开串口监视器（会抢 COM3）。

### 05:13 +08:00 — 传输期间离线预检门禁输入与 HTTP 契约

- 观察：XMODEM 传输仍在进行（PID 40712 CPU 持续增长），不能触碰 COM3。等待期间可做的事是
  把产品门禁的输入与契约提前验通，避免板子起来后卡在低层错误上。
- 判断依据与动作：
  1. 用 `verify_mushroom_web.py` 自身的 `load_metadata` / `build_envelope` 校验两份候选输入；
  2. 审查 `examples/mushroom-web-licheerv-nano/src/service.rs` 的 `handle_inference` 执行顺序，
     确认"故意送错 CRC 的那次请求"是否会占用 `request_id`；
  3. 比对板端与校验器的 `MODEL_NAME`、信封常量与 `/health` 键集。
- 结果：
  - **门禁输入可用**：
    `mushroom_runtime_rgb_verified.bin`（1,228,800 字节，source 640x480，resized 640x480，pad (0,80)）
    与 `runtime_input_rgb_u8.bin`（1,228,800 字节，source 263x191，resized 640x464，pad (0,88)）
    都能构造出合法 `ARIM` 信封，总长均为 1,228,840 字节，CRC32 分别为 `9391da25` 与 `7f31ec54`。
  - **错 CRC 不吃 request_id**：`handle_inference` 的顺序是
    `wifi_ready → tpu_ready → PhoneImageEnvelopeV1::parse → payload 长度 → try_acquire()`。
    `request_id` 在 `try_acquire()` 里才 `fetch_add`，而错 CRC 在 `parse` 阶段就已返回
    400 `INVALID_ENVELOPE`。因此校验器的"request_id 连续"检查不会假失败。
  - **契约常量一致**：`MODEL_NAME = "mushroom_yolov5s_cv181x_int8_sym"` 在板端
    `modules/axmodel_mushroom_yolov5/src/lib.rs:15` 与校验器第 20 行完全相同；
    `MAGIC=ARIM`、`VERSION=1`、`HEADER_LENGTH=40`、`PAYLOAD_LENGTH=1228800`、
    `CONTENT_TYPE=application/vnd.arceos.rgb-u8` 全部一致；
    `write_health_response` 恰好输出 `{"wifi":..,"tpu":..,"model":..}` 三个键，与
    校验器 `expected` 的精确相等比较相容。
- 下一步：继续等待传输完成（预计 `05:25`–`05:29`）。`MUSHROOM_WEB_URL` 出现后执行
  `verify_mushroom_web.py --url ... --input-rgb <上述文件> --input-meta <对应 json> --requests 20`。

### 05:15 +08:00 — 传输期间理清 `go` 之后的完整序列与时间预算

- 观察：传输仍在进行（PID 40712 CPU `7.02s → 7.27s`），日志仍受块缓冲未刷新。
- 判断依据：阅读 `send_arceos_xmodem.py` 在 `jump` 分支之后的流程，以及各 `verify_*` 的默认超时。
- 动作：把完整序列与超时预算整理进 `HANDOFF.md` §6.3。
- 结果：确认 `go 0x80200000` 之后的序列是——
  `send_aic_firmware_bundle`（**逐个握手**：板子先打印 `READY AIC_FIRMWARE <名字> <长度>`，
  每个文件 60 秒超时）→ `verify_aic8800_stack`(30s) → `_rf_and_mac`(60s) →
  `_management`/`_me`/`_sta_interface`(各 30s) → `_scan`(60s) → 若带
  `--wifi-credentials-prompt` 则 `send_wifi_credentials` → `_link_up`(30s) → `_dhcp`(150s)。
  加上固件本体约 380 KB（约 33 秒）与人工输入凭据的等待，`go` 之后还要数分钟；
  **整轮从按 RESET 到 `MUSHROOM_WEB_URL` 预期 20–28 分钟**。
  另注：`verify_aic8800_firmware_boot`（180 秒）虽然定义在文件里，但本启动器路径并不调用它。
- 下一步：继续等待。若在某个 `verify_*` 步骤超时，失败点会直接决定是固件握手、AIC8800 初始化，
  还是 DHCP 环节——那将是下一轮定位的入口。
### 13:48 +08:00 — 首次产品传输中止：日志止于 U-Boot 就绪行，且大镜像路径从未被验证

- 观察：会话空闲约 8.5 小时后回到现场（13:47）。`tools/sg2002/logs/mushroom-web-sta-board-20260925-051029.log`
  停在 `24,840` 字节，`lastWrite=08:15:38`；sender 进程（PID 40712）已退出；
  `COM3` 打开报 `The port 'COM3' does not exist.`（适配器已不在）。
- 判断依据：
  1. 逐字节检查日志原始末尾（UTF-16LE），内容止于
     `## Ready for binary (xmodem) download to 0x80200000 at 115200 bps...`，其后没有任何字节
     ——连 xmodem 库的进度输出都没有；
  2. 在 `C:\Users\chen\Documents\ArcOS移植sg2002` 全范围内按 `-newermt "2026-09-25 05:00"` 查找，
     **没有任何新增或修改文件**，说明再无第二次启动尝试；
  3. 检索全部历史日志中的 `## Total Size`，**XMODEM 只成功完成过两次**：`114,752` 字节与 `118,848` 字节。
- 动作：核对历史成功体积上限；确认当前无法再采集（COM3 已消失）；停止任何可能干扰的动作。
- 结果（本轮确定的结论）：
  - **本次产品镜像 `8,212,544` 字节，是历史最大成功传输 `118,848` 字节的约 69 倍。**
    把整个产品镜像走 U-Boot `loadx` + XMODEM 送进内存这条路径，
    **在本项目中从未在十分之一这个体积上被验证过**；
  - 传输在 U-Boot 就绪后没有产生任何进度输出，随后中止；日志未记录到任何异常文本，
    可能是 Python stdout 经 `Tee-Object` 管道块缓冲、进程被终止时缓冲丢失；
  - 当前 `COM3` 不存在，需重新插回适配器才能继续。
- 下一步：先确认现场现象（终端最后显示了什么、是否拔过适配器或按过 RESET），
  再决定是重试 XMODEM，还是改用更可靠的加载路径——板子 U-Boot 能读 SD 卡
  （`mmc0 is current device`、`Boot from SD dev 0`），可把 `.bin` 放进 FAT 分区后用
  `fatload mmc 0 0x80200000 <file>` + `go 0x80200000`，避开 8.2 MB 的串口传输。
  该替代方案属部署方式变更，需先与用户确认。

### 14:0x +08:00 — 改用 SD 卡 fatload 加载产品镜像

- 观察：05:10 的首次产品启动在 `XMODEM CRC handshake detected` 之后没有任何进度输出即中止，
  日志止于 `## Ready for binary (xmodem) download to 0x80200000 at 115200 bps...`；
  sender 已退出，COM3 不复存在。用户确认终端当时报了错。
- 判断依据：
  1. `XMODEM_TRANSFER_MODE = "xmodem"`（**普通 XMODEM，非 XMODEM-1K**），每帧仅 128 字节。
     产品镜像 `8,212,544` 字节 → **64,161 个停等往返**；
  2. 全部历史日志中 `## Total Size` 只出现过两次成功记录：`114,752` 与 `118,848` 字节，
     即该路径从未在超过十分之一产品体积上被验证；
  3. 同一份启动日志显示 U-Boot 从 SD 卡读 `11,757,220` 字节的 `boot.sd` 只用 `1043 ms`（10.7 MiB/s）；
  4. 出厂 Linux 的 init 会把整张卡导出为 USB 大容量存储
     （`echo /dev/mmcblk0 > functions/mass_storage.disk0/lun.0/file`）。
- 动作：
  1. 在 `send_arceos_xmodem.py` 增加 `load_binary_via_fatload()`：发送
     `fatload mmc 0 0x80200000 <文件名>`，并**校验 U-Boot 回报的字节数等于本地镜像大小**；
     同时识别 `Unable to read file` / `Failed to load` 两种失败；
  2. 增加 `--fatload=<文件名>` 选项，命中时跳过 XMODEM 传输，其余（`go`、AIC8800 固件握手、
     Wi-Fi 凭据、各 `verify_*`）完全复用；
  3. 启动器增加 `-FatloadName` 参数并透传，`[3/3]` 提示改为显示实际加载方式；
  4. 在 `HANDOFF.md` 新增 §6.5 记录完整操作步骤与失败判读。
- 结果：`send_arceos_xmodem.py` **28 项测试通过**（新增 3 项：成功、大小不符、文件不存在）；
  启动器测试 `SG2002 launcher validation PASS`；`-ValidateOnly` 配置已包含 `FatloadName`。
  提交 `4cecf8e feat(tools): load the product image from the SD card instead of over XMODEM`。
- 下一步：把 `.bin` 放入 TF 卡 FAT 分区（建议短名如 `arceos.bin`），用
  `-FatloadName arceos.bin` 重跑启动器并按一次 RESET。
  优先用读卡器取卡写入；若走板子 USB 大容量存储导出，注意 Linux 侧可能仍只读挂载着
  `mmcblk0p1`，存在写入冲突风险。
### 14:13–14:17 +08:00 — ArceOS 首次真板运行：fatload 生效，WPA2 链路打通，卡在 DHCP

- 观察：用户用 `-FatloadName arceos.bin` 运行启动器并按了一次 RESET。串口日志
  `tools/sg2002/logs/mushroom-web-sta-board-20260925-141315.log`（UTF-16LE）显示：
  `soph# U-Boot prompt detected` → `fatload mmc 0 0x80200000 arceos.bin` →
  **`8212544 bytes read in 729 ms (10.7 MiB/s)`** → `U-Boot read 8212544 bytes from arceos.bin`
  → `go 0x80200000` → `## Starting application at 0x80200000 ...` → ArceOS 横幅与
  `arch = riscv64 / platform = riscv64-licheerv-nano`，随后
  `SG2002 AIC8800D80 full firmware boot test`。
- 判断依据：
  1. **fatload 路径完全生效**：字节数精确等于本地镜像 `8,212,544`，耗时 `729 ms`，
     而 XMODEM 同体积预估需 14–18 分钟且历史上从未在十分之一体积上成功过；
  2. 横幅 `SG2002 AIC8800D80 full firmware boot test` 来自共享库
     `examples/aic8800-firmware-boot-licheerv-nano/src/lib.rs:125`，
     `mushroom-web-licheerv-nano` 在 `Cargo.toml:11` 依赖它，**不是装错了镜像**；
  3. AIC8800 固件 5 个文件全部握手成功（`AIC_FIRMWARE_RECEIVED` 逐一匹配 CRC32）；
  4. 门禁第 1、2 项取得证据：`AIC8800_FIRMWARE_BOOT_PASS`、`AIC8800_STACK_PASS`、
     `AIC8800_RF_MAC_PASS`（MAC `38:7a:cc:98:e6:46`）、`AIC8800_MANAGEMENT_PASS`、
     `AIC8800_ME_PASS`、`AIC8800_STA_INTERFACE_PASS`、`AIC8800_SCAN_PASS`，
     用户输入密码后 `AIC8800_ASSOCIATION_PASS` → EAPOL 1/2/3/4 → `AIC8800_KEY_INSTALL_PASS`
     → `AIC8800_LINK_UP_PASS` → **`AIC8800_CONTROL_PORT_OPEN_PASS`**；
  5. **卡在 DHCP**：`AIC8800_DHCP_STARTED` 之后持续输出
     `AIC8800_DHCP_DIAGNOSTIC tx=1..4 rx=0 ... last-tx-type=0x0800 last-tx-length=304
     last-rx-type=0x0000 last-rx-length=0`——**板子在发 DHCP DISCOVER，一个回包也没收到**。
- 排除项：满屏 `AIC8800_NETWORK_EVENT undecoded-data error=UnsupportedFrameControl
  { frame_control: 128 }` **不是成因**。`modules/axdriver_aic8800/src/data.rs:59` 判定
  `frame[0] & 0x0f != 0x08` 即报错，而 `0x0080` 的 frame[0]=0x80 是 **Beacon 管理帧**
  （type=0 subtype=8），只是被喂进了数据解码器。EAPOL 数据帧（ethertype `0x888e`）
  解码成功，说明接收路径本身可用。
- 结果：**门禁 1、2 项通过**（AIC8800 固件启动、WPA2 受控端口打开）；
  第 3 项 DHCP 未通过，`MUSHROOM_WEB_URL` 未出现；sender 在 150 秒 DHCP 超时后失败。
- 下一步（按优先级）：
  1. 在 MEIZU 热点侧验证 DHCP 是否正常：用另一台设备连同一热点确认能拿到地址；
     关掉再打开热点；检查是否有"最大设备数"限制或客户端隔离；
  2. 换一个普通路由器热点复测，以区分"热点侧不响应"与"板端 DHCP/广播收发问题"；
  3. 若换 AP 仍 `rx=0`，则怀疑广播数据帧的收发路径，需要查
     `axdriver_aic8800` 的 tx 目标地址处理与固件 RX 过滤设置。
### 14:30 +08:00 — DHCP 卡点定位：关联后收不到任何数据帧

- 观察：用户指出**这台电脑正连着 MEIZU 热点**（`WLAN 192.168.245.147`），即热点在手机上，
  电脑本身就是该热点的一个已获地址的客户端。
- 判断依据与动作：
  1. **热点侧排除**：电脑已从该热点取得 `192.168.245.147`，说明热点 DHCP 服务器工作正常。
     （另查到本机 `icssvc`/`IcsSvc`（Windows 移动热点服务）为 Stopped、`本地连接* 9/10`
     为 `169.254.x.x` Tentative——这属于正常，因为热点不在本机。）
  2. 复核板端 TX 路径：`network.rs` 的 `AicTransmitToken::consume` →
     `AicAssociationClient::send_ethernet_frame`（`association.rs:275`）→
     `build_d80_ethernet_data_transfer`；`destination_address` 直接取以太网帧 `frame[0..6]`，
     广播即 `ff:ff:ff:ff:ff:ff`，**无硬编码**。
     `build_d80_eapol_data_transfer` 只是转发给以太网构造器，两者描述符完全相同；
     TX 描述符的 `RWNX_HWQ_BE=1`、`EAPOL_TID=0`（TID 0 即 Best Effort，命名误导但非缺陷）。
  3. 复核板端 RX 路径：整个 749 行日志中出现的以太网类型**只有 `0x888e`（EAPOL，1 次）**；
     `AIC8800_DHCP_DIAGNOSTIC` 的 `rx` 自始至终为 `0`，`last-rx-type=0x0000`。
     `network.rs:142` 的 `receive()` 只在 `AssociationEvent::Data` 时返回帧，
     `rx=0` 意味着**关联建立后没有产生过任何一个 `Data` 事件**。
  4. 日志中 8 条 `undecoded-data` 与 1 条 `transport message-type=0x12` 受
     `self.transport_events < 8` **打印上限**约束，只是前若干条，不能当作总数。
- 结果（本轮确定的结论）：
  - 板端**不是** MAC 错误（`38:7a:cc:98:e6:46` 一致用于 `InterfaceConfig`、TX 源地址与固件读取）；
  - 板端**不是** TX 描述符硬编码问题（两个构造器共用同一路径）；
  - 真正症状是：**WPA2 关联完成、受控端口打开之后，固件不再向主机投递任何数据帧**，
    只投递管理帧（beacon）；DHCP DISCOVER 正常发出，OFFER 无论广播还是单播都收不到。
    这已不是"NAT/热点配置"问题，而是**板端固件/驱动的数据接收通路问题**。
  - 尚未定位到确切的代码缺陷，因此**本轮不做推测性修改**——改驱动内部而没有证据只会引入新变量。
- 下一步（区分剩余假设的关键实验）：让板子保持关联，同时**在电脑上制造广播流量**
  （例如 `ping 192.168.245.255`、浏览网页、`arp -d` 后 ping 网关），
  观察板端日志里是否出现**任何** `AIC8800_DATA_FRAME`。若一条都不出现，
  则确认是"关联后数据帧 RX 通路未生效"（怀疑固件 RX 过滤/数据通路使能未设置），
  需在 `me`/`association` 流程里补齐对应配置命令；若出现 ARP 但无 DHCP，
  则转向 DHCP 报文构造或广播处理方向。
