# SG2002 STA Phone-to-TPU Product Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce one LicheeRV Nano WE ArceOS image that joins the configured WPA2 network, obtains DHCP, serves the phone upload page, executes the fixed mushroom model on the SG2002 TPU, and returns detection JSON.

**Architecture:** Reuse the proven AIC8800 firmware/WPA2/DHCP bootstrap as the owner of the live smoltcp `Interface`, `Device`, and `SocketSet`. Add a host-tested polling stream adapter and one-request HTTP orchestration layer, then pass the DHCP-bound network session to the hardware product binary. The product initializes `TpuEngine` once and serves sequential TCP connections on port 80 without a filesystem.

**Tech Stack:** Rust `no_std`, ArceOS `axstd`/`axhal`, smoltcp revision `21a2f82`, SG2002 SDIO/AIC8800 drivers, SG2002 TPU DMABUF/WEIGHT runtime, PowerShell UART launcher.

**Spec:** `C:\Users\chen\Documents\ArcOS移植sg2002\LicheeRV_Nano_ArceOS手机上传与TPU推理设计.md`

## Global Constraints

- Target board is LicheeRV Nano WE with SG2002 and AIC8800D80.
- `PhoneImageEnvelopeV1` is exactly 40 header bytes plus 1,228,800 RGB_U8 CHW payload bytes.
- HTTP routes are exactly `GET /`, `GET /health`, and `POST /api/infer`.
- `POST /api/infer` uses `application/vnd.arceos.rgb-u8` and `Content-Length: 1228840`.
- The fixed DMABUF and WEIGHT assets are loaded once by `TpuEngine::initialize`; ONNX and TPU-MLIR never run on the board.
- Requests are handled sequentially; no image or result history is written to storage.
- Wi-Fi credentials remain UART-provided runtime data and must not be added to source, logs, images, or Git.
- Existing QEMU examples and existing LicheeRV Nano diagnostic examples must continue to build.
- Every task ends with fresh verification, a Git commit, and a push to `personal/codex/sg2002-phone-tpu`.

## Review Focus

- A request split at every possible header/body boundary must produce the same parsed request; Task 1 tests fragmented reads.
- A peer that disconnects during headers or body must not call the TPU backend; Task 1 tests both truncation points.
- Repeated `Pending` receive/send states must either make progress or end at the fixed inactivity timeout; Task 2 tests both directions.
- Partial TCP sends must preserve byte order and length; Task 2 tests multiple partial sends before completion.
- TPU initialization failure must leave `/health` available and make `/api/infer` return `503`; Task 3 tests the backend/readiness wrapper.

---

### Task 1: One-connection HTTP orchestration

**Files:**
- Create: `examples/mushroom-web-licheerv-nano/src/server.rs`
- Modify: `examples/mushroom-web-licheerv-nano/src/lib.rs`
- Test: `examples/mushroom-web-licheerv-nano/tests/server.rs`

**Interfaces:**
- Consumes: `stream::receive_request`, `service::handle_request`, `service::write_http_rejection`, `response::ByteWriter`, `service::InferenceBackend`.
- Produces: `serve_one<S, B>(stream: &mut S, header_storage: &mut [u8; MAX_REQUEST_HEADER_LENGTH], body_storage: &mut [u8], readiness: ServiceReadiness, inference_lock: &InferenceLock, backend: &mut B, receive_us: u64) -> Result<ServeOutcome, ServeError<S::ReadError, S::WriteError>>`, where `S: ByteReader + ByteWriter` uses distinct associated error aliases through a new `DuplexStream` trait.

- [ ] **Step 1: Write failing orchestration tests**

  Add literal HTTP fixtures proving: `GET /health` returns `200`; a valid inference request calls the backend once; malformed HTTP writes the exact mapped error response; EOF during headers and EOF during body return `ServeOutcome::PeerClosed` and never call the backend.

- [ ] **Step 2: Run the focused tests and observe RED**

  Run: `cargo test -p arceos-mushroom-web-licheerv-nano --test server`

  Expected: compilation fails because `server` and `serve_one` do not exist.

- [ ] **Step 3: Implement the minimal orchestration boundary**

  Define:

  ```rust
  pub trait DuplexStream {
      type ReadError;
      type WriteError;
      fn read(&mut self, destination: &mut [u8]) -> Result<usize, Self::ReadError>;
      fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::WriteError>;
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ServeOutcome { Responded, PeerClosed }

  #[derive(Debug, Eq, PartialEq)]
  pub enum ServeError<R, W> { Read(R), Write(W) }
  ```

  Adapt the existing reader/writer calls without changing the parser or response contracts. Map `Http` to its specified response, `HeaderTooLarge` to HTTP `413` with code `HEADER_TOO_LARGE`, and `UnexpectedEof` to `PeerClosed` without invoking inference.

- [ ] **Step 4: Run focused and package tests**

  Run: `cargo test -p arceos-mushroom-web-licheerv-nano --test server && cargo test -p arceos-mushroom-web-licheerv-nano`

  Expected: all product package tests pass with zero failures.

- [ ] **Step 5: Commit and push**

  Commit message: `feat: add mushroom HTTP connection service`

### Task 2: Polling TCP stream core

**Files:**
- Create: `examples/mushroom-web-licheerv-nano/src/polling_stream.rs`
- Modify: `examples/mushroom-web-licheerv-nano/src/lib.rs`
- Test: `examples/mushroom-web-licheerv-nano/tests/polling_stream.rs`

**Interfaces:**
- Consumes: `server::DuplexStream`.
- Produces: `TcpPump`, `ReceiveState`, `SendState`, `PollingStream<P>`, and `PollingStreamError<P::Error>`; the hardware smoltcp adapter in Task 3 implements `TcpPump`.

- [ ] **Step 1: Write failing state-machine tests**

  Use a scripted real test double implementing the complete `TcpPump` contract. Prove: pending then data returns the exact bytes; closed receive returns `0`; pending until `HTTP_IO_INACTIVITY_TIMEOUT_NANOS` returns `Timeout`; partial sends concatenate to the exact input; transport failures preserve the underlying error.

- [ ] **Step 2: Run the focused tests and observe RED**

  Run: `cargo test -p arceos-mushroom-web-licheerv-nano --test polling_stream`

  Expected: compilation fails because the polling stream types do not exist.

- [ ] **Step 3: Implement the minimal polling stream**

  Define the inactivity limit as 30 seconds and reset it on every successful receive or send. `read` repeatedly calls `poll` then `try_receive`; `write_all` repeats until every byte is accepted. `Pending` spins through the pump, `Closed` becomes EOF for reads and `Closed` error for writes, and the timeout is measured by the pump's monotonic `now_nanos`.

- [ ] **Step 4: Run focused and package tests**

  Run: `cargo test -p arceos-mushroom-web-licheerv-nano --test polling_stream && cargo test -p arceos-mushroom-web-licheerv-nano`

  Expected: all product package tests pass with zero failures.

- [ ] **Step 5: Commit and push**

  Commit message: `feat: add polling TCP stream for SG2002 web service`

### Task 3: Hardware smoltcp server and TPU product binary

**Files:**
- Modify: `examples/aic8800-firmware-boot-licheerv-nano/src/lib.rs`
- Modify: `examples/aic8800-firmware-boot-licheerv-nano/src/main.rs`
- Modify: `examples/mushroom-web-licheerv-nano/Cargo.toml`
- Create: `examples/mushroom-web-licheerv-nano/src/hardware.rs`
- Create: `examples/mushroom-web-licheerv-nano/src/main.rs`
- Modify: `examples/mushroom-web-licheerv-nano/src/lib.rs`
- Test: `examples/mushroom-web-licheerv-nano/tests/server.rs`

**Interfaces:**
- Consumes: `DhcpBoundHandler`, live smoltcp interface/device/socket set, Task 2 `TcpPump`, Task 1 `serve_one`, and `TpuEngine`.
- Produces: a hardware-gated binary `arceos-mushroom-web-licheerv-nano` and `run_http_server<D: Device>(...) -> !` listening on TCP port 80.

- [ ] **Step 1: Add the failing unavailable-engine service test**

  Add a backend wrapper test proving `/health` reports `tpu: NotReady` and `/api/infer` returns `503 TPU_NOT_READY` without calling an unavailable backend.

- [ ] **Step 2: Run the focused test and observe RED**

  Run: `cargo test -p arceos-mushroom-web-licheerv-nano --test server`

  Expected: compilation fails because the hardware backend readiness wrapper does not exist.

- [ ] **Step 3: Expose the DHCP socket handle to the bound handler**

  Extend the exact callback to receive `dhcp_handle: smoltcp::iface::SocketHandle`. Keep the diagnostic binary as a no-op handler and cross-build it after the signature change.

- [ ] **Step 4: Implement hardware integration**

  Add optional hardware dependencies on the AIC8800 boot package, `axhal`, `axstd`, the LicheeRV Nano platform, and smoltcp with `socket-tcp`. Use 64 KiB RX and TX buffers, matching `modules/axnet/src/smoltcp_impl/mod.rs`. The pump polls the interface, drains DHCP configuration events, receives/sends through the TCP socket, and reports link/transport closure without borrowing the socket across an interface poll.

  The binary initializes `TpuEngine` once after DHCP. It keeps an unavailable wrapper if initialization fails, prints the actual DHCP address and `http://<address>/`, allocates one reusable 4,096-byte header buffer and one reusable 1,228,840-byte body buffer, accepts one connection at a time, invokes `serve_one`, closes or aborts the socket, and returns it to `listen(80)`.

- [ ] **Step 5: Run host tests and both RISC-V cross-builds**

  Run:

  ```text
  cargo test -p arceos-mushroom-web-licheerv-nano
  make A=examples/aic8800-firmware-boot-licheerv-nano MYPLAT=axplat-riscv64-licheerv-nano build
  make A=examples/mushroom-web-licheerv-nano MYPLAT=axplat-riscv64-licheerv-nano FEATURES=hardware build
  cargo fmt --all -- --check
  git diff --check
  ```

  Expected: all tests pass, both binaries are produced, formatting passes, and the diff check is empty.

- [ ] **Step 6: Commit and push**

  Commit message: `feat: run mushroom web inference over AIC8800`

### Task 4: Deployment launcher and true-board STA gate

**Files:**
- Modify: `C:\Users\chen\Documents\ArcOS移植sg2002\run_arceos_licheerv_nano.ps1`
- Modify: `C:\Users\chen\Documents\ArcOS移植sg2002\TPU_INFERENCE_STATUS.md`
- Create: `C:\Users\chen\Documents\ArcOS移植sg2002\logs\mushroom-web-sta-board-20260923.log`

**Interfaces:**
- Consumes: Task 3 product binary, existing UART/XMODEM/U-Boot launcher, and UART-provided Wi-Fi credentials/firmware transfer protocol.
- Produces: one command that builds, transfers, boots, captures UART evidence, and prints the board URL without storing credentials.

- [ ] **Step 1: Write a failing PowerShell launcher test**

  Add or extend a Pester-free parser test that invokes the launcher validation mode and proves the exact example name `mushroom-web` resolves to the Task 3 ELF/BIN paths and the current WSL distro `Ubuntu`.

- [ ] **Step 2: Run it and observe RED**

  Run the existing launcher test entrypoint from PowerShell.

  Expected: failure because `mushroom-web` is not registered.

- [ ] **Step 3: Implement the launcher route without credentials**

  Register the exact product example and its hardware feature. Preserve interactive UART credential entry and firmware streaming; do not add SSID or password literals to the script, command line, or log.

- [ ] **Step 4: Run launcher tests and perform the board gate**

  Build and start capture, ask the user only for the physical RESET press when the listener is ready, and retain the complete UART log. Required evidence is: AIC firmware boot, WPA2 controlled-port open, DHCP address, printed HTTP URL, `/health` response, one valid phone upload returning detection JSON, one invalid CRC rejected before TPU, and a second valid upload proving the listener is reusable.

- [ ] **Step 5: Record evidence, commit, and push**

  Update the status document with exact log lines and hashes. Commit source-controlled launcher/status changes to their configured Git repository and push; if the outer workspace has no Git remote, copy the maintained launcher into `tools/sg2002/` in the ArceOS branch, commit there, and push.

### Task 5: STA product regression and documentation reconciliation

**Files:**
- Modify: `docs/superpowers/plans/2026-09-23-sg2002-sta-product-closure.md`
- Modify: `C:\Users\chen\Documents\ArcOS移植sg2002\LicheeRV_Nano_ArceOS手机上传与TPU推理设计.md`

**Interfaces:**
- Consumes: all Task 4 board evidence and the existing QEMU/LicheeRV Nano build matrix.
- Produces: an evidence-backed STA completion record and the exact next gate for AP mode.

- [ ] **Step 1: Run the complete host and cross-build regression**

  Run all package tests for the SDIO driver, AIC8800 driver, model runtime, and product service; then build every registered LicheeRV Nano example and the existing QEMU `examples/helloworld` and `examples/httpserver` examples.

- [ ] **Step 2: Run the 20-request board soak**

  Upload the same validated image 20 times from the phone page. Record request IDs, input CRC, detection count, TPU status, per-stage timing, connection recovery, and memory allocation behavior. Any crash, changed CRC, missing response, or growing per-request allocation fails this gate.

- [ ] **Step 3: Reconcile documentation with evidence**

  Replace stale WPA2/DHCP status text with exact commit IDs and board log evidence. Mark only proven gates complete and identify AP mode as the next task; do not claim camera or Bluetooth support.

- [ ] **Step 4: Commit and push**

  Commit message: `docs: record SG2002 STA phone inference gate`
