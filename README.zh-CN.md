# tTerm — 现代化 SSH 终端与 SFTP 文件管理器

[English](./README.md) | 简体中文

`tTerm` 是一个面向开发者、运维和日常服务器使用者的桌面终端工具。它把 **本地终端、SSH 远程连接、SFTP 文件管理、传输进度追踪、会话恢复和安全密码存储** 放在同一个轻量应用里，让你不用在终端、文件传输工具和连接管理器之间来回切换。

如果你经常连接服务器、上传下载文件、管理多个终端会话，tTerm 想成为那个”打开就能干活”的工具。

![main-overview](./docs/screenshot/main-overview.png)

## 为什么选择 tTerm？

- **终端和文件管理一体化**：SSH 连接后即可打开 SFTP 面板，浏览目录、上传、下载、重命名、删除文件。
- **多任务更顺手**：多标签会话、标签拖拽排序、会话恢复，让多个服务器和本地终端并行工作不混乱。
- **ZMODEM（rz/sz）传输**：无 SFTP 的设备或跳板机上，直接在终端里 `rz`/`sz` 上传下载，自动识别，与 SecureCRT/Xshell 用法一致。
- **传输过程看得见**：上传、下载、批量操作都有进度展示，任务状态清晰可控。
- **可视化端口转发**：在独立页面管理本地、远程、动态（SOCKS5）SSH 隧道，实时查看状态与流量，断线自动重连。
- **更安全的连接体验**：支持主机密钥确认、连接配置保存，以及系统安全存储/加密保险库能力。
- **更现代的桌面体验**：基于 Tauri 构建，体积轻、启动快，拥有主题、字体、国际化和跨平台支持。

## 核心功能

### 终端体验

![terminal-search](./docs/screenshot/terminal-search.png)

- 本地 Shell 终端，支持自定义 Shell（自动检测，或从系统可用 Shell 中选择）
- 终端内搜索，实时高亮匹配结果
- 多标签会话管理，支持复制、重命名、关闭其他、关闭右侧标签
- 标签拖拽排序
- 终端尺寸自适应
- Web 链接识别
- 应用重启后恢复上次会话

### SSH 连接管理

- SSH 远程终端连接
- 密码或密钥认证
- 连接配置保存与复用
- 连接测试
- SSH 主机密钥确认
- 已知主机记录管理
- 每个标签页可固定的连接信息栏

### 内置 SFTP 文件管理器

![sftp-browser](./docs/screenshot/sftp-browser.png)

- 浏览远程目录
- 创建文件夹
- 文件/目录重命名
- 文件/目录删除
- 批量删除与删除前预览
- 文件上传、下载
- 拖拽上传本地文件/文件夹
- 传输取消与状态追踪
- 剪贴板支持，可复制文件路径

### ZMODEM 文件传输（rz / sz）

在无法使用 SFTP 的场景（网络设备控制台、串口/嵌入式设备、只提供交互式 Shell 的堡垒机等），可以直接在终端里用 `rz` / `sz` 传文件，行为与 SecureCRT / Xshell 一致。本地终端和 SSH 会话都支持，无需额外配置。

> 远端需要安装 `lrzsz`（如 `apt install lrzsz`、`yum install lrzsz`，macOS 本地终端可用 `brew install lrzsz`）。

**下载文件（远端 → 本机）**

```bash
sz file.tar.gz            # 单个文件
sz a.log b.log c.log      # 多个文件，依次传输
```

tTerm 检测到传输请求后立即开始接收，无需任何弹窗，文件保存到 ZMODEM 下载目录（默认是系统"下载"文件夹）。同名文件不会被覆盖，而是自动重命名为 `file (1).tar.gz`。

**上传文件（本机 → 远端）**

```bash
rz
```

tTerm 检测到 `rz` 后会弹出文件选择框（可多选），选中后开始上传到远端当前目录；在选择框中点取消则会通知远端 `rz` 正常退出。

**进度与取消**

- 传输进度、速度和结果显示在传输管理器中，与 SFTP 任务共用同一个面板。
- 取消方式：在传输管理器中点击取消，或直接在终端按 `Ctrl+C`。两种方式都会向远端发送标准中止序列，让远端 `rz`/`sz` 立即退出，不必等待超时。
- 传输过程中，除 `Ctrl+C` 外的键盘输入会被暂时屏蔽，避免破坏传输数据。
- SSH 断线重连时，未完成的传输会被自动清理（远端进程已随旧连接结束）。

**设置与手动触发**

- **设置 → 通用 → ZMODEM**：
  - **自动识别 ZMODEM 传输**（默认开启）：关闭后 tTerm 不再扫描终端输出中的 rz/sz 触发序列。
  - **下载目录**：自定义下载文件的保存位置，留空则使用系统下载目录。
- **手动触发**：关闭了自动识别，或自动识别没有生效时，可在终端右键菜单选择 **"使用 ZMODEM 发送文件 (rz)"** / **"使用 ZMODEM 接收文件 (sz)"**，然后在终端中运行 `rz` 或 `sz <文件>`。也可以在 **设置 → 键盘快捷键 → ZMODEM** 中为这两个动作绑定快捷键（默认未绑定）。手动触发只对当前标签页的下一次传输生效。

**SFTP 还是 ZMODEM？**

两者互不冲突，不需要在连接时选择：SFTP 使用独立的 SSH 子通道，适合浏览目录、批量和大文件传输，速度更快；ZMODEM 走终端数据流本身，适合没有 SFTP 子系统、需要经过多级跳板/`su`/`docker exec` 等场景，或者只是想在当前 Shell 所在目录顺手传一个文件。服务器支持 SFTP 时优先使用 SFTP。

### 传输管理器

- 展示上传/下载任务
- 支持批量传输任务展开查看
- 实时进度、速度和状态反馈
- 清理已完成任务

### 个性化与易用性

![theme-editor](./docs/screenshot/theme-editor.png)

- 多套内置主题
- 自定义主题编辑，支持实时预览
- 终端 16 色配色预览
- 字体设置，支持系统字体选择器
- 光标样式选择
- 中英文界面，自动检测系统语言
- Windows、macOS、Linux 桌面端支持
- 版本间配置自动迁移

### 安全能力

- 系统 Keyring 集成
- 可选加密密码保险库
- 旧版 SSH 密码数据迁移
- 敏感数据本地存储

## 快速开始

### 下载

可在 [Releases](https://github.com/330079598/tTerm/releases) 页面下载预编译版本：

- **macOS** — Apple Silicon (aarch64) 和 Intel (x86_64) `.dmg` 与 `.app` 包
- **Windows** — `.exe` NSIS 安装包
- **Linux** — `.AppImage`、`.deb` 和 `.rpm` 包

### 环境要求

- Node.js 18+
- pnpm
- Rust 1.70+
- Tauri 2 所需的系统依赖

不同平台还需要：

- **Windows**：Microsoft C++ Build Tools
- **macOS**：Xcode Command Line Tools
- **Linux**：WebKitGTK、OpenSSL、AppIndicator 等 Tauri 依赖

### 安装依赖

```bash
pnpm install
```

### 开发模式运行

```bash
pnpm tauri dev
```

在 Linux 上，如果 Wayland 后端因 GDK 协议错误退出，或者 WebKitGTK 报告 GBM
缓冲区错误，可以使用兼容 XWayland 的开发命令：

```bash
pnpm tauri:dev:linux
```

该命令会设置 `GDK_BACKEND=x11` 并禁用 WebKitGTK 的 DMABUF 渲染器，系统需要已安装
XWayland。

### 构建桌面应用

```bash
pnpm tauri build
```

构建产物会由 Tauri 生成到 `src-tauri/target` 下对应平台的 bundle 目录中。

### CI/CD

项目包含 GitHub Actions 工作流，用于自动化构建：

- **release-main.yml** — 推送到 `main` 或 `v*` 标签时触发，自动构建 macOS (aarch64 + x86_64)、Ubuntu 24.04 和 Windows 版本，并创建 GitHub 草稿发布。
- **release-dev-beta.yml** — 开发/测试版发布工作流。

## 常用脚本

```bash
# 启动前端开发服务器
pnpm dev

# 构建前端资源
pnpm build

# 预览前端构建结果
pnpm preview

# 启动 Tauri 开发模式
pnpm tauri dev

# 在 Linux 上通过 XWayland 启动（Wayland/GBM 兼容模式）
pnpm tauri:dev:linux

# 构建桌面应用
pnpm tauri build

# 检查代码规范
pnpm lint

# 自动修复 ESLint 问题
pnpm lint:fix

# 格式化源码
pnpm format

# 检查格式化
pnpm format:check
```

## 技术栈

### 前端

- React 18
- TypeScript
- Vite
- TanStack Router
- xterm.js
- i18next / react-i18next
- Radix UI Toast
- lucide-react
- Tailwind CSS 4

### 桌面与后端

- Tauri 2
- Rust 2021
- portable-pty
- russh
- russh-sftp
- Tokio
- keyring
- aes-gcm / argon2 / zeroize

## 项目结构

```text
.
├── src/                 # React 前端应用
│   ├── components/      # 终端、SFTP、设置、主题等 UI 组件
│   ├── contexts/        # 配置、主题、传输任务等全局状态
│   ├── hooks/           # 标签、连接、会话恢复等业务 Hook
│   ├── i18n/            # 中英文国际化资源
│   ├── lib/             # 主题、启动、工具函数
│   ├── routes/          # TanStack Router 页面
│   └── types/           # 前端类型定义
├── src-tauri/           # Tauri / Rust 后端
│   ├── src/config/      # 应用配置与路径
│   ├── src/core/        # PTY、命令和应用状态
│   ├── src/fonts/       # 系统字体能力
│   ├── src/profiles/    # 连接配置管理
│   ├── src/session/     # 会话持久化
│   ├── src/sftp/        # SFTP 连接、文件操作和传输
│   ├── src/ssh/         # SSH 客户端、密钥、密码存储
│   ├── src/terminal/    # 终端类型和交互
│   └── src/zmodem/      # ZMODEM（rz/sz）协议引擎、触发检测与收发
├── public/              # 静态资源
└── dist/                # 前端构建输出
```

## 使用场景

- 同时维护多台服务器，需要快速切换 SSH 会话
- 在服务器和本机之间频繁上传、下载文件
- 希望在一个窗口内完成终端操作和远程文件管理
- 在网络设备、嵌入式设备或只允许交互式 Shell 的堡垒机上用 `rz`/`sz` 传文件
- 需要保存常用连接配置，但又希望密码安全存储在本地
- 想要一个更轻、更现代、可自定义主题的桌面终端工具

## 开发说明

tTerm 的前端通过 Tauri `invoke` 调用 Rust 后端命令。终端能力由 `portable-pty` 提供，SSH/SFTP 能力由 `russh` 和 `russh-sftp` 提供，敏感信息通过系统 Keyring 或本地加密保险库管理。

开发时建议同时关注：

- 前端交互是否符合多标签、多任务场景
- Rust 命令是否返回清晰、可展示的错误信息
- 文件传输是否有完整的进度、取消和失败状态
- 涉及密码、密钥、主机指纹的逻辑是否保持本地、安全、可确认
- 版本间配置迁移是否覆盖所有数据类型（连接配置、会话、已知主机、密码、SFTP 目录缓存）

### ZMODEM 实现方案

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

## 贡献

欢迎提交 Issue 和 Pull Request。建议在提交前运行：

```bash
pnpm lint
pnpm format:check
pnpm build
```

如果改动涉及 Rust/Tauri 后端，也建议运行：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## License

MIT
