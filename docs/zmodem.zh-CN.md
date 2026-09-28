# ZMODEM 文件传输（rz / sz）

[English](./zmodem.md) | 简体中文

在无法使用 SFTP 的场景（网络设备控制台、串口/嵌入式设备、只提供交互式 Shell 的堡垒机等），可以直接在终端里用 `rz` / `sz` 传文件，行为与 SecureCRT / Xshell 一致。本地终端和 SSH 会话都支持，无需额外配置。

> 远端需要安装 `lrzsz`（如 `apt install lrzsz`、`yum install lrzsz`，macOS 本地终端可用 `brew install lrzsz`）。

## 下载文件（远端 → 本机）

```bash
sz file.tar.gz            # 单个文件
sz a.log b.log c.log      # 多个文件，依次传输
```

tTerm 检测到传输请求后立即开始接收，无需任何弹窗，文件保存到 ZMODEM 下载目录（默认是系统"下载"文件夹）。同名文件不会被覆盖，而是自动重命名为 `file (1).tar.gz`。

## 上传文件（本机 → 远端）

```bash
rz
```

tTerm 检测到 `rz` 后会弹出文件选择框（可多选），选中后开始上传到远端当前目录；在选择框中点取消则会通知远端 `rz` 正常退出。

## 进度与取消

- 传输进度、速度和结果显示在传输管理器中，与 SFTP 任务共用同一个面板。
- 取消方式：在传输管理器中点击取消，或直接在终端按 `Ctrl+C`。两种方式都会向远端发送标准中止序列，让远端 `rz`/`sz` 立即退出，不必等待超时。
- 传输过程中，除 `Ctrl+C` 外的键盘输入会被暂时屏蔽，避免破坏传输数据。
- SSH 断线重连时，未完成的传输会被自动清理（远端进程已随旧连接结束）。

## 设置与手动触发

- **设置 → 通用 → ZMODEM**：
  - **自动识别 ZMODEM 传输**（默认开启）：关闭后 tTerm 不再扫描终端输出中的 rz/sz 触发序列。
  - **下载目录**：自定义下载文件的保存位置，留空则使用系统下载目录。
- **手动触发**：关闭了自动识别，或自动识别没有生效时，可在终端右键菜单选择 **"使用 ZMODEM 发送文件 (rz)"** / **"使用 ZMODEM 接收文件 (sz)"**，然后在终端中运行 `rz` 或 `sz <文件>`。也可以在 **设置 → 键盘快捷键 → ZMODEM** 中为这两个动作绑定快捷键（默认未绑定）。手动触发只对当前标签页的下一次传输生效。

## SFTP 还是 ZMODEM？

两者互不冲突，不需要在连接时选择：SFTP 使用独立的 SSH 子通道，适合浏览目录、批量和大文件传输，速度更快；ZMODEM 走终端数据流本身，适合没有 SFTP 子系统、需要经过多级跳板/`su`/`docker exec` 等场景，或者只是想在当前 Shell 所在目录顺手传一个文件。服务器支持 SFTP 时优先使用 SFTP。

## 实现方案

ZMODEM 协议完全在 Rust 后端实现（`src-tauri/src/zmodem/`），前端只负责设置、文件选择和进度展示。

- **自研推送式协议引擎**：没有使用现有的 `zmodem` crate（同步阻塞读写、仅支持单文件、已停止维护）。`protocol/engine.rs` 是一个纯状态机：`feed(bytes) -> { outgoing, actions, passthrough }`，本身不持有任何 I/O 句柄。原因是 `portable-pty` 的 writer 只能 `take_writer()` 一次，因此由调用方（本地 PTY 读线程 / SSH 读任务）用已有的写句柄把 `outgoing` 写回。
- **协议细节**：支持 CRC16 与 CRC32（发送端使用 CRC32）、ZDLE 转义、十六进制与二进制头、`ZRPOS` 断点重传、多文件批量接收。所有线上格式均用真实 `lrzsz` 抓包校验。
- **触发检测**：`detect.rs` 扫描终端输出中的 `**\x18B00`（远端运行 `sz`，本机接收）和 `**\x18B01`（远端运行 `rz`，本机发送），并保留少量跨读取边界的缓存，防止触发序列被拆成两段时漏检。
- **数据通路**：本地 PTY 和 SSH 的原始字节链路本身就是二进制安全的，因此 ZMODEM 数据不经过任何 UTF-8 转换。检测到传输后，终端输出改为交给协议引擎处理；传输结束后剩余的字节（如新的 Shell 提示符）照常送到 xterm。
- **接收方向**：直接在读线程/读任务中处理，写入下载目录，文件名会被净化以防止目录穿越。
- **发送方向**：需要主动连续推送数据块而不等待逐块确认，所以在独立的 OS 线程上运行（`send.rs`）。读线程通过 `std::sync::mpsc` 把远端回传的字节转发给发送线程；发送线程结束后通道关闭，读线程自动恢复正常处理。本地 PTY 发送期间会临时把终端设为 raw 模式（`cfmakeraw`），防止回显和 XON/XOFF 流控破坏二进制帧。另外遇到 `ZNAK` 或重复的 `ZRINIT` 时会重发 `ZFILE` 头，作为兜底（SSH 场景下远端终端模式不受本机控制，只能依靠这一层）。
- **状态管理**：每个标签页的传输状态存放在 `ZmodemMap`、`ZmodemArmedSendMap`、`ZmodemManualDetectMap` 中（`core/state.rs`），并用 `session_nonce` 防止旧会话串用。由于这些状态会同时被 OS 线程和异步任务访问，统一使用 `std::sync` 锁，而不是 tokio 锁的 `blocking_*` 方法。
- **前端**：`useZmodemTransfers.ts` 监听 `zmodem-send-requested-{tabId}`、`zmodem-transfer-start/progress/complete-{tabId}` 事件，复用现有的传输管理器；`TransferTask.cancel` 让 ZMODEM 任务使用自己的取消逻辑。

测试方面，除单元测试外还有 8 个需要真实环境的集成测试（默认 `#[ignore]`），分别用真实 `sz`/`rz` 通过管道、本地 PTY 和临时启动的用户态 `sshd` 验证上传与下载：

```bash
brew install lrzsz   # 或对应平台的 lrzsz 包
cd src-tauri && cargo test --lib zmodem::real -- --ignored --test-threads=1
```

