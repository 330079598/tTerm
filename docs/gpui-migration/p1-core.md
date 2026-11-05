# P1 抽取共享业务核心

目标：业务逻辑脱离 Tauri 和 GPUI 运行，两个客户端调用同一套服务。完成后 Tauri 版行为不变，GPUI 原型可以经同一服务建立和关闭本地与 SSH 会话。

本阶段全部是重构。[执行规则](README.md#2-执行规则)第 2 至 4 条在这里最重要：不改行为，移动与修改分开，不改持久化格式。

## 现状

2026-11-01 统计，不含测试文件：

| 耦合点                       | 数量                   | 位置                                           |
| ---------------------------- | ---------------------- | ---------------------------------------------- |
| 引用 `AppHandle`             | 约 344 处              | 分布在多数领域模块中                           |
| `#[tauri::command]`          | 209 个                 | 各模块的 `commands` 或 `mod.rs`                |
| `app.manage(...)` 注册的状态 | 20 个                  | `src-tauri/src/lib.rs` 第 114–133 行           |
| 全局 `static` 状态           | 28 个                  | 数据库、配置目录、密钥库、跳板连接池、限速器等 |
| `ChannelContext.app`         | 1 处，影响所有会话输出 | `src-tauri/src/core/state.rs:70`               |

各模块发出事件的次数（`.emit` 与 `.emit_to`）：`sftp` 25、`ssh` 10、`tunnel` 5、`background` 4、`zmodem` 3、`core` 2、`proxy` 2、`session_log` 2、`terminal` 2、`notification` 1、`serial` 1。

全局 `static` 状态可以暂时保留：它们不依赖 Tauri，两个客户端不会在同一进程中同时运行。P1-09 移动代码时一并移入 core，并在报告中列出，供以后决定是否改为显式持有。

## P1-01 应用路径注入

- 依赖：G1
- 规模：M
- 并行：P1-02、P1-10、P1-11
- 参考：`src-tauri/src/config/paths.rs`、`src-tauri/src/db/mod.rs`、`src-tauri/src/migrate.rs`、`src-tauri/src/session_log/`
- 工作：
  1. 定义 `AppPaths`：配置目录、数据目录、缓存目录、日志目录，以及现有代码用到的其他目录。启动时由适配层根据平台计算并注入，core 不再调用 Tauri 的路径解析。
  2. 原生端的计算必须与 Tauri 在三平台上的结果完全一致：应用标识 `com.stone.tTerm`，同样的大小写和子目录。
  3. 找出所有直接解析路径的位置，包括日志、备份、下载目录与临时文件。
- 产出：`AppPaths` 及其两种构造方式、单元测试。
- 验收：
  - 一个测试在三平台 CI 上比较两种构造方式的结果完全相同。
  - Tauri 版启动后读写的目录与改动前相同，报告附改动前后的目录列表。
- 评估重点：Linux 上 XDG 环境变量的处理；Windows 上 Roaming 与 Local 的区别；macOS 上 `Application Support` 与 `Caches`。
- 不做：迁移或改名任何目录。

## P1-02 事件接口与 Tauri 适配器

- 依赖：G1
- 规模：M
- 参考：架构约定 A3；`src-tauri/src/tunnel/` 中的 `STATUS_EVENT`
- 工作：
  1. 在 `tterm-core` 定义 `AppEvent` 枚举与 `EventSink` trait。事件按领域分组，每个变体的 payload 使用现有的 serde 类型。
  2. 在 Tauri 包中实现 `TauriEventSink`：每个变体映射到现有的事件名与发送方式（`emit` 或 `emit_to(EventTarget::any(), ...)`），包括按 `tab_id` 拼接的动态事件名，例如 `sftp-upload-batch-complete-{tab_id}`。
  3. 实现测试用的 `RecordingEventSink`，记录发出的事件。
  4. 以 `tunnel` 模块为试点完成迁移，作为其他模块的样板。
- 产出：事件接口、两个实现、迁移后的 `tunnel` 模块。
- 验收：
  - `tunnel` 模块不再引用 `AppHandle` 来发事件。
  - 一个测试证明 `TauriEventSink` 为每个 `tunnel` 事件生成的事件名与 payload JSON 与改动前一致。
  - 隧道面板在 Tauri 版中工作正常，附操作录屏或手工验证步骤。
- 评估重点：事件枚举是否过早设计成一个巨大的类型；按领域拆分是否便于 GPUI 端订阅。

## P1-03 迁移非 SFTP 模块的事件

- 依赖：P1-02
- 规模：L
- 并行：P1-04、P1-05、P1-06
- 参考：P1-02 的样板
- 工作：按 P1-02 的方式迁移 `ssh`、`background`、`zmodem`、`core`、`proxy`、`session_log`、`terminal`、`notification`、`serial` 的事件，以及 `src-tauri/src/` 根目录下文件中的事件（`launch_dir.rs`、`updater.rs` 等）。每个模块一个提交。
- 产出：上述模块不再用 `AppHandle` 发事件。
- 验收：
  - 对每个事件，有测试比较事件名与 payload JSON。
  - `grep` 证明这些模块中没有剩余的 `.emit(` 或 `.emit_to(` 调用，适配器除外。
  - 前端测试全部通过，`pnpm tauri dev` 中手工验证 SSH 连接、ZMODEM、串口、退出确认、通知点击。
- 评估重点：事件发送的时序是否改变，特别是退出流程中的 `app-will-quit`。
- 不做：`AppHandle` 的其他用途（窗口、托盘、路径），留给 P1-06 至 P1-08。

## P1-04 迁移 SFTP 的事件

- 依赖：P1-02
- 规模：L
- 参考：`src-tauri/src/sftp/`（53 个文件，约 1.4 万行，19 个文件引用 Tauri）
- 工作：迁移 SFTP 的 25 处事件发送，包括上传、下载、删除、搜索、外部编辑（`sftp-external-edit`）、批次完成等进度事件。搜索结果目前经 `ipc::Channel` 发送（`sftp/internal/api/search.rs`），改为由 core 定义的流式接口，Tauri 适配器转回 Channel。
- 产出：SFTP 模块不再用 `AppHandle` 发事件。
- 验收：同 P1-03；另需在 Tauri 版中手工验证上传、下载、删除、递归搜索、外部编辑与取消。
- 评估重点：高频进度事件的节流是否保持原样。

## P1-05 PromptBroker

- 依赖：P1-02
- 规模：L
- 参考：架构约定 A4；`src-tauri/src/ssh/auth/prompter.rs`、`ssh/auth.rs`、`ssh/jump/handler.rs`、`proxy/approval.rs`、`core/state.rs` 中的 `HostPromptMap`；`respond_ssh_auth_prompt`、`respond_ssh_host_key_prompt`、`respond_proxy_command_prompt` 三个 command
- 工作：
  1. 先列出所有“后端发出提示，等待用户回应”的流程，至少包括：主机密钥确认、SSH 认证提示（密码、键盘交互、MFA）、跳板机的同类提示、ProxyCommand 审批及其过期（`proxy-command-prompt-expired`）、退出与关闭确认（`hold_exit_for_confirmation`、`resolve_close_request`、`confirm_quit_app`）。报告中附完整清单。
  2. 在 core 中实现 `PromptBroker`：发出带请求 ID 的提示事件，返回等待回复的 future；支持超时、会话关闭时的取消，以及回复已过期请求时的处理。
  3. 用它替换 `HostPromptMap`、`AuthPromptMap`、`ProxyCommandPrompts` 等分散的实现。三个 `respond_*` command 改为调用 broker，签名不变。
- 产出：`PromptBroker`、迁移后的各流程、测试。
- 验收：
  - 单元测试覆盖：正常回复、超时、取消、重复回复、回复不存在的 ID。
  - 跳板链中多跳依次提示的顺序不变。
  - 用现有 WSL podman 测试主机手工验证主机密钥变化、密码错误重试、键盘交互认证。
- 评估重点：会话关闭时是否所有等待都被取消，没有悬挂的任务；`session_nonce` 对迟到回复的隔离是否保留。

## P1-06 会话输出接口

- 依赖：P1-02
- 规模：L
- 参考：架构约定 A5；`src-tauri/src/core/state.rs`、`src-tauri/src/terminal/io_batcher.rs`、`io_batcher/`、`terminal/reader.rs`、`ssh/` 中的读取任务、`serial/session`
- 工作：
  1. 定义 `SessionSink` trait：字节输出、会话状态变化、退出原因，以及背压确认的接口。
  2. `ChannelContext.app` 改为 `Arc<dyn SessionSink>`。Tauri 实现保持现在的二进制 Channel、批处理和约 1 MiB 未确认上限。
  3. 二进制协议分流（ZMODEM）的位置与顺序不变。
- 产出：`SessionSink` 及 Tauri 实现，所有会话类型改用它。
- 验收：
  - `ChannelContext` 不再含有 Tauri 类型。
  - P0-01 的吞吐负载下，Tauri 版的吞吐与内存与改动前相差不超过 5%。
  - 一个测试用记录型 sink 驱动本地 PTY，验证输出、背压暂停与恢复。
- 评估重点：背压与取消在会话关闭时的行为；锁的持有范围是否扩大。

## P1-07 AppServices 与 command 变薄

- 依赖：P1-03 至 P1-06
- 规模：L，分为四张卡，可以并行
- 参考：架构约定 A2；`src-tauri/src/lib.rs` 第 114–133 行的 `manage` 列表
- 共同工作：
  1. 每张卡把负责模块的 `tauri::State` 与 `app.state::<T>()` 换成 `AppServices` 中的服务。
  2. command 只保留参数转换、调用服务、把结果转成 `Result<T, String>`。
  3. 服务方法不接受任何 Tauri 类型。
- 共同验收：负责模块中除 command 函数外不再出现 `tauri::`；command 的名称、参数与返回类型不变（以 `src/` 中的 `invoke` 调用为准核对）；前端测试与 Rust 测试通过。

| 卡     | 模块                                                                                                                                                                         |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1-07a | `core`、`session`、`terminal`、`ssh`、`serial`、`zmodem`、`monitor`                                                                                                          |
| P1-07b | `sftp`                                                                                                                                                                       |
| P1-07c | `config`、`db`、`profiles`、`command_library`、`backup`、`sync`、`themes.rs`、`fonts`                                                                                        |
| P1-07d | `tunnel`、`proxy`、`session_log` 与 `session_log.rs`、`notification`、`background`、`agent_integration`、`system_integration`、`app_state.rs`、`launch_dir.rs`、`updater.rs` |

- 评估重点：服务之间的依赖是否形成环；`AppServices` 的构造顺序是否与现在的初始化顺序一致（数据库、密钥库、旧数据导入）。

## P1-08 平台能力 trait

- 依赖：P1-01
- 规模：L
- 参考：架构约定 A9；Tauri 插件 opener、os、dialog、clipboard-manager、updater、process、single-instance、frame；`clipboard_files.rs`、`main_window.rs`、`window_blur.rs`、`ime_focus.rs`、`drag_out/`、`notification/`、`system_integration/`、`background/`（托盘与退出）
- 工作：
  1. 在 `tterm-platform` 中定义 trait：打开链接与文件、文件对话框、剪贴板（文本与文件列表）、系统通知、托盘、窗口操作（显示、聚焦、背景色、模糊）、应用更新、进程重启与退出。
  2. Tauri 包中实现这些 trait，内部调用现有插件与代码。
  3. core 中原来直接调用插件的地方改为调用 trait。
  4. 只供 Tauri 使用、原生端另有做法的能力（例如 `ime_focus.rs` 修复的 WebView2 焦点问题）不进 trait，在报告中列出。
- 产出：trait 定义、Tauri 实现、清单。
- 验收：core 不依赖任何 `tauri-plugin-*`；Tauri 版功能不变。
- 评估重点：trait 是否按能力而非按 Tauri 插件划分；能否在 P5-01 中直接用原生 crate 实现。

## P1-09 代码移入 tterm-core

- 依赖：P1-07、P1-08
- 规模：L
- 工作：
  1. 按领域把已解耦的模块从 `src-tauri/src/` 移到 `crates/tterm-core/src/`，每个领域一个只做移动的提交。顺序建议：`db`、`config`、`profiles`、`command_library`、`ssh`、`terminal`、`serial`、`zmodem`、`sftp`、`tunnel`、`proxy`、`monitor`、`session_log`、`backup`、`sync`，其余随后。
  2. Tauri 包只保留 command 函数、事件与平台适配器、`lib.rs` 的注册代码。
  3. Shell 集成脚本（`terminal/shell_integration/`）等资源随模块移动，确认 `include_str!` 等路径正确。
  4. 测试随模块移动；依赖 Tauri 的测试改为使用记录型 sink 与测试平台实现。
- 产出：`tterm-core` 包含业务逻辑，Tauri 包变薄。
- 验收：
  - `cargo tree -p tterm-core` 中不含 `tauri` 与 `gpui`。
  - 测试数量不少于移动前。
  - 每个移动提交的 `git diff -M` 只有路径与 `use` 语句的变化。
- 评估重点：`pub` 范围是否因跨 crate 而被过度放宽；AGENTS.md 要求不用 `pub use` 重新导出。

## P1-10 WebView 本地存储迁入后端

- 依赖：G1
- 规模：M
- 并行：P1-01 至 P1-09
- 参考：`src/lib/backupFrontendState.ts`、`src/lib/recentCommands.ts`、`src/lib/quickConnect.ts`、`src/lib/profileRecents.ts`、`src/components/SftpDrawer/sftpView.ts`、`useSftpColumnWidths.ts`、`src/lib/serialSendStore.ts`、`src/lib/commandComposer.ts`、`src/lib/uiStyle.ts`、`src/lib/themePreloader.ts`、`src/components/LogsPanel/replayView.ts`
- 背景：原生客户端无法读取 WebView 的 localStorage。这些数据必须在切换之前，由某个 Tauri 版本搬进后端存储。
- 工作：
  1. 列出所有 localStorage 键，分为两类：用户数据（最近命令、快速连接与连接的最近记录、SFTP 视图与列宽、串口发送历史与回显设置、命令编写器选项、日志回放视图）；可以重建的缓存（主题缓存、界面风格缓存、缓存版本号）。
  2. 用户数据存入数据库或配置文件，新增后端 command 读写；前端改为通过后端读写，首次启动时把 localStorage 中已有的值迁移过去，迁移成功后保留原值一个版本以便回退。
  3. 备份中的“前端状态”部分（`BackupFrontendState`）改为由后端从新存储读取，备份格式保持不变，旧备份仍可导入。
- 产出：后端存储、迁移逻辑、测试。
- 验收：
  - 测试覆盖：首次迁移、重复启动不重复迁移、localStorage 值损坏时的处理、旧备份导入。
  - 在 Tauri 版中手工验证升级前后数据一致。
- 评估重点：这项改动需要随一个正式的 Tauri 版本发布，并在切换前留出足够的升级时间。报告中写明这一点。
- 不做：缓存类数据的迁移。

## P1-11 前端纯逻辑移植到 Rust

- 依赖：G1、W0-01；各组还依赖其使用的核心类型所在的卡
- 规模：每组 M 或 L
- 并行：七组彼此独立，也可以与 P1-01 至 P1-10 同时进行
- 共同工作：
  1. 把 `src/lib/` 等处的纯逻辑模块移植为 Rust。终端输出相关的放在 `tterm-terminal`，其余放在 `tterm-core`。
  2. **现有的 `*.test.ts` 是规格。** 把每个测试用例转写为 Rust 测试，输入与期望输出保持一致。无法直接对应的用例在报告中逐条说明。
  3. 依赖浏览器 API（DOM、canvas、localStorage）的部分不移植，标注由谁替代。以下模块预计只为 WebView 或 React 存在，原生端不需要移植，执行者确认后在报告中逐个说明理由：`src/lib/canvasFontHost.ts`（WKWebView 中脱离文档的 canvas 字体问题）、`components/TerminalTab/rendererRelease.ts` 与 `useTerminalRenderer.ts`（WebGL 与 Canvas 渲染器的切换与释放）、`imeFocusRepair.ts` 与 `src-tauri/src/ime_focus.rs`（WebView2 的输入法焦点）、`outputAck.ts`（由原生端的背压确认替代）、`SettingsDialog/settingsLazy.ts` 与 `settingsLoaders.ts`（代码分割）、`src/routes/` 与 `routeTree.gen.ts`（前端路由）、`src/hooks/useLatestRef.ts`、`useStableRef.ts`（React 工具）。`imeCursorGuard.ts`、`imeEarlyInput.ts`、`imeContextReset.ts` 不在此列，由 P2-11 逐个判断。
  4. 移植期间 TS 版本保持不动，Tauri 版继续使用 TS 版本。
- 共同验收：Rust 测试包含所有对应 TS 用例；`cargo test --workspace` 通过；报告附 TS 用例与 Rust 测试的对照表。
- 共同评估重点：语义是否一致，特别是 Unicode 处理（JS 字符串是 UTF-16，Rust 是 UTF-8）、正则表达式方言差异、数字精度。

| 卡     | 内容             | 主要来源                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------ | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1-11a | 终端输出处理     | `agentStatus`、`osc52`、`oscLinkDedupe`、`terminalCwd`、`remoteCwd`、`terminalCommandCapture`、`terminalOutputScanner`、`sudoPrompt`、`passwordFeedback`、`loginScript`、`pasteGuard`、`terminalNotifications`、`predictiveEcho`、`distro`；`components/TerminalTab/` 中的 `attentionReports`、`commandMarks`、`commandTracking`、`cwdReports`、`osReports`、`savedPasswordPrompts`、`cursorMoveHold`                                                                                                                                                                                                     |
| P1-11b | 关键词高亮       | `lib/keywordHighlight/`，并与 nyaterm 的 `keywords.rs` 比较，二者择一或合并，写决策记录                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| P1-11c | 配置与默认值     | `appConfig`、`appConfigNormalize`、`tabSettings`、`terminalFont`、`scrollback`、`uiStyle`、`spinnerStyle`、`localShells`、`terminalShells`、`shellCommands`、`sshShellIntegration`、`remoteShellIntegration`、`startup`                                                                                                                                                                                                                                                                                                                                                                                   |
| P1-11d | 工作区模型       | `paneLayout`、`paneNavigation`、`paneTemplates`、`tabsState`、`tabGroups`、`tabGroupsPersistence`、`tabNavigation`、`tabClosing`、`tabMenuActions`、`tabIcons`、`localTabTitle`、`tabNudge`、`tabAttention`、`tabAgentIndicator`、`appNavigation`                                                                                                                                                                                                                                                                                                                                                         |
| P1-11e | 快捷键           | `lib/keymap/` 全部，包括组合键解析、平台显示格式与冲突检测                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| P1-11f | 主题             | `themeDefinitions`、`themeDerivation`、`themeImport`、`themeCatalog`、`themeUtils`、`customThemes`、`terminalPalette`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| P1-11g | 连接、命令与其他 | `quickConnect`、`profileConnections`、`profileGroups`、`profileSearch`、`profileRecents`、`recentCommands`、`commandComposer`、`commandVariables`、`composerRun`、`composerRuns`、`proxy`、`serial`、`serialSendStore`、`sftpStartDirectory`、`monitorMetrics`、`optionSearch`、`terminalLogReplay`、`terminalCd`、`droppedPaths`、`externalEditor`、`notificationBadge`、`vaultLock`、`userVerification`、`sync`、`updater` 中的纯逻辑部分；`SettingsDialog/settingsSearch.ts` 与 `settingsSearchIndex.ts`；`SftpDrawer/` 中的 `sftpView`、`sftpHistory`、`sftpArchive`、`sftpSearch`、`sftpDrawerUtils` |

## P1-13 导入代码按仓库规范拆分并合并

- 依赖：G1
- 规模：L
- 并行：P1-01 至 P1-11
- 参考：P0-09、P0-10 的超限清单；AGENTS.md 中“拆分文件”一节
- 工作：
  1. 把验证分支上的 `tterm-terminal` 与终端绘制代码按大小限制拆分：超过 800 行的文件按职责拆成子模块，超过 150 行的函数拆成小函数。测试文件豁免。
  2. 拆分只移动代码，不改逻辑。每个文件的拆分是一个独立提交。
  3. 拆分完成后把这两部分合回 `GPUI`；原型中其余可保留的代码一并整理合并，P0 的捷径保留并在代码中注明由哪张卡替换。
- 产出：符合规范的终端代码合并进 `dev`。
- 验收：
  - `pnpm check:size` 与 clippy 通过，基线文件没有新增条目，没有新增 `allow` 或 `expect`。
  - 测试数量与验证分支相同，全部通过。
  - 每个拆分提交的 `git diff -M` 只有代码位置变化。
- 评估重点：拆分后的模块划分是否便于 U-01 继续对照上游的改动；`third-party.md` 中的来源路径是否随拆分更新。

## P1-12 无界面集成测试与原型接入 SSH

- 依赖：P1-09
- 规模：M
- 参考：本机 WSL podman 测试主机与 sudo 主机测试的现有用法
- 工作：
  1. 编写不依赖 Tauri 与 GPUI 的集成测试：通过 `AppServices` 打开本地 PTY 与 SSH 会话，经跳板机连接，回应认证提示，收发数据，关闭会话，检查任务全部结束。需要测试主机的测试标为 `#[ignore]`，与现有的 `real_shell_integration` 测试一样单独运行。
  2. GPUI 原型改为经 `AppServices` 建立本地与 SSH 会话，替换 P0-14 中直接调用 portable-pty 和系统 `ssh` 的捷径。
- 产出：集成测试、更新后的原型。
- 验收：
  - 集成测试在 Linux 上通过，附运行输出。
  - 原型中可以打开一个保存在 tTerm 数据库中的 SSH 连接，完成主机密钥确认和密码认证，并正常关闭。
  - Tauri 版与原型不同时写同一个数据库：原型使用隔离的数据目录，报告说明做法。
- 评估重点：这是 P1 的完成标准，也是 P2 与 P3 的起点。
