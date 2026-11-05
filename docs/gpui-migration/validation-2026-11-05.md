# GPUI 迁移可行性验证

> 验证日期：2026-11-05。验证对象：[迁移计划](../gpui-migration-plan.zh-CN.md)与[任务清单](README.md)。
> 分支：`GPUI`，从 `dev` 切出；`HEAD` 与 `origin/dev` 同为 `5a7e90b`。
> 本报告只记录本次实际核对或编译验证过的内容。没有条件验证的部分集中列在第 8 节，不能按通过处理。

## 1. 结论

**技术路线成立，可以迁移；但现有证据还不足以支持直接进入全面迁移。**

三点判断：

1. **路线成立。** tTerm 的业务核心确实在 Rust 里，终端模型可以近乎零改动地从 nyaterm 抽出来，GPUI 也能在本机编译并跑通无界面测试。这不是从零写终端，也不是简单换掉 Tauri。
2. **最关键的复用点已经实证。** `nyaterm-terminal`（12,941 行）抽出来后只补了 242 行 `nyaterm-core` 代码，原有 **255 个测试全部通过**。这条是整个计划的成本基础，现在有了证据，而不只是读代码的推断。
3. **但迁移的前提问题没有回答。** 迁移的理由是 macOS 与 Linux 的性能，而现有 Tauri 版本慢在哪里还没有测量。如果瓶颈是 macOS 的透明窗口与模糊，先修 Tauri 的收益远大于迁移。计划里的 G0 必须先做，不能跳过。

| 问题                         | 结论                            | 依据                                          |
| ---------------------------- | ------------------------------- | --------------------------------------------- |
| 文档里的代码统计是否可信     | 可信，误差来自 11-01 之后的提交 | 第 3 节逐项核对                               |
| 上游固定提交与许可证是否真实 | 真实                            | 第 4 节，GitHub API                           |
| GPUI 能否在本机构建并测试    | 能                              | 5.1、5.2 节                                   |
| nyaterm 终端模型能否抽取     | 能，耦合很低                    | 5.3 节，255 个测试通过                        |
| 终端绘制层能否复用           | 结构上可行，未编译验证          | 5.4 节，129 处调色板引用集中在 10 个文件      |
| 后端核心能否脱离 Tauri       | 结构上可行，未编译验证          | 62% 的 Rust 行数位于不引用 `tauri::` 的文件中 |
| 是否存在阻塞性平台风险       | 有两个，且都还没排除            | 第 6 节：Linux 输入法、无 GPU 环境            |
| 工作量估算是否可信           | 计划已自我修正，90–120 人周可信 | 第 7 节                                       |

一句话建议：**按计划先做 G0 剖析与 P0 的平台验证，再决定是否全面迁移；不要先动 P1 的重构。**

## 2. 验证环境与方法

本机：macOS（Apple Silicon），`rustc 1.99.1`、`cargo 1.99.1`、Xcode 26.6、Homebrew clang 22.1.8。

方法分三类：

1. **静态核对。** 用 `git`、`grep`、`find`、`python3` 在 `dev@5a7e90b` 上重新统计文档里的数字与路径。
2. **上游核对。** 用 GitHub API 核对固定提交、许可证、issue 状态与 nyaterm 的发布状态。
3. **编译实证。** 两个独立 spike：
   - GPUI 最小程序 + 无界面测试（`TestAppContext`）；
   - 从固定提交抽出 `nyaterm-terminal`，用最小 `nyaterm-core` 替代后编译并跑测试。

所有临时工程放在 `/tmp`，没有写入仓库。

## 3. 文档与本仓库的核对

统计口径与文档一致：行数包含注释与空行，前端不含测试文件另行标注。

| 指标                        | 文档                  | 实测                          | 判断                     |
| --------------------------- | --------------------- | ----------------------------- | ------------------------ |
| 前端文件（TS/TSX/CSS）      | 783                   | 793                           | 一致，有新增文件         |
| 前端行数                    | 约 10.7 万            | 108,728                       | 一致                     |
| Rust 文件                   | 337                   | 341                           | 一致                     |
| Rust 行数                   | 约 8.5 万             | 85,354                        | 一致                     |
| 引用 Tauri API 的 RS 文件   | 112                   | 113                           | 一致                     |
| `#[tauri::command]`         | 209                   | 210                           | 一致                     |
| `AppHandle` 出现次数        | 约 344                | 350                           | 一致                     |
| `app.manage(...)`           | 20                    | 21                            | 一致                     |
| 全局 `static`               | 28                    | 38（含 2 个测试计数器）       | 口径不同，需注明         |
| SFTP 文件 / 行数 / 耦合     | 53 / 约 1.4 万 / 19   | 53 / 13,837 / 19              | 完全一致                 |
| 各模块 `.emit` 次数         | 见 P1 文档            | 除 `background` 5 对 4 外全对 | 一致                     |
| `TerminalTab`（不含测试）   | 约 1.15 万            | 11,534                        | 完全一致                 |
| `SettingsDialog`            | 约 16,200             | 16,201                        | 完全一致                 |
| `SftpDrawer`                | 约 8,500              | 9,294                         | 有增长（迁移文档后新增） |
| `TTermApp` / `TabBar`       | 约 5,200 / 1,500      | 5,985 / 2,021                 | 有增长                   |
| `contexts` / `hooks` / `ui` | 1,800 / 1,700 / 2,200 | 1,778 / 1,763 / 2,393         | 一致                     |
| i18n 文案                   | 约 2,800 键           | en 2,830 / zh 2,799           | 一致（复数见下）         |

结论：文档不是估算出来的，是从真实代码量出来的。P0 与 P1、P2 中引用的文件路径（`core/state.rs`、`terminal/io_batcher.rs`、`config/paths.rs`、`drag_out/`、`session_log/`、`scripts/prepare-conpty.ps1` 等）逐个存在；`ChannelContext` 也确实直接持有 `tauri::AppHandle`（`src-tauri/src/core/state.rs:70`），`io_batcher.rs` 的约 1 MiB 未确认上限与文档描述一致。

唯二需要修改的是统计口径：

- 全局 `static` 按 `^\s*static [A-Z_0-9]+:` 统计是 38 个，文档写 28 个，应说明统计范围（例如是否排除测试计数器与模块内缓存）。
- `TabBar`、`TTermApp`、`SftpDrawer` 在 11-01 之后有增长，任务卡里的行数需要在开工前重新核对。文档第 2 节执行规则第 8 条已经要求这样做。
- **i18n 的复数键不能直接比键集合。** en 有 160 个带 `_one`/`_other` 后缀的键（约 80 组），zh 只用 `_other`，因为中文没有单复数。P3-03 的验收写成“两个语言文件的键集合相同”会在今天的数据上直接失败；测试需要先归一化复数后缀，或者只比较基础键。

另外两处依赖差异 P0-06 需要一起核对，文档没写到：

- **TLS 后端不同。** tTerm 的 rustls 0.23 启用 `ring`，nyaterm 启用 `aws_lc_rs`。rustls 0.23 在同一构建里只能选一个 provider，合并 workspace 时必须统一。
- **portable-pty 来源不同。** nyaterm 固定的是 `nyakang/portable-pty@1ececcf9`，tTerm 用 crates.io 的 0.9。终端模型 crate 不依赖它，但 GPUI 端接 PTY 时会遇到。

## 4. 上游核验

### 4.1 固定提交与仓库

| 来源                | 固定提交   | 是否存在 | 说明                                                      |
| ------------------- | ---------- | -------- | --------------------------------------------------------- |
| `nyakang/nyaterm`   | `1521b6b7` | 是       | 2026-11-01，Apache-2.0，仍在活跃开发（2026-11-05 有推送） |
| `nyakang/zed`       | `8e5a53c0` | 是       | 2026-10-29，`gpui` 包版本 0.2.2，Apache-2.0               |
| `nyakang/gpui-kit`  | `c0ca1511` | 是       | 2026-10-29，与 Zed 的 Windows 输入法修复对齐              |
| `nyakang/alacritty` | `3b60b2ed` | 是       | 2026-10-19，记录上游 rebase 与验证                        |

nyaterm 已发布 `v2.0.0`、`v2.0.1`、`v2.0.2`，旧 Tauri 主线在 `archive/tauri-v1` 分支。计划中“nyaterm 自己完成过一次 Tauri 到 GPUI 的迁移”属实，这是本项目最直接的风险清单来源。

### 4.2 许可证

| 组件                | 文档结论         | 核对结果                                                                                                                   |
| ------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Zed `gpui`          | Apache-2.0       | 是，固定提交的 `Cargo.toml` 明确 `Apache-2.0`                                                                              |
| Zed `terminal`      | GPL-3.0-or-later | 是，不能复制或链接                                                                                                         |
| Zed `terminal_view` | GPL-3.0-or-later | 是，不能复制或链接                                                                                                         |
| Zed `gpui_tokio`    | Apache-2.0       | 是                                                                                                                         |
| nyaterm workspace   | Apache-2.0       | 是，`[workspace.package] license = "Apache-2.0"`                                                                           |
| gpui-kit            | Apache-2.0       | **仓库有 `LICENSE-APACHE`，但 `Cargo.toml` 没有 `license` 字段，crates.io 元数据也是空**。复用前必须补登记并确认 SPDX 结论 |

### 4.3 issue 状态（2026-11-05 查询）

计划写作时（11-01）的状态已经变化，风险判断要跟着更新：

| issue | 计划中的状态 | 现在                  | 对判断的影响                                                              |
| ----- | ------------ | --------------------- | ------------------------------------------------------------------------- |
| #1017 | 未关闭       | 仍 open               | Linux fcitx 切回应用后输入失效，风险仍在                                  |
| #1073 | 未关闭       | 仍 open               | X11 XIM 出错后永久失效，风险仍在                                          |
| #1043 | 未关闭       | 仍 open               | 回滚区写满后的闪烁，风险仍在                                              |
| #938  | 未标注       | closed，`not_planned` | **不是修复**：无 GPU 的 Citrix 云桌面问题被明确放弃，风险比文档写的更严重 |
| #1020 | 未标注       | closed，`duplicate`   | 合并到 #938，问题依旧                                                     |
| #985  | 未标注       | closed，completed     | 滚动丢失已修复                                                            |
| #923  | 未标注       | closed，completed     | 滚动不连续已修复                                                          |
| #1093 | 未关闭       | closed，completed     | Wayland 标题重复标记完成（维护者称下个版本引入修复）                      |

这条更新很关键：**无 GPU 与远程桌面场景不是“可能退化”，而是上游明确不打算修**。tTerm 的用户常在虚拟机、堡垒机、云桌面里工作，必须自己兜底，或者接受不支持。

### 4.4 版本陷阱

`gpui` 在 crates.io 上的 **0.2.2 发布于 2025-10-22**，而 nyaterm 固定的 fork 也是 `version = "0.2.2"`，但代码是 **2026-10-29** 的。两者同名不同代：

- crates.io 0.2.2 的构建脚本还在用 bindgen 生成 dispatch 绑定、用 `xcrun metal` 预编译 Metal shader；
- 固定 fork 的 `build.rs` 已经去掉这两步，改用 objc2 绑定；
- fork 有 `gpui_wgpu` crate，crates.io 0.2.2 用的是 blade。

结论：**P0-06、P0-08 必须按 `pins.md` 使用 git rev，不能写 `gpui = "0.2.2"`。** 5.1 节只证明 crates.io 的旧版能编译；5.2 节证明固定 fork 也能编译，两者不是一份代码。

## 5. 编译实证

### 5.1 GPUI 最小程序与无界面测试（crates.io 0.2.2）

在 `/tmp` 建立最小 crate，依赖 `gpui = { version = "0.2.2", features = ["test-support", "runtime_shaders"] }`，内容是一个窗口加一个 `Render` 视图。

结果：

- 依赖树 1,269 行，debug 二进制 32 MB，全量依赖编译约十分钟；
- `cargo test --test headless` 通过，`#[gpui::test]` + `TestAppContext` 可以在无界面环境下构建视图、读取 Entity 状态。

过程中遇到两个环境问题，值得写进 W0 与 P0-08：

1. 本机 clang 来自 Homebrew，bindgen 找不到 SDK 头，报 `dispatch/dispatch.h` 缺失。设置 `SDKROOT=$(xcrun --show-sdk-path)` 与 `BINDGEN_EXTRA_CLANG_ARGS` 后通过。
2. 默认特性需要 `xcrun metal` 编译 shader，而 Xcode 26 的 Metal Toolchain 是可选组件，未安装时报 `cannot execute tool 'metal'`。用 `runtime_shaders` 特性绕过后通过。

这两条只影响 crates.io 的旧 0.2.2；固定 fork 的 `build.rs` 不含这两步，它的特性表里也没有 `runtime_shaders`。但它们说明一件事：**GPUI 的构建环境比 Tauri 挑剔**，CI 与开发机都需要显式准备，不能假设“有 Rust 就能编译”。

### 5.2 固定 fork 的编译

在 `nyakang/zed@8e5a53c0` 的浅克隆上执行 `cargo check -p gpui`（macOS arm64，rustc 1.99.1，工作区 411 个成员），**通过**：依赖解析与下载约 10 分钟，编译 8 分 12 秒。

这个结果和 5.1 对照，正好说明版本陷阱是真的：

- 固定 fork 的 `build.rs` 不再调用 bindgen 与 `xcrun metal`，所以 5.1 的两个环境坑在固定版本上不存在，不需要装 Metal Toolchain；
- fork 自带的 `rust-toolchain.toml` 固定 1.98.1，本次移除后用 1.99.1 也能编译；
- fork 的平台层已经拆成 `gpui_apple`、`gpui_linux`、`gpui_macos`、`gpui_windows`、`gpui_wgpu` 等 crate，`gpui_linux` 通过 `gpui_wgpu` 渲染，与计划 3.3 节的描述一致。

限制：这里只做了 `cargo check`，没有链接、没有打开窗口，也只验证了 macOS。三平台的真实构建与运行仍属于 P0-08 的验收。

### 5.3 抽出 `nyaterm-terminal`

做法：把固定提交的 `crates/nyaterm-terminal` 整体复制到独立 workspace，用一个最小 `nyaterm-core` 替代原依赖。替代代码共 242 行：

- `character_encoding.rs` 167 行，原样复制；
- `agent_capture.rs` 75 行，原样复制；
- 一个 6 个变体的 `AiExecutionProfile` 枚举。

结果：

- `cargo check --lib` 通过；
- `cargo test -p nyaterm-terminal --lib`：**255 个测试全部通过**，1.24 秒；
- 依赖的 Alacritty 分支解析出的 `vte` 是 0.15.0，与 tTerm 现有的 `vte 0.15` 相同，不存在版本冲突。

这验证了计划 3.2 节的核心判断：终端模型本身的耦合只有字符编码与 AI 命令捕获两项（后者只出现在测试里）；关键词规则与单元格高度属于绘制层，不在模型 crate 内。前三项都能用极小的替代物接上，P0-09 的成本估算（M，验证分支版本）是可信的。

### 5.4 没有做的 spike

- **终端绘制层**（`nyaterm-terminal-gpui`，15,523 行）：没有编译。它依赖 `nyaterm_ui::ThemePalette`（129 处引用，分布在 10 个文件），替换量有界，但需要真正接一次 GPUI 才能确认。这是 P0-10 的工作。
- **tTerm 后端核心**：没有做抽取编译。静态证据是 62%（52,760 / 85,354）的 Rust 行数位于不引用 `tauri::` 的文件中，P1 的拆分是可行的，但 210 个 command 与 350 处 `AppHandle` 的改造量是真实的。

## 6. 风险与门槛

| 风险               | 证据                                      | 等级 | 处理                                           |
| ------------------ | ----------------------------------------- | ---- | ---------------------------------------------- |
| Linux 输入法       | #1017、#1073 仍 open                      | 阻塞 | 先做 P0-16 四组合矩阵；不通则不迁移 Linux      |
| 无 GPU / 远程桌面  | #938 `not_planned`、#1020 duplicate       | 阻塞 | 先做 P0-17；上游不修，只能自研降级或明确不支持 |
| Windows 输入法补丁 | 固定 fork 的关键提交就是 Windows IME 修复 | 高   | 自管分支，按 U-01 跟踪；不能只依赖上游         |
| 文件拖出           | 上游没有，仅在 nyaterm 分支               | 高   | P0-19 先做方案对比，三平台各自验证             |
| 终端协议双应答     | tTerm 后端已有 `ssh_query_handler`        | 中   | P2-12 明确唯一回复方，补测试                   |
| GPL 边界           | `terminal` / `terminal_view` 是 GPL-3.0   | 中   | 已规避；复用清单要落实到文件与提交             |
| 上游漂移           | nyaterm 三天内有推送，fork 落后上游       | 中   | 固定 rev + 每月 U-01，补丁记录来源与回归用例   |
| 数据兼容           | 应用标识、数据库、钥匙串、备份格式        | 中   | P1-10 与 P5-04；localStorage 必须先迁回后端    |
| 工作量             | 任务卡合计 450–600 人天                   | 高   | 按 90–120 人周立项；P0 后重新估算              |
| 双版本维护         | 迁移期间 Tauri 版仍在开发                 | 中   | 业务逻辑先进 core，避免两边各写一遍            |

两个阻塞项都不是“写代码能解决”的，而是**平台与上游的客观限制**。这正是 P0 存在的意义。

## 7. 工作量判断

计划自己已经修正过一次：最初 20–35 人周，按任务卡求和改为 90–120 人周。本次核对支持后者：

- 前端需要重写的界面代码约 10.9 万行，对应 166 个测试文件（18,959 行）作为行为规格；
- nyaterm 的 GPUI 桌面层是 **354,600 行**（其中测试 39,214 行），可复用的两个终端 crate 只有 28,464 行；
- tTerm 的后端有 85,354 行，其中约 6 成可以保留，但仍需 210 个 command 的变薄与 350 处 `AppHandle` 的解耦。

所以“终端 MVP 6–10 周、整体一人 5–9 个月”是下限，不是承诺。两个人的分工也不会把关键路径减半。

## 8. 未验证项

以下内容本次没有条件验证，报告中不作为通过：

- 固定 fork 在 Linux 与 Windows 上的编译与运行（macOS 已通过 `cargo check`，未链接、未运行）；
- 真实窗口的绘制、输入法、候选框、DPI 与多屏行为；
- 任何性能数字（迁移动机本身）；
- 无 GPU、RDP、Citrix 环境的实际表现；
- 文件拖出、托盘、通知、单点登录、更新器、签名与公证；
- 终端绘制层的实际抽取；
- 真实 SSH/SFTP/串口/ZMODEM 会话；
- 长稳、内存与能耗。

## 9. 建议的下一步

按顺序，不要跳步：

1. **先做 G0（1–2 天）。** 在 macOS 上对比“透明 + 模糊”“只透明”“都不开”三种构建的滚动帧耗时、空闲 CPU 与 WindowServer 占用；在 Linux 上记录 WebKitGTK 版本与实际渲染器。结论可能是“只需修 Tauri”，那就先修。
2. **P0-06 固定 fork 并编译。** 用 `pins.md` 的 rev，不用 crates.io；macOS 已在本报告通过 `cargo check`，还需要补 Linux、Windows 与真实链接，并把系统依赖写进 CI。
3. **P0-09 / P0-10 的最小 spike。** 终端模型已经证明可抽；下一步把绘制层接到本地 PTY，先跑 `vim`、`tmux`、`htop`，再谈功能对齐。
4. **平台门槛先测。** Linux 输入法四组合、无 GPU 环境，这两项决定项目是否继续，要排在最前面，不要留到 P2 之后。
5. **P1 的重构可以并行判断。** 核心解耦对 Tauri 版本身也有价值，即使不迁移也值得做；但不要在 G0/G1 之前大规模动 `ssh`、`sftp`。
6. **迁移期间 Tauri 版保持可用。** 本文验证的都是旁路 spike，没有改动 `dev` 的行为。

## 10. 复现命令

```sh
# 分支与基线
git rev-parse --abbrev-ref HEAD        # GPUI
git rev-parse HEAD origin/dev          # 同为 5a7e90b

# 规模统计
find src -type f \( -name '*.ts' -o -name '*.tsx' -o -name '*.css' \) | wc -l
find src-tauri -name '*.rs' -not -path '*/target/*' -exec cat {} + | wc -l
grep -rho '#\[tauri::command' src-tauri/src --include='*.rs' | wc -l
grep -rl 'tauri::' src-tauri/src --include='*.rs' | wc -l

# Tauri 耦合
grep -n -A6 'pub struct ChannelContext' src-tauri/src/core/state.rs

# 上游
curl -sS https://api.github.com/repos/nyakang/nyaterm/commits/1521b6b7
curl -sS https://api.github.com/repos/nyakang/zed/commits/8e5a53c0

# 终端模型抽取（临时工程）
cd /tmp/nya-extract && cargo test -p nyaterm-terminal --lib
```

固定 fork 的编译命令见 5.2 节。
