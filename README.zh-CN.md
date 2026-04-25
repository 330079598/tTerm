# tTerm — 现代化 SSH 终端与 SFTP 文件管理器

[English](./README.md) | 简体中文

`tTerm` 是一个面向开发者、运维和日常服务器使用者的桌面终端工具。它把 **本地终端、SSH 远程连接、SFTP 文件管理、传输进度追踪、会话恢复和安全密码存储** 放在同一个轻量应用里，让你不用在终端、文件传输工具和连接管理器之间来回切换。

如果你经常连接服务器、上传下载文件、管理多个终端会话，tTerm 想成为那个“打开就能干活”的工具。

## 为什么选择 tTerm？

- **终端和文件管理一体化**：SSH 连接后即可打开 SFTP 面板，浏览目录、上传、下载、重命名、删除文件。
- **多任务更顺手**：多标签会话、标签拖拽排序、会话恢复，让多个服务器和本地终端并行工作不混乱。
- **传输过程看得见**：上传、下载、批量操作都有进度展示，任务状态清晰可控。
- **更安全的连接体验**：支持主机密钥确认、连接配置保存，以及系统安全存储/加密保险库能力。
- **更现代的桌面体验**：基于 Tauri 构建，体积轻、启动快，拥有主题、字体、国际化和跨平台支持。

## 核心功能

### 终端体验

- 本地 Shell 终端
- 多标签会话管理
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

### 内置 SFTP 文件管理器

- 浏览远程目录
- 创建文件夹
- 文件/目录重命名
- 文件/目录删除
- 批量删除与删除前预览
- 文件上传、下载
- 拖拽上传本地文件/文件夹
- 传输取消与状态追踪

### 传输管理器

- 展示上传/下载任务
- 支持批量传输任务展开查看
- 实时进度、速度和状态反馈
- 清理已完成任务

### 个性化与易用性

- 多套内置主题
- 自定义主题编辑
- 终端配色预览
- 字体设置
- 中英文界面
- Windows、macOS、Linux 桌面端支持

### 安全能力

- 系统 Keyring 集成
- 可选加密密码保险库
- 旧版 SSH 密码数据迁移
- 敏感数据本地存储

## 快速开始

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

### 构建桌面应用

```bash
pnpm tauri build
```

构建产物会由 Tauri 生成到 `src-tauri/target` 下对应平台的 bundle 目录中。

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
│   └── src/terminal/    # 终端类型和交互
├── public/              # 静态资源
└── dist/                # 前端构建输出
```

## 使用场景

- 同时维护多台服务器，需要快速切换 SSH 会话
- 在服务器和本机之间频繁上传、下载文件
- 希望在一个窗口内完成终端操作和远程文件管理
- 需要保存常用连接配置，但又希望密码安全存储在本地
- 想要一个更轻、更现代、可自定义主题的桌面终端工具

## 开发说明

tTerm 的前端通过 Tauri `invoke` 调用 Rust 后端命令。终端能力由 `portable-pty` 提供，SSH/SFTP 能力由 `russh` 和 `russh-sftp` 提供，敏感信息通过系统 Keyring 或本地加密保险库管理。

开发时建议同时关注：

- 前端交互是否符合多标签、多任务场景
- Rust 命令是否返回清晰、可展示的错误信息
- 文件传输是否有完整的进度、取消和失败状态
- 涉及密码、密钥、主机指纹的逻辑是否保持本地、安全、可确认

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
