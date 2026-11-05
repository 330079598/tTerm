# W0 准备与 P0 验证

本阶段回答五个问题：现有版本慢在哪里；在官方 Alacritty 之上能否搭出可用的终端（2026-11-05 起不再整体复用 nyaterm 终端，见[决策 007](decisions/007-official-sources-and-terminal-route.md)）；真实会话是否可用；关键平台是否可以接受；收益是否值得继续投入。对应[迁移计划](../gpui-migration-plan.zh-CN.md)第 6.0 节与 P0。

本阶段的原型代码允许走捷径（例如直接调用 portable-pty），但必须放在 `tterm-gpui` 中，并在报告里列出所有捷径，P1 之后逐一替换。

**G1 必需与可推迟。** 回答 G1 只需要：W0-01 至 W0-04、W0-05 中 P0-24 用到的屏幕、P0-01 至 P0-06、P0-08、P0-09、P0-10、P0-13 至 P0-18、P0-21、P0-22、P0-27、P0-28，以及 2026-11-05 新增的 P0-24（[ui-parity.md](ui-parity.md#p0-24-ui-原语还原验证)）、P0-25（[ui-parity.md](ui-parity.md#p0-25-gpui-绘制能力核实)）、P0-26（[tech-selection.md](tech-selection.md#p0-26-打包与更新衔接可行性)）。P0-07、P0-11、P0-12、P0-19、P0-20、P0-23、P0-29 可以推迟到 G1 之后，但必须在依赖它们的卡之前完成：P0-12 在 P2-06 之前，P0-11 在 P2-24 之前，P0-19 在 P4-05d 之前，P0-20 在 P4-07 之前，P0-07 在 P0-19 之前，P0-23 在 P2 与 P3 开始之前，P0-29 在 P2-16 之前。

**验证分支与大小限制。** 自写的终端代码（P0-09、P0-10、P0-12、P0-13）从第一天起遵守全部仓库规则，按正常流程合回 `GPUI`。只有原样复制的第三方代码（P0-28 的 Alacritty 应用层、P0-29 的 nyaterm 模块）可以先放在不合并的验证分支 `migrate/spike`（从 `GPUI` 切出）上，暂不拆分；该分支上 `pnpm check:size` 与 clippy 的长度检查可以失败，其他检查必须通过。G1 决定继续后，由 P1-13 拆分并合并。

## W0 准备

### W0-01 建立 Cargo workspace 并让检查覆盖新 crate

- 依赖：无
- 规模：M
- 并行：W0-02、W0-03
- 参考：`src-tauri/Cargo.toml`、`src-tauri/clippy.toml`、`scripts/check-file-size.mjs`、`.github/workflows/check.yml`、`AGENTS.md`
- 工作（2026-11-05 修订：workspace 根改到仓库根目录，见[技术选型第 2.1 节](tech-selection.md#21-workspace-布局修订-readme-约定-a1)）：
  1. 在仓库根新建 `Cargo.toml`，`[workspace]` 成员为 `src-tauri` 与 `crates/*`。把 `src-tauri/Cargo.toml` 的 `[lints.clippy]` 与 `[profile.*]` 提升到根，现有包与新 crate 均使用 `lints.workspace = true`。`clippy.toml` 移到根目录，确认对所有成员生效。`Cargo.lock` 用 `git mv` 移到根目录，单独提交。
  2. 创建空的库 crate `crates/tterm-core`、`crates/tterm-terminal`、`crates/tterm-platform`，每个只有 `lib.rs` 与说明职责的模块文档注释。其余 crate 在用到它们的卡中创建。
  3. `scripts/check-file-size.mjs` 的扫描范围增加 `crates/`，豁免规则保持不变。
  4. CI 与 AGENTS.md 的命令改为在仓库根执行 `cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo test --workspace`；发布工作流中的 `src-tauri/target` 改为根目录的 `target`。
  5. 加入 `rust-toolchain.toml`，CI 改为读取它。
  6. 确认 Tauri 版构建、打包与 `pnpm tauri dev` 不受影响。
- 产出：workspace 配置、三个空 crate、更新后的检查脚本、CI 与 AGENTS.md。
- 验收：
  - 三个平台的 CI 全部通过（可在推送后用 `gh workflow run check.yml --ref <分支>` 手动触发）。
  - 发布流程不为本卡单独发版：手动触发 `release-dev-beta.yml` 会向 beta 用户真实发布。改为在本卡合入 `dev` 后的下一次正常 beta 发布中验证各平台安装包与 `latest.json` 齐全，由评估者核对。
  - `Cargo.lock` 移动前后锁定的版本完全相同。
  - 在新 crate 中临时放入超过 800 行的文件时 `pnpm check:size` 失败；放入超过 150 行的函数时 clippy 失败。报告附两次失败输出，临时文件不提交。
  - `git diff` 中 Tauri 包的源码没有变化。
- 评估重点：`Cargo.lock` 是否因 workspace 化而意外升级了依赖；`target/` 位置变化是否影响打包脚本。
- 不做：移动任何现有代码。

### W0-02 迁移文档骨架与第三方登记

- 依赖：无
- 规模：S
- 工作：
  1. 创建 `docs/gpui-migration/reports/`、`docs/gpui-migration/decisions/`，各放一个说明命名规则的 `README.md`。
  2. 创建 `third-party.md`，表头为：本仓库路径、来源仓库、来源提交、来源路径、许可证、改动说明。
  3. 创建 `pins.md` 模板，由 P0-06 填写。
  4. 创建 `status.md`：每张卡一行，列为 ID、状态、执行者、分支、报告链接，初始全部为“未开始”，此后由评估者维护（README 第 8 节 U-02）。
  5. 创建 `feature-matrix.md` 模板：功能、现有实现位置、原生实现位置、状态（未开始、进行中、对齐、明确放弃）、验收证据。P4-27 负责填满。
- 产出：上述文件。
- 验收：文件存在，格式通过 `pnpm format:check`。
- 不做：填写实际内容。

### W0-03 三平台测试环境清单 `[人工]`

- 依赖：无
- 规模：S
- 工作：列出可用的测试机器与环境，并为每个环境写明进入方式和已安装的工具。至少覆盖：
  - macOS：Apple Silicon，Retina 与外接普通 DPI 显示器；Intel Mac（正式版同时发布 `x86_64-apple-darwin`，旧的 Intel 集成显卡是 Metal 渲染的风险点）；有条件时加双显卡 MacBook。
  - Linux：GNOME Wayland、KDE Wayland、KDE 或 Xfce X11；fcitx5 与 ibus 各一套；Wayland 分数缩放（125%、150%）；无 GPU 直通的虚拟机。
  - Windows：Windows 11 物理机、Windows 10 22H2、经 RDP 访问的 Windows、无 3D 加速的虚拟机，有条件时加 Citrix 或其他云桌面。
  - SSH 测试主机：沿用现有 WSL podman 测试主机。
- 产出：`docs/gpui-migration/environments.md`。
- 验收：每个环境标明“可用”或“缺失”。缺失的环境在后续卡中一律记为“未验证”。

## P0-A 剖析现有 Tauri 版本

### P0-01 性能基线工具与负载

- 依赖：W0-03
- 规模：M
- 并行：无，P0-02 至 P0-04 都使用它的产出
- 参考：[vtebench](https://github.com/alacritty/vtebench)（克隆到各测试机，路径记入 `environments.md`）、`src-tauri/src/terminal/io_batcher.rs`、`src/components/TerminalTab/outputAck.ts`
- 工作：
  1. 在 `scripts/perf/` 编写可重复的负载：vtebench 的标准负载；`cat` 一个 50 MB 的文本文件；`yes` 持续 30 秒；在 vim 中滚动大文件；tmux 中 4 个窗格同时输出；1、10、30 个空闲会话。
  2. 编写各平台的采集脚本，统计整个进程树（包括 WebView 子进程与 GPU 进程）的 CPU、内存，以及可获得时的 GPU 内存：macOS 用 `ps` 与 `footprint`，Linux 用 `ps` 与 `/proc`，Windows 用 PowerShell 性能计数器。
  3. 定义帧耗时的测量方式：Tauri 版使用 WebView 开发者工具的性能录制；记录方法和读数口径，原生版在 P0-21 用同一口径。
  4. 输出 CSV，列为：平台、构建、负载、指标、数值、单位、样本数、备注。
- 产出：`scripts/perf/` 中的负载与采集脚本、使用说明 `scripts/perf/README.md`、三平台的首次基线 `docs/gpui-migration/baseline/<平台>.csv`。
- 验收：
  - 同一台机器上连续运行两次，主要指标差异在 10% 以内，否则说明噪声来源。
  - 使用 release 构建，字体、窗口大小、回滚行数在报告中写明。
- 评估重点：是否遗漏 WebView 子进程；是否把 debug 构建的数据当作基线。
- 不做：修改应用代码。

### P0-02 macOS 透明窗口与模糊开销

- 依赖：P0-01
- 规模：S
- 并行：P0-03、P0-04
- 参考：`src-tauri/tauri.macos.conf.json`、`src-tauri/src/window_blur.rs`、`src-tauri/src/main_window.rs`
- 工作：
  1. 构建三个变体：现状；关闭模糊、保留透明；`transparent: false` 且关闭模糊。变体只在本地构建，不提交配置改动。
  2. 每个变体运行 P0-01 的负载，并记录 WindowServer 进程的 CPU。
  3. 用 Instruments 的 Time Profiler 与 Core Animation 工具各录一次滚动与持续输出。
- 产出：报告与数据 CSV。
- 验收：三个变体在同一台机器、同一显示器上测得；结论说明透明与模糊各自带来多少开销。
- 评估重点：外接显示器与内建 Retina 显示器的结果是否一致。
- 不做：修改默认配置。是否关闭透明作为改进 Tauri 版的建议写进报告。

### P0-03 Linux WebKitGTK 渲染路径

- 依赖：P0-01
- 规模：S
- 并行：P0-02、P0-04
- 参考：`src/components/TerminalTab/useTerminalRenderer.ts`、`src-tauri/tauri.linux.conf.json`
- 工作：
  1. 在 X11 与 Wayland 下分别记录：WebKitGTK 版本、实际启用的 xterm 渲染器（WebGL 或 Canvas）、WebGL 是否可用、是否发生过上下文丢失回退。
  2. 对比环境变量 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 与 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 的影响。
  3. 用 P0-01 的负载测量每种组合。
- 产出：报告与数据 CSV。
- 验收：至少覆盖一个 Wayland 与一个 X11 会话；说明瓶颈在渲染器、合成还是其他环节。
- 评估重点：发行版自带的 WebKitGTK 版本差异，是否需要在多个发行版上复测。

### P0-04 前端与 IPC 热点剖析

- 依赖：P0-01
- 规模：M
- 并行：P0-02、P0-03
- 参考：`src/components/TerminalTab/ptySession.ts`、`src/components/TerminalTab/outputAck.ts`、`src-tauri/src/terminal/io_batcher.rs`、AGENTS.md 中“阻塞工作”一节
- 工作：
  1. 用 React Profiler 录制：持续输出、切换标签、拖动分屏、打开设置、打开 SFTP 抽屉。列出重渲染最多的组件。
  2. 测量输出从后端发送到 xterm 解析完成的耗时，以及 ack 往返对吞吐的影响。
  3. 找出仍为同步的 `#[tauri::command]` 中做文件、数据库、钥匙串或加密工作的命令。
- 产出：热点列表，每项给出位置、影响和是否能在 Tauri 版内修复。
- 验收：每个热点附录制文件或测量数据。
- 不做：修复热点。值得修的另开卡，在 Tauri 版中进行。

### P0-05 剖析结论

- 依赖：P0-02、P0-03、P0-04
- 规模：S
- 工作：汇总三份报告，按平台写出瓶颈、在 Tauri 内可修复的部分、只有换掉 WebView 才能解决的部分，以及对迁移收益的判断。
- 产出：`docs/gpui-migration/reports/P0-05.md`，并更新迁移计划第 1 节的按平台判断表。
- 验收：每个结论都能追溯到前三份报告中的数据。

### G0 剖析结论确认 `[人工]`

2026-11-05 已决定正式迁移（[决策 001](decisions/001-formal-migration.md)），G0 不再决定是否迁移。根据 P0-05 决定：哪些问题先在 Tauri 版修复，以及 P0-21 与 G4 使用的性能基线。决定写入 `decisions/005-g0.md`。P0-B 不等待 G0。

## P0-B 技术验证

### P0-06 固定依赖组合

- 依赖：W0-01
- 规模：M
- 参考：[技术选型第 3、5 节](tech-selection.md#3-gpui-来源)、[决策 007](decisions/007-official-sources-and-terminal-route.md)、`src-tauri/Cargo.toml`、`pins.md`
- 工作（2026-11-05 修订：只用官方来源）：
  1. 在根 `Cargo.toml` 的 `[workspace.dependencies]` 中固定：`gpui-kit = "=0.9.0"`（带入 `gpui-pre =0.3.11`，即 `zed@f816e1f`）、`alacritty_terminal = "=0.26.0"`。不引入任何 git 依赖，不引入 `nyakang` 或其他个人分支。确认通过 gpui-kit 的再导出能拿到 GPUI 与 `gpui_platform` 的全部所需接口；拿不到时才直接依赖同版本的 `gpui-pre` 系列 crate，并说明原因。
  2. 与 tTerm 现有依赖对齐，至少检查：`vte`（tTerm 用 0.15）、`tokio`、`rustls` 与加密后端（tTerm 用 `ring`）、`reqwest`、`image`、`regex`、`windows`、`objc2`。用 `cargo tree -d` 列出重复依赖。
  3. **实测打补丁的退路。** 把 `gpui-pre 0.3.11` 的源码放到临时目录，改一行无害代码，用 `[patch.crates-io]` 替换后编译通过并能看到改动生效；然后撤销，不提交。报告写清步骤，供以后需要给 GPUI 打补丁时照做。
  4. 确定 Rust 工具链：取 `gpui-pre`、gpui-kit 与 `alacritty_terminal` 要求的最低版本，必要时调高 `rust-toolchain.toml`；决定 workspace 的 `resolver` 是否改为 `"3"`。
  5. 评估三平台的编译时间与 release 二进制体积；为 GPUI 等大依赖设置开发构建的优化级别，记录增量编译时间。
- 产出：`pins.md`、`decisions/002-dependency-sources.md`、重复依赖清单、打补丁步骤。
- 验收：
  - 一个只依赖 gpui-kit 与 `alacritty_terminal` 的空程序在三平台编译通过（CI 未推送时按[决策 006](decisions/006-w0-01-ci-waiver.md) 记为未验证）。
  - `Cargo.lock` 中没有任何 git 来源的依赖。
  - 与 tTerm 的 workspace 共存时没有类型不统一的重复依赖。
  - 第 3 步的打补丁退路有可复现的步骤与输出。
- 评估重点：`pins.md` 中每一项的许可在固定版本上是否仍为 Apache-2.0；gpui-kit 的 `Cargo.toml` 没有 `license` 字段，以仓库 `LICENSE-APACHE` 为准。

### P0-07 GPUI 补丁差距评估

- 依赖：P0-06
- 规模：M
- 参考：[技术选型第 3 节](tech-selection.md#3-gpui-来源)中放弃的 nyakang 补丁表；`nyakang/zed` 相对官方的 38 个提交（Apache-2.0，可以阅读和移植）；官方 Zed 自 `zed@f816e1f` 之后的提交
- 工作：
  1. 逐项记录 nyakang 补丁中 tTerm 可能需要的部分：Windows 输入法只更新光标的组合事件、图集分配失败处理、关闭对话框时恢复所属窗口、隐藏光标、macOS 光标状态。每项写：作用、对应的 tTerm 场景、官方是否已有等价实现、改动规模。
  2. 检查官方 Zed 在 `f816e1f` 之后是否有 tTerm 需要的修复（输入法、Wayland、无障碍、渲染），作为升级 `gpui-pre` 的依据。
  3. 每项给出“不需要”“等官方”“需要时按 P0-06 的退路打补丁”之一的结论。
- 产出：`docs/gpui-migration/gpui-patches.md`。
- 验收：每项都有结论和理由；结论为“需要”的项附上触发它的测试场景（例如 P0-18 的某个输入法用例）。
- 不做：实际打补丁。

### P0-08 最小 GPUI 程序

- 依赖：W0-01、P0-06
- 规模：S
- 工作：
  1. 创建二进制 crate `tterm-gpui`：打开一个窗口，显示一段中英文混排文本与一个 gpui-kit 按钮，可以正常退出。
  2. 使用 tTerm 的图标。开发期的二进制名为 `tterm-gpui`，不能叫 `tTerm`：同一 workspace 中两个同名二进制会让 Cargo 报输出文件冲突，而且 `.cargo/config.toml` 的 macOS runner 会把名为 `tTerm` 的二进制装进 Tauri 开发版的应用包（`com.stone.tTerm.dev`）。开发版使用独立的应用标识 `com.stone.tTerm.gpui-dev`、显示名 `tTerm GPUI Dev` 与数据目录（执行规则第 7 条），数据目录在 P1-01 之后才接入。改名为 `tTerm` 在 P5-02 进行。
  3. macOS 开发运行方式：扩展 `scripts/macos-dev-bundle.sh`，让它对 `tterm-gpui` 也生效，但使用上面的 GPUI 开发版标识与显示名，不读 `TAURI_CONFIG`；`Info.plist` 带上 `src-tauri/Info.plist` 中的 `NSLocalNetworkUsageDescription`。没有它，macOS 会拦截到局域网主机的连接，系统通知也需要应用包。验收时确认 `pnpm tauri:dev` 的行为不变。
  4. 在 CI 中为三平台增加 `tterm-gpui` 的构建任务。Linux 需要安装 GPUI 的系统依赖，写进 CI 与说明文档。
  5. 接通 GPUI 的测试支持（`gpui` 的 `test-support` 特性），写一个打开窗口、发送动作并断言状态的样例测试，确认它在三平台 CI 上无界面运行。
- 产出：`crates/tterm-gpui/`、CI 改动、`tterm-gpui/README.md`（开发环境依赖与运行方式）。
- 验收：三平台 CI 构建与样例测试通过；本地在 macOS、Windows、Linux 至少各运行一次并截图；macOS 应用包能连到局域网内的 SSH 主机。
- 不做：任何业务功能。

### P0-09 终端模型（官方 Alacritty）

- 依赖：P0-06、P0-27
- 规模：L
- 参考：P0-27 的设计说明；官方 `alacritty_terminal 0.26.0` 的文档；`src-tauri/src/terminal/`（现有的输出批处理与查询应答）；**不得阅读 Zed 的终端源码**（技术选型第 5 节的净室规则）
- 工作：
  1. 在 `crates/tterm-terminal` 中封装 `alacritty_terminal::Term`：写入字节、resize、回滚容量、可见区域快照（行、单元格属性、光标、选区）、变更行的追踪，以及标题、响铃、剪贴板、需要回写给 PTY 的应答等事件。
  2. 快照对绘制层只读，绘制不长期持有解析需要的锁（计划 7.3 节）。
  3. 搜索与选区先用 Alacritty 自带的能力，接口按 P2 的需要设计。
  4. 终端模型只接收 UTF-8。GBK 等会话编码由现有的 `src-tauri/src/terminal/encoding.rs` 在写入终端前转换（P1 移入 core），模型不重复实现。
  5. 抗崩溃测试：用随机字节流和手写的边界序列持续写入（例如 proptest），不得出现 panic。
- 产出：`tterm-terminal` 及其测试。
- 验收：
  - 全部检查通过，包括大小限制，不新增 `allow` 或 `expect`。
  - `tterm-terminal` 不依赖 GPUI、Tauri。
  - 抗崩溃测试至少运行 10 万组随机输入无 panic，报告给出种子与运行时间。
  - 报告声明执行者未阅读 Zed 终端源码。
- 评估重点：快照的拷贝量是否与可见行数成正比，而不是与回滚区成正比。
- 不做：绘制与输入。

### P0-10 终端绘制元素

- 依赖：P0-09、P0-27、P0-25
- 规模：L
- 参考：P0-27 的设计说明；GPUI 的 `Element` 与文字排版接口；`src/lib/terminalPalette.ts`；**不得阅读 Zed 的终端源码**
- 工作：
  1. 在 `crates/tterm-terminal-view` 中实现终端的 GPUI Element：由字体度量算出单元格宽高与行列数；按行合并相同样式的文字再排版；背景色块、光标各形状、选区、下划线样式、宽字符与 emoji。
  2. 终端调色板类型放在 `tterm-terminal`，字段对应 `src/lib/terminalPalette.ts`：16 色、前景、背景、光标、选区、搜索高亮。
  3. 基本滚动：滚轮改变显示偏移；完整的滚动条与锚定见 P0-12。
  4. 方框与 Powerline 字符先用字体自带字形，P0-28 移植 Alacritty 的内置字形后替换。
  5. 绘制基准：用 GPUI 的 profiler 记录每帧耗时，供 P0-21 与 P2-25 使用。
- 产出：`tterm-terminal-view`、调色板类型、绘制基准。
- 验收：全部检查通过；同一字体与窗口尺寸下行列数与 Tauri 版一致；空闲时不重绘；报告声明未阅读 Zed 终端源码。
- 评估重点：每帧的排版是否有缓存；后台标签是否停止绘制通知。

### P0-11 录制字节流回放

- 依赖：P0-10
- 规模：M
- 参考：`src-tauri/src/session_log/`、`src/lib/terminalLogReplay.ts`
- 工作：
  1. 准备回放夹具：tTerm 会话日志中带时间信息的录制，加上手工构造的序列（宽字符、组合字符、emoji、备用屏幕、滚动区域、真彩色、超链接）。
  2. 在原型中按原始时间或最快速度回放，显示在终端视图中。
  3. 对同一夹具，比较 xterm.js 与新终端在结束时的屏幕文本与属性。可以借助 Tauri 版的 SerializeAddon 导出参照。
- 产出：`tterm-terminal` 的测试夹具目录、回放工具、比较结果。
- 验收：差异逐项列出，并判断属于 Unicode 版本差异、实现缺陷还是可以接受的行为差异。

### P0-12 滚动、滚动条与回滚区锚定

- 依赖：P0-10
- 规模：L
- 参考：P0-27 的设计说明；现有 xterm.js 的滚动行为（作为目标）；nyaterm issue #985、#923、#1043（作为要避免的问题）；**不得阅读 Zed 的终端源码**
- 工作：
  1. 小数滚动量累积、边界限制、显示偏移；滚动条的滑块几何、拖拽、搜索标记。
  2. 后台标签策略：不可见时继续解析，停止快照与绘制通知。
  3. **回滚区写满后的锚定。** 官方 Alacritty 没有滚动计数（nyaterm 为此打了补丁，本项目不打）。评估不改内核的做法，例如比对视口内容找出移动的行数；或者向 Alacritty 官方提交补丁、合入前接受与 xterm.js 一致的退化行为。不得 fork Alacritty。
  4. 为三类场景写回归测试：滚动时历史丢失或跳行；回滚区写满后，不在底部时快速输出导致的闪烁；锚定行被淘汰时的行为。
- 产出：滚动模块及其测试、锚定方案的决策记录。
- 验收：三类场景有自动化测试；锚定行为与现有 xterm.js 版对比后逐项说明差异。
- 评估重点：锚定在回滚区淘汰时的坐标换算。

### P0-13 原型输入：键盘、选区、剪贴板、输入法

- 依赖：P0-10、P0-28
- 规模：M
- 参考：P0-27 的设计说明；P0-28 移植的按键与鼠标协议；GPUI 的输入法接口；**不得阅读 Zed 的终端源码**
- 工作：接通键盘输入编码、鼠标选区、复制粘贴、输入法预编辑与候选框定位，达到能在 shell 中正常使用的程度。
- 产出：原型中的输入处理。
- 验收：在三平台的 shell 中完成中文输入、复制、粘贴；记录未实现的按键与模式；报告声明未阅读 Zed 终端源码。
- 不做：完整的按键与鼠标协议，留给 P2-03、P2-04、P2-11。

### P0-27 终端净室设计说明（参考 Zed）

- 依赖：W0-02
- 规模：M
- 并行：P0-01 至 P0-08
- 参考：Zed `crates/terminal`、`crates/terminal_view`（GPL-3.0-or-later，**只读**）；[技术选型第 5 节](tech-selection.md#5-终端)的净室规则
- 工作：读 Zed 的终端实现，写一份**纯文字**的设计说明 `docs/gpui-migration/terminal-design.md`，覆盖：模型与视图的划分；PTY 输出到屏幕更新的数据流与线程；快照与变更追踪；绘制时的排版与缓存；滚动、选区、搜索、链接；输入法的预编辑与候选框定位；按键与鼠标事件的路由；resize；已知的边界情况与 Zed 处理过的坑。可以写 Alacritty 与 GPUI 的公开接口名。
- 产出：设计说明。
- 验收：
  - 说明中没有代码片段、伪代码逐行翻译或 Zed 私有的类型与函数名。评估者逐节检查。
  - 每节说明“为什么这样设计”，不是只描述“Zed 是这样做的”。
  - 执行者在报告中声明自己将不参与 P0-09、P0-10、P0-12、P0-13 的实现。
- 不做：任何代码。

### P0-28 移植 Alacritty 应用层的输入协议与内置字形

- 依赖：P0-09
- 规模：L
- 参考：官方 Alacritty（Apache-2.0）`alacritty/src/input/keyboard.rs`、`input/mod.rs`、`renderer/text/builtin_font.rs`、`display/hint.rs`，固定到与 `alacritty_terminal 0.26.0` 同一发布的提交
- 工作：
  1. 按键到转义序列，含应用光标键、修饰键组合与 kitty 键盘协议；从 winit 的按键类型改写为 GPUI 的 `Keystroke`。
  2. 鼠标上报与选区规则：各上报模式、双击与三击选词选行、滚轮在备用屏幕上的行为。
  3. 方框字符、Powerline 箭头等内置字形，改为用 GPUI 的路径或矩形绘制，接入 P0-10。
  4. URL 与路径识别。
  5. 每个复制的文件保留原许可声明，文件头注明来源提交、原路径与修改，登记 `third-party.md`。
- 产出：上述模块与测试；原样复制、尚未拆分的部分放在 `migrate/spike`。
- 验收：按键编码有逐条对照测试（按键、修饰键、模式 → 期望字节）；鼠标上报各模式有测试；内置字形的截图与 Tauri 版对比；第三方登记完整。
- 评估重点：改写时是否改变了协议行为；与 tTerm 现有的 `src-tauri/src/terminal/ssh_query_handler.rs` 是否重复应答。

### P0-29 nyaterm 模块挑选与质量关卡

- 依赖：P0-09
- 规模：M
- 参考：nyaterm（Apache-2.0）固定到某个发布版本，例如 v2.0.2：`crates/nyaterm-terminal-gpui/src/keywords.rs`（关键词高亮，与 P1-11b 和 tTerm 的 `src/lib/keywordHighlight/` 对照）；`crates/nyaterm-terminal/src/sixel.rs` 与 `graphics/`（sixel 与 kitty 图片协议）。会话编码不在候选内，tTerm 后端已有实现。
- 工作：
  1. 对每个候选模块做质量关卡：在 tTerm 的 clippy 配置下的告警清单；抗崩溃测试（随机输入不 panic）；它依赖的 nyaterm 其他代码；按仓库大小规则拆分所需的改动量。
  2. 判断它是否依赖 nyaterm 给 Alacritty 加的滚动计数；依赖的一律不取。
  3. 通过关卡的模块复制进来，登记 `third-party.md`；不通过的写明原因和自己实现的估算。
  4. 图片协议是现有 Tauri 版没有的功能，不在对齐范围内：本卡只给出结论，是否加入由决策人另行决定。
- 产出：`reports/P0-29.md` 中的逐模块结论、复制进来的模块与测试。
- 验收：每个候选模块都有“采用”或“不采用”的结论和证据；采用的模块全部检查通过（拆分完成前放在 `migrate/spike`）。
- 评估重点：复制的模块是否夹带了 nyaterm 的应用层依赖。

### P0-14 原型接入本地 PTY

- 依赖：P0-10、P0-13
- 规模：M
- 工作：
  1. 原型直接用 portable-pty 启动本机默认 shell，接通输入、输出与 resize。Windows 上必须与 Tauri 版一样加载随应用分发的 ConPTY（`src-tauri/resources/conpty/` 中的 `conpty.dll` 与 `OpenConsole.exe`，由 `build.rs` 调用 `scripts/prepare-conpty.ps1` 准备），否则颜色与重排行为会与 Tauri 版不同，对比失去意义。
  2. 远程会话在 P0 中通过本地 PTY 运行系统 `ssh` 命令完成，用于验证 vim、tmux、TUI 的显示。经 tTerm SSH 服务的接入在 P1-12 完成。
  3. 输出管线按计划 7.3 节：合并刷新通知，有界队列，不在每个数据包上绘制。
- 产出：可以日常敲命令的原型。
- 验收：三平台可以运行 shell、vim、tmux；持续输出时内存不无限增长；关闭窗口时 shell 进程退出。
- 评估重点：读取线程与界面线程之间的锁；是否把系统 `ssh` 的捷径记录为待替换项。

### P0-15 终端兼容性冒烟

- 依赖：P0-14
- 规模：M
- 工作：在原型中运行并截图记录：vim、neovim、tmux、htop、less、man、git log 分页、Claude Code、Codex 等常用 AI TUI；鼠标上报、备用屏幕、括号粘贴、应用光标键、焦点报告；vttest 的主要章节；宽字符、组合字符与 emoji 宽度。
- 产出：兼容性表格，每行列出现象、与 Tauri 版对比、严重程度。
- 验收：所有场景都有结论；阻塞性问题单独列出。

### P0-16 Linux 输入法矩阵 `[人工]`

- 依赖：P0-14
- 规模：M
- 参考：迁移计划 7.2 节；nyaterm issue #1017、#1073
- 工作：在 X11（XIM）与 Wayland（text-input-v3）下，分别搭配 fcitx5 与 ibus 验证：启动后首次输入；切换到其他应用再切回；重启输入法进程；预编辑时切换标签；候选框随光标移动；X11 下 XIM 握手出错后的恢复。
- 产出：四个组合的结果表与录屏。
- 验收：每格为“通过”“失败”或“未验证”。失败项附复现步骤，并判断是否属于 GPUI 平台层。
- 评估重点：这是 P0 的停止条件之一，结论不能含糊。

### P0-17 无 GPU 与远程桌面环境 `[人工]`

- 依赖：P0-14
- 规模：M
- 参考：迁移计划 7.7 节；nyaterm issue #938、#1020
- 工作：
  1. 在 W0-03 列出的无 GPU 环境中运行原型与 Tauri 版：空闲、拖动窗口、持续输出、输入。
  2. 记录实际使用的渲染适配器（Windows 上的 D3D11 适配器，包括 WARP；Linux 上的 Vulkan 实现，如 llvmpipe）。
  3. 尝试降级手段：空闲不重绘、限制帧率、拖动时降低绘制频率，记录效果。
- 产出：对比数据与降级方案评估。
- 验收：每个环境给出与 Tauri 版的对比；说明是否存在可行的降级方案。
- 评估重点：这是 P0 的停止条件之一。

### P0-18 输入法、DPI、多屏与显示环境变化 `[人工]`

- 依赖：P0-14
- 规模：M
- 工作：
  1. macOS 与 Windows：系统中文输入法与常用第三方输入法。
  2. 三平台：在不同缩放比例的两块显示器之间拖动窗口；Linux Wayland 的分数缩放；字体回退（中文、emoji、Nerd Font 符号）。
  3. 显示环境变化：睡眠唤醒、显示器热插拔、双显卡 MacBook 切换显卡、RDP 断开后重连、Windows 锁屏。GPUI 在这些情况下可能丢失 GPU 设备或图集，需确认能恢复绘制而不崩溃。
- 产出：结果表与截图。
- 验收：每项有结论；崩溃或无法恢复绘制的项附日志。

### P0-19 文件拖出方案

- 依赖：P0-07、P0-08
- 规模：M
- 参考：tTerm `src-tauri/src/drag_out/`（已有 macOS 与 Windows 的原生实现）、`src/components/SftpDrawer/useSftpDragOut.ts`；`nyakang/zed` 的文件导出补丁（Apache-2.0，只作参考，见 P0-07）
- 工作：优先方案是在 GPUI 窗口上复用 tTerm 现有的 `drag_out` 原生代码，通过窗口的原生句柄发起拖拽；只有它在某个平台不可行时，才评估按 P0-06 的退路给 GPUI 打补丁。各做一个最小演示，把一个延迟生成内容的文件拖到 Finder 或资源管理器，Linux 拖到文件管理器。
- 产出：演示代码与决策记录。
- 验收：至少一种方案在三平台可行，或明确列出不可行的平台。
- 评估重点：延迟下载（拖放时才开始下载）与大文件的处理。

### P0-20 远程编辑器方案

- 依赖：P0-08
- 规模：M
- 参考：`src/components/RemoteFileEditor.tsx`、`src/components/CodeMirrorEditor.tsx`、`package.json` 中的 CodeMirror 语言包、gpui-kit 编辑器
- 工作：用 gpui-kit 的编辑器做原型，验证：撤销与重做、查找替换、语法高亮（覆盖现有支持的语言：CSS、HTML、JavaScript、JSON、Markdown、Rust、YAML）、50 MB 文件打开与滚动、中文输入法、编码与换行格式的保留、保存时的冲突检测。
- 产出：结果表与决策记录。
- 验收：每项有结论；不能满足的项给出替代方案（例如对大文件改为只读或调用外部编辑器）。

### P0-21 原型性能对比

- 依赖：P0-14、P0-01
- 规模：M
- 工作：用 P0-01 的负载和口径测量原型，用 P0-10 的绘制基准测绘制耗时，与基线对比。另测空闲时的唤醒次数与能耗：光标闪烁、动画与定时器是否让空闲窗口持续重绘（macOS 用 `powermetrics` 或活动监视器的“能耗影响”，Linux 用 `powertop`）。
- 产出：对比 CSV 与报告。
- 验收：同一机器、同一构建条件；说明原型缺失的功能可能对数字造成的影响。

### P0-22 P0 结论与重新估算

- 依赖：P0-15 至 P0-21
- 规模：S
- 工作：
  1. 汇总所有 P0 报告，按迁移计划第 6 节的停止条件逐项判断。
  2. 列出功能差异，并按 P1 至 P5 的任务卡重新估算工期。估算时对照 nyaterm 对应模块的规模。
  3. 列出需要修改的任务卡。
- 产出：`reports/P0-22.md`，更新迁移计划第 10 节。
- 验收：每个停止条件都有明确结论与证据链接。

### P0-23 供 agent 使用的界面自动化与截图

- 依赖：P0-08
- 规模：M
- 背景：现在 agent 通过 WebView2 的 CDP 驱动 Tauri 开发版窗口并截图，GPUI 窗口没有这个入口。没有替代手段，P2 至 P4 的大量验收只能靠人工截图，评估也无法复现。
- 工作：
  1. 优先使用 GPUI 的测试支持：在无界面的测试中构建视图、发送动作与按键、读取状态，并把视图渲染成图片（核实固定版本的 GPUI 是否支持离屏截图）。
  2. 补充一个只在开发构建中启用的本地控制接口（例如只监听本机的套接字，需要随机令牌）：列出窗口与焦点、发送动作与按键、截取窗口图像。正式构建中不编译这部分代码。
  3. 写使用说明：如何启动、如何截图、哪些操作会打开真实的系统对话框而不能在自动化中触发。
- 产出：测试辅助模块、开发控制接口、`tterm-gpui/AUTOMATION.md`。
- 验收：一个示例脚本打开设置页、切换一项设置、截图并读回设置值；正式构建的二进制中不含控制接口（报告给出检查方法）。
- 评估重点：控制接口的安全性，不能在正式版中被其他本地进程利用。

### G1 是否全面迁移 `[人工]`

根据 P0-22 与 [acceptance.md 第 4 节](acceptance.md#g1-技术验证通过进入-p1)的标准决定是否进入 P1。迁移已经决定（[决策 001](decisions/001-formal-migration.md)），G1 发现阻塞时只调整方案或缩小平台范围。决定写入 `decisions/003-g1.md`。
