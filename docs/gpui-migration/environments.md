# 测试环境清单

本文件是任务卡 [W0-03](p0-validation.md#w0-03-三平台测试环境清单-人工) 的产出，列出 GPUI 迁移可用的测试机器与环境、每个环境的进入方式和已装工具。后续各卡按本表判断能验证什么：**标为“缺失”的环境，相关验收项一律记为“未验证”**（[执行规则第 12 条](README.md#2-执行规则)）。

- 状态只有“可用”和“缺失”两种。
- 不在本文件写密码、令牌、私钥或公网地址。进入方式只写机器名、VM 名、内网地址与账号名。
- 环境有变化（新建 VM、换显示器、升级系统）时更新本表，并在最后的“变更记录”中加一行。缺失的环境补上之后，此前记为“未验证”的项需要在对应卡上补验证，不会自动变成“通过”。
- 信息来源：Windows 物理机与 SSH 测试主机在 2026-11-07 由本机命令采集；Mac 的信息取自此前各卡报告中记录的环境（W0-04、P0-06、P0-08、P0-25、P0-28），没有记录的项写“未记录”。

## 1. 总表

| 环境                                 | 状态 | 用于                                  | 位置                              |
| ------------------------------------ | ---- | ------------------------------------- | --------------------------------- |
| Windows 11 物理机                    | 可用 | P0-01、P0-17、P0-18、P0-21、P0-26     | [2.1](#21-windows-11-物理机)      |
| Windows 多显示器、不同缩放           | 可用 | P0-18                                 | [2.1](#21-windows-11-物理机)      |
| Windows 10 22H2                      | 缺失 | 兼容性                                | [2.2](#22-windows-虚拟机)         |
| 经 RDP 访问的 Windows                | 缺失 | P0-17、P0-18（断开重连）              | [2.2](#22-windows-虚拟机)         |
| 无 3D 加速的 Windows 虚拟机          | 缺失 | P0-17                                 | [2.2](#22-windows-虚拟机)         |
| Citrix 或其他云桌面                  | 缺失 | P0-17                                 | —                                 |
| macOS Apple Silicon，HiDPI（2×）     | 可用 | P0-02、P0-18、P0-24、W0-05、对比工具  | [3.1](#31-mac-mini-m4)            |
| macOS 内建 Retina 显示器             | 缺失 | P0-02                                 | [3.1](#31-mac-mini-m4)            |
| macOS 外接普通 DPI 显示器（1×）      | 缺失 | P0-02、P0-18                          | [3.1](#31-mac-mini-m4)            |
| Intel Mac                            | 缺失 | Metal 兼容性（`x86_64-apple-darwin`） | [3.2](#32-其他-mac)               |
| 双显卡 MacBook                       | 缺失 | P0-18（切换显卡）                     | [3.2](#32-其他-mac)               |
| Linux GNOME Wayland                  | 缺失 | P0-03、P0-16、P0-18                   | [4.1](#41-linux-桌面)             |
| Linux KDE Wayland                    | 缺失 | P0-03、P0-16                          | [4.1](#41-linux-桌面)             |
| Linux KDE 或 Xfce X11                | 缺失 | P0-03、P0-16（XIM）                   | [4.1](#41-linux-桌面)             |
| Linux fcitx5                         | 缺失 | P0-16                                 | [4.1](#41-linux-桌面)             |
| Linux ibus                           | 缺失 | P0-16                                 | [4.1](#41-linux-桌面)             |
| Linux Wayland 分数缩放（125%、150%） | 缺失 | P0-18                                 | [4.1](#41-linux-桌面)             |
| 无 GPU 直通的 Linux 虚拟机           | 缺失 | P0-17                                 | [4.1](#41-linux-桌面)             |
| Linux 物理机（真实 GPU）             | 缺失 | P0-03、P0-21                          | [4.1](#41-linux-桌面)             |
| Linux 编译与无界面测试（容器）       | 可用 | 编译检查、无界面测试                  | [4.2](#42-linux-编译与无界面测试) |
| SSH 测试主机                         | 可用 | P0-14、P1-12、N-01、SFTP 与 sudo 测试 | [5](#5-ssh-测试主机)              |

### 1.1 对 P0 停止条件的影响

- **P0-16 Linux 输入法矩阵**：四个组合全部缺少环境，整张卡为“未验证”。它是 P0 的停止条件之一，G1 时需要决策人决定：补建 Linux 桌面环境后再验证，或者接受 Linux 输入法未验证进入 P1。
- **P0-17 无 GPU 与远程桌面**：Windows 与 Linux 的无 GPU 环境、RDP、云桌面全部缺失，同样是停止条件之一。本机可以在不增加硬件的情况下补上 Windows 部分，见 2.2 节。
- **P0-03 Linux WebKitGTK 渲染路径**、**P0-18 的 Linux 项**：缺少 Linux 桌面，为“未验证”。容器只能做编译和无界面测试，不能代替。

## 2. Windows

### 2.1 Windows 11 物理机

- 状态：可用。即日常开发机，tTerm 主仓库在 `D:\code\rust\tTerm`，迁移任务的 worktree 在同级目录 `D:\code\rust\tTerm-<任务ID>`。
- 系统：Windows 11 Pro，版本 10.0.26300（build 26300）。
- 硬件：MSI MS-7D42 主板，Intel Core i7-12700K，64 GB 内存。
- 显卡：NVIDIA GeForce RTX 4060 Ti（驱动 32.0.16.1714）与 Intel UHD Graphics 770 核显（驱动 32.0.101.7088）。显示器接在哪块卡上未记录；把显示器接到主板输出可以测试核显，不是必需项。
- 显示器：两块 27 英寸、2560×1440 的显示器（AOC Q27G2G3R4B、Redmi 27 NU），其中一块竖放（1440×2560）。系统缩放均为 100%（96 DPI）。
  - P0-18 的“不同缩放的两块显示器”：测试时把其中一块临时改为 125% 或 150%，测完改回，在报告中写明。
- 输入法：微软拼音、微信输入法（WeType），以及英文键盘。
- 工具：rustc / cargo 1.99.1（与 `rust-toolchain.toml` 一致），node v24.19.0，pnpm 11.15.0，git 2.56.0，PowerShell 7.6.9，Windows 自带 OpenSSH 与 `mstsc`，Claude Code 2.1.319，Codex CLI 0.167.0。
- 虚拟化：系统已启用虚拟机监控程序（`HypervisorPresent = True`）；装有 VMware Workstation（`C:\Program Files (x86)\VMware\VMware Workstation`），没有 VirtualBox。Hyper-V 功能是否启用未核实，需要管理员权限查看（`Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All`）。
- 已安装的 tTerm：正式版 0.8.1（P0-15 报告记录）。
- 截图与自动化：
  - GPUI 在 Windows 上没有无头渲染器（`current_headless_renderer` 在 Windows 返回 `None`，见 P0-10 报告），截图要打开真实窗口后用 `Window::render_to_image` 读回纹理。
  - Tauri 开发版可以经 WebView2 的调试端口（CDP）驱动，用法见 P0-15 报告“偏差”一节。
- 性能测试：vtebench 位于 WSL Ubuntu 的 `/home/stone/vtebench`，提交 `ead80032e57dee2e75f0b51f2ea67528647d9944`。Rust 工具链由决策人准备后，在该目录编译 release；通过本地标签运行 `bash /mnt/d/code/rust/tTerm-P0-01/scripts/perf/workload.sh vtebench <绝对结果目录>`，标准负载取该检出的 `benchmarks/`。当前编译、运行未验证（WSL 缺少 Rust 工具链）。
- 注意：Tauri 开发版的数据目录 `com.stone.tTerm.dev` 与已安装的正式版 `com.stone.tTerm` 都存有真实连接，测试不得读写，见[执行规则第 7 条](README.md#2-执行规则)。

### 2.2 Windows 虚拟机

- 状态：缺失。本机没有建 Windows 虚拟机。
- 补建方法（需要时）：在本机的 VMware Workstation 或 Hyper-V 中建一台 Windows 11 虚拟机，关闭 3D 加速（VMware 取消“加速 3D 图形”，Hyper-V 不开 GPU 分区），显示适配器预期为 Microsoft Basic Render Driver（WARP），以 `dxdiag` 实际显示为准。用 `mstsc` 直接连接这台虚拟机，即可同时覆盖“无 3D 加速”与“经 RDP 访问”两行。Windows 10 22H2 另建一台。
- 建好后在本节写明 VM 名、系统版本、显示适配器、内网地址与账号名，并把总表对应行改为“可用”。

## 3. macOS

### 3.1 Mac mini M4

- 状态：可用。
- 机型：Mac mini，Apple M4（10 核）。
- 系统：macOS 27.2（26B123），arm64。
- 显示器：外接 Redmi 27 NQS，5120×2880，界面等效 2560×1440，缩放 2×（P0-28 报告）。这是 ui-parity.md 5.2 节“macOS、DPR 2”门槛使用的显示条件。
  - Mac mini 没有内建显示器，“内建 Retina 显示器”一行为缺失。
  - 没有记录外接 1× 普通 DPI 显示器。需要时可以临时借用 Windows 机器的 2560×1440 显示器，接上后在本节补记。
- 输入法：未记录。
- 仓库位置：`/Users/stone/Code/rust/tTerm`，worktree 在同级目录（例如 `/Users/stone/Code/rust/tTerm-P0-25`）。
- 工具：rustc / cargo 1.99.1，Xcode 26.6，Homebrew clang 22.1.8，node v24.20.0，pnpm 11.25.0，git 2.55.0，Playwright 1.54.0（WebKit 26.0，revision 2191），OrbStack（Docker）。
- 已知环境问题：
  - Xcode 26 的 Metal Toolchain 是可选组件，未安装时 GPUI 默认特性编译 shader 会报 `cannot execute tool 'metal'`（P0-06 报告）。
  - pnpm 11.25 在干净检出上会因 esbuild 的构建脚本未获批准而报 `ERR_PNPM_IGNORED_BUILDS`，需要在 worktree 中执行一次 `pnpm approve-builds --all`（W0-04 报告），生成的文件不提交。
  - 部分 agent 会话的沙箱不允许写 `~/.cargo`，曾用 `CARGO_HOME=/tmp/<任务>/cargo-home` 绕过（P0-06、P0-08 报告）。
- 截图：
  - agent 会话没有“屏幕录制”权限，`screencapture -x` 与 `screencapture -l` 都失败（P0-08、P0-25 报告）。GPUI 截图用无头 Metal 渲染；真实窗口截图（Tauri 窗口、窗口级模糊）由 stone 手工截取，标 `[人工]`。
  - 给终端或 Codex 授予屏幕录制权限后，可以改为自动截取真实窗口，授予后在本节补记。
- 性能测试：vtebench 未克隆。P0-01 开始前克隆，并把路径填在这里。

### 3.2 其他 Mac

- Intel Mac：缺失。
- 双显卡 MacBook：缺失。

## 4. Linux

### 4.1 Linux 桌面

- 状态：缺失。没有 Linux 物理机，也没有 Linux 桌面虚拟机，总表中所有 Linux 桌面、输入法、分数缩放与无 GPU 的行都是缺失。
- WSLg 不能代替 Linux 桌面：它跑在 Weston 上，输入法与合成器都不代表真实桌面，不计入本表。
- 补建方法（需要时）：在 Windows 机器的 VMware Workstation 中建两台虚拟机即可覆盖 P0-16 的四个组合：
  - A：Ubuntu 26.04 Desktop，GNOME Wayland（text-input-v3），可开启 125%、150% 分数缩放；
  - B：Xubuntu 26.04 或 Debian 13 Xfce，X11（XIM）；
  - 两台都装 fcitx5 与 ibus，测试时切换；关闭 3D 加速时渲染器为 llvmpipe，可同时作为“无 GPU 直通的 Linux 虚拟机”。实际渲染器以 `glxinfo -B` 或 `vulkaninfo --summary` 的输出为准。

### 4.2 Linux 编译与无界面测试

- 状态：可用，只用于编译和不需要窗口的测试，不能验证任何界面、输入法或渲染项。
- Mac 上的 OrbStack：Docker 中的 Ubuntu 24.04 arm64，P0-06、P0-08 用它做过 Linux 编译（见 P0-08 报告“Linux（Docker，Ubuntu 24.04 arm64）”一节）。
- Windows 上的 WSL：见第 5 节，可以编译 x86_64 Linux 目标。
- CI 的 Linux 任务见 `.github/workflows/check.yml`。

## 5. SSH 测试主机

- 状态：可用。
- 宿主：Windows 机器上的 WSL 2 发行版 `Ubuntu`（Ubuntu 26.04.1 LTS，内核 6.6.87.2-microsoft-standard-WSL2），rootless podman 5.7.0，`docker` 是 podman 的别名。
- 地址：容器端口从 Windows 访问要用 WSL 的 IP（2026-11-07 为 `172.27.149.142`，WSL 重启后可能变化，用 `wsl.exe -d Ubuntu -- hostname -I` 查看），不是 `127.0.0.1`。
- 已有镜像：`ubuntu:26.04`、`ubuntu:24.04`、`fedora:43`、`alpine:3.22`、`centos:6`。
- sudo 测试主机：`scripts/sudo-test-host/Containerfile`（sudo 1.9；`SUDO_RS=1` 时为 ubuntu:26.04 的 sudo-rs）。用法：`TTERM_SUDO_TEST_HOST=<ip>:<端口> cargo test --lib -- --ignored real_sudo`。
- WSL 中已装：tmux、vim、neovim、htop、python3、zsh；未装：fish、fcitx5、ibus、Claude Code、Codex。N-01 的 SSH 续接需要在测试主机上安装并登录 Claude Code 与 Codex，`[人工]`。
- Mac 上也可以起测试主机（2026-11-10）：OrbStack 的 Docker 构建同一个镜像，`docker build -f scripts/sudo-test-host/Containerfile -t tterm-sudo-host scripts/sudo-test-host`，`docker run -d --name <名> -p 127.0.0.1:<端口>:22 tterm-sudo-host`，从本机用 `127.0.0.1:<端口>` 访问。本机没有开 sshd。
- 从 Windows 调用 WSL 的注意事项：`wsl.exe -d Ubuntu -- bash -c '...'` 会弄坏 `$变量` 与 `$(...)`，把脚本写成文件再用 `wsl.exe -d Ubuntu -- bash /mnt/...` 运行；在 Git Bash 中调用时加 `MSYS_NO_PATHCONV=1`。
- P0-14、P0-15 中经 `wsl.exe` 运行的场景走的是 Windows ConPTY，不等于 Linux 原生终端环境。

## 6. 变更记录

| 日期       | 变更                                                                                |
| ---------- | ----------------------------------------------------------------------------------- |
| 2026-11-07 | 首次建立：Windows 物理机与 SSH 测试主机由本机采集，Mac 取自各卡报告；Linux 桌面缺失 |
| 2026-11-08 | P0-01：登记 vtebench 最终 WSL 路径、提交与用法；性能采集条件与结果见任务报告        |
