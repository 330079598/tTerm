# P2 原生终端 MVP

目标：GPUI 端的终端在日常使用上与 Tauri 版对齐，覆盖本地 shell、SSH、串口，以及 tTerm 在 xterm.js 之上加的所有增强。

前提：P1-12 与 P1-13 已完成，终端组件来自 P0-09 至 P0-14 与 P0-28 的原型；P0-23 的界面自动化可用。实现终端的执行者遵守[技术选型第 5 节](tech-selection.md#5-终端)的 Zed 参考规则：可以阅读 Zed 的终端源码，不复制代码，报告中列出阅读过的文件（[决策 008](decisions/008-zed-terminal-reading.md)）。

## 对齐范围

现有终端功能集中在 `src/components/TerminalTab/`（约 1.15 万行，不含测试）与 `src/lib/` 中的终端逻辑。现有 xterm.js 的配置见 `src/components/TerminalTab/openTerminal.ts`：光标样式与闪烁、macOS Option 作为 Meta、Option 拖动强制选择、回滚行数、字体族、字号、字重、字间距、行高、主题、透明背景、概览标尺。

前端拦截的转义序列：

| 序列                  | 用途                               | 现有位置                                       |
| --------------------- | ---------------------------------- | ---------------------------------------------- |
| OSC 133、633          | 命令标记、命令跟踪，鼠标上报保护   | `commandTracking.ts`、`mouseReportingGuard.ts` |
| OSC 7、OSC 9;9        | 工作目录报告                       | `cwdReports.ts`                                |
| OSC 9、777 通知       | 通知与标签提醒                     | `attentionReports.ts`                          |
| OSC 777 `tterm-agent` | AI agent 状态                      | `src/lib/agentStatus.ts`                       |
| OSC 777 系统报告      | 远程系统信息                       | `osReports.ts`                                 |
| OSC 52                | 写剪贴板                           | `src/lib/osc52.ts`                             |
| CSI ? h / l           | 光标可见性跟踪（输入法、预测回显） | `imeCursorGuard.ts`、`predictiveEcho.ts`       |
| CSI 3 J               | 清除回滚区后释放内存               | `scrollbackMemory.ts`                          |

这些处理在 P1-11a 中已移植为 Rust 逻辑，本阶段把它们接到终端解析流程上。

## 共同验收

每张卡除自身的验收外，还需要：

1. 与 Tauri 版对同一操作的行为对比，写在报告中，差异逐项说明。
2. 新增的行为有自动化测试；界面相关的用 GPUI 的测试支持（`TestAppContext` 等）或回放夹具测试。
3. 设置项的读取使用现有配置字段，不新增或改名字段。

## P2-01 终端视图与会话绑定

- 依赖：P1-12
- 规模：M
- 参考：迁移计划 4 节的核心约束；`src/components/TerminalTab/useTerminalLifecycle.ts`、`ptySession.ts`
- 工作：
  1. 会话由 core 持有，终端视图只订阅输出、发送输入。视图销毁不结束会话；标签移动、分屏变化时视图可以重新附着到同一会话。
  2. 处理会话关闭、重连后 `session_nonce` 变化时的迟到输出。
  3. 标签关闭时释放后端为它保存的状态，例如连接过程中输入的密码（现由 `src/hooks/useConnectionManager.ts` 触发）。
  4. 标签进入与离开视野时的处理（`useTerminalActivation.ts`）：重新计算尺寸、获得焦点、清除该标签的提醒。会话范围内的状态（`useSessionScopedState.ts`：主机密钥提示、连接状态与进度、本地 shell 启动失败原因）在新会话开始时重置。
- 验收：测试覆盖视图销毁后会话仍在、重新附着后内容完整、旧会话的迟到输出被丢弃、关闭标签后保存的密码被清除。

## P2-02 输出管线与背压

- 依赖：P2-01
- 规模：M
- 参考：迁移计划 7.3 节；P1-06 的 `SessionSink`；P0-27 的设计说明（后台标签与刷新策略）
- 工作：GPUI 端实现 `SessionSink`：解析在后台线程进行，合并刷新通知，按帧读取可见区域快照；不可见的标签继续解析但停止快照与绘制；背压确认在解析完成后发出。
- 验收：P0-01 的持续输出负载下内存有界；输入与 Ctrl+C 在持续输出时仍能及时响应（报告给出延迟数据）；30 个后台会话同时输出时，前台终端帧率不明显下降。
- 评估重点：界面读取快照时持有解析锁的时间。

## P2-03 键盘输入编码

- 依赖：P2-01
- 规模：M
- 参考：`src/components/TerminalTab/terminalInput.ts`、`useTerminalShortcuts.ts`；xterm.js 的按键编码行为
- 工作：
  1. 覆盖所有按键与修饰键组合的编码：应用光标键与应用小键盘模式、功能键、Alt 作为 Meta（macOS 上由 `macOptionIsMeta` 设置控制）、Ctrl 组合键、Windows 上的 AltGr。
  2. 应用快捷键与终端输入的优先级与 Tauri 版一致：哪些组合键先被应用截获，哪些送进终端。
- 验收：一张按键编码对照表，列出每个组合在 Tauri 版与原生版中发送的字节，并有自动化测试。
- 评估重点：macOS 上 Option 输入特殊字符与作为 Meta 的切换；不同键盘布局。

## P2-04 鼠标与选区

- 依赖：P2-01
- 规模：M
- 参考：`mouseReportingGuard.ts`、`src/lib/terminalText.ts`
- 工作：单击、双击选词、三击选行、矩形选择；鼠标上报模式（1000、1002、1003、1006）；按住 Shift 或 macOS 上按住 Option 强制选择；鼠标上报保护（OSC 133 命令结束后自动关闭残留的鼠标上报）；Linux 中键粘贴。
- 验收：在 vim、tmux、htop 中验证鼠标；选词的分隔符规则与 Tauri 版一致。

## P2-05 剪贴板与粘贴

- 依赖：P2-03、P2-04
- 规模：M
- 参考：`useTerminalClipboard.tsx`、`src/lib/pasteGuard.ts`、`src/lib/osc52.ts`、`useTerminalFileDrop.ts`、`src/lib/droppedPaths.ts`
- 工作：选中即复制等设置；括号粘贴；多行粘贴确认；OSC 52 写剪贴板及其设置；拖入文件时插入经过转义的路径（按目标 shell 的规则）。
- 验收：粘贴确认的触发条件与 `pasteGuard` 的测试一致；拖入路径的转义覆盖空格、引号与非 ASCII 字符。

## P2-06 滚动、滚动条与回滚内存

- 依赖：P0-12、P2-02
- 规模：M
- 参考：`scrollbackMemory.ts`、`ScrollbackLinesField.tsx`、`src/lib/scrollback.ts`
- 工作：把 P0-12 的滚动模块接入正式视图；概览标尺显示搜索结果与命令标记；回滚行数设置与动态调整；清除回滚区后释放内存。
- 验收：回滚区写满后浏览历史不闪烁、不跳行（P0-12 的回归测试继续通过）；修改回滚行数后不需要重开会话。

## P2-07 终端搜索

- 依赖：P2-06
- 规模：M
- 参考：`TerminalSearchBar.tsx`、`useTerminalSearch.ts`、`searchTypes.ts`
- 工作：正则、区分大小写、全词匹配、高亮全部结果、上一个与下一个、结果计数，以及在大回滚区中的性能。
- 验收：10 万行回滚中搜索的耗时（报告给出数据）；输出持续增加时结果位置正确。

## P2-08 链接

- 依赖：P2-04
- 规模：S
- 参考：`@xterm/addon-web-links` 的使用位置、`src/lib/oscLinkDedupe.ts`
- 工作：OSC 8 超链接；URL 自动识别；按修饰键点击打开；悬停样式与提示。
- 验收：与 Tauri 版的识别规则对比；打开链接走 P1-08 的平台能力。

## P2-09 字体与绘制设置

- 依赖：P2-01
- 规模：M
- 参考：`src/components/SettingsDialog/FontSettingsTab.tsx`、`FontFamilyPicker.tsx`、`CursorStylePicker.tsx`、`TerminalPaddingFields.tsx`、`src/lib/terminalFont.ts`、`src/hooks/useTerminalFontZoom.ts`、`src-tauri/src/fonts/`
- 工作：字体族与回退链（中文、emoji、Nerd Font 符号）、字号、字间距、行高、粗体字重；光标样式与闪烁；内边距；字号缩放快捷键；系统字体列表。
- 验收：每个设置修改后即时生效；与 Tauri 版并排截图对比同一内容。
- 评估重点：字体回退时的字形宽度与对齐。

## P2-10 终端主题与调色板

- 依赖：P1-11f、P2-01
- 规模：S
- 参考：`useTerminalTheme.ts`、`src/lib/terminalPalette.ts`、`isTransparentTerminalTheme`
- 工作：应用终端主题的全部槽位，切换主题即时生效；透明背景主题与窗口模糊配合。
- 验收：现有内置主题逐一截图，与 Tauri 版对比。

## P2-11 输入法完整实现

- 依赖：P0-13、P2-03
- 规模：L
- 参考：迁移计划 7.2 节；`imeContextReset.ts`、`imeCursorGuard.ts`、`imeEarlyInput.ts`、`imeFocusRepair.ts`、`src/lib/ime.ts`；P0-16、P0-18 的结果
- 工作：
  1. 预编辑文本在光标处内联绘制，候选框跟随光标，包括滚动、缩放、跨屏之后。
  2. 逐个审查现有的四个输入法修复：判断它修的是 WebView 特有问题（原生端不需要）还是终端层问题（原生端需要同等处理），写进报告。
  3. 避免按键事件与输入法提交重复发送。
- 验收：P0-16 与 P0-18 的全部场景重新测试；报告附四个修复的审查结论。
- 评估重点：这是用户最容易感知的退化点。

## P2-12 协议应答的唯一来源

- 依赖：P2-02
- 规模：M
- 参考：`src-tauri/src/terminal/ssh_query_handler.rs`；Alacritty 的应答实现
- 工作：
  1. 列出所有需要终端回复的查询：DA1、DA2、DSR、CPR、DECRQM、XTVERSION、OSC 10、11、12 颜色查询、窗口尺寸查询等。
  2. 对每个查询确定唯一的回复方：后端的 `ssh_query_handler`，或终端模型。Tauri 版中 xterm.js 负责回复的那部分，在原生版中由终端模型负责。
  3. 增加测试，确保同一查询不会被回复两次，且回复内容与屏幕状态一致。
- 验收：查询表与测试；在 vim、tmux、neovim 中不出现多余字符。

## P2-13 Shell 集成与终端报告

- 依赖：P1-11a、P2-02
- 规模：L
- 参考：上方“对齐范围”中的序列表；`src-tauri/src/terminal/shell_integration/`；`src/lib/sshShellIntegration.ts`、`remoteShellIntegration.ts`
- 工作：
  1. 把 OSC 133、633、7、9、777、52 的处理接到解析流程上，事件交给工作区（标签提醒、工作目录、AI 状态、通知）。
  2. 命令标记：在概览标尺显示，支持跳到上一个或下一个命令，选中命令输出。
  3. 标题（OSC 0、2）、响铃。
- 验收：bash、zsh、fish、PowerShell 的 shell 集成在本地与 SSH 中都工作；现有的 `real_shell_integration` 测试继续通过。

## P2-14 密码与 sudo 提示

- 依赖：P1-11a、P2-13
- 规模：M
- 参考：`PasswordPrompt.tsx`、`passwordPromptLayout.ts`、`PasswordFeedbackOverlay.tsx`、`SavedPasswordPromptBar.tsx`、`savedPasswordNotice.ts`、`useCapsLockNotice.ts`、`src/lib/sudoPrompt.ts`、`src/lib/passwordFeedback.ts`、`SudoPromptPatternsSetting.tsx`
- 工作：识别密码与 sudo 提示；显示已保存密码的填充栏及其快捷键；密码输入时的字符计数反馈；Caps Lock 提示。
- 验收：现有中英文与 doas 提示样例全部识别；填充的密码不出现在日志与回滚区中。
- 评估重点：密码在内存中的生命周期（现有代码使用 `Zeroizing`）。

## P2-15 预测回显

- 依赖：P1-11a、P2-02
- 规模：M
- 参考：`src/lib/predictiveEcho.ts`、`components/TerminalTab/predictiveEcho.ts`、`predictiveEchoOverlay.ts`、`PredictiveEchoSection.tsx`
- 工作：在高延迟连接上预测普通输入与退格的回显，以覆盖层显示，确认或撤销预测。
- 验收：用 `tc netem` 或等价工具模拟 300 ms 延迟，对比 Tauri 版的表现。

## P2-16 关键词高亮

- 依赖：P1-11b、P2-02
- 规模：M
- 参考：`keywordHighlighter.ts`、`useKeywordHighlight.ts`、`src/lib/keywordHighlight/`、`src-tauri/src/config/keyword_highlight.rs`
- 工作：按用户规则与内置规则高亮输出；规则修改即时生效；大量输出时的性能。
- 验收：规则集的匹配结果与 TS 测试一致；持续输出负载下开启与关闭高亮的吞吐差异（报告给出数据）。

## P2-17 输入注入接口

- 依赖：P2-03
- 规模：S
- 参考：`src/components/TerminalTab/BroadcastControls.tsx`、`useComposerTarget.ts`、`src/lib/composerRun.ts`
- 工作：为广播输入、命令编写器、命令库、`cd` 到 SFTP 目录（`useCdInTerminal.ts`）提供统一的输入注入接口，遵守括号粘贴与输入节奏。界面在 P4 中完成。
- 验收：接口有测试；注入的输入与键盘输入经过同一个编码与节奏控制路径。

## P2-18 会话日志

- 依赖：P2-02
- 规模：S
- 参考：`src-tauri/src/session_log/`、`src/lib/terminalLogRecording.ts`、`TabLogIndicator.tsx`
- 工作：确认日志记录在 core 中进行，与界面无关；标签上的记录状态指示。
- 验收：同一会话在两个客户端中生成的日志文件内容一致。

## P2-19 终端内容保存与恢复

- 依赖：P2-02
- 规模：M
- 参考：`savedTerminalContent.ts`、`terminalSnapshot.ts`；迁移计划 7.6 节
- 工作：
  1. 读取 Tauri 版用 SerializeAddon 保存的终端内容（转义序列文本），回放到新终端中，并标示为恢复的内容。回放过程中不触发通知、剪贴板写入、标题变化等副作用。
  2. 决定原生版保存的格式：继续使用转义序列文本，或新格式。新格式需要写决策记录，并保证 Tauri 版回退时不会读坏数据。
- 验收：用真实的旧快照测试恢复效果；回放不触发副作用的测试。

## P2-20 ZMODEM、XMODEM 与串口显示

- 依赖：P2-02
- 规模：M
- 参考：`src-tauri/src/zmodem/`、`useZmodemTransfers.ts`、`serialDisplay.ts`、`SerialFileTransfer.tsx`
- 工作：二进制协议分流时终端暂停显示；传输进度与取消的界面钩子；串口的十六进制等显示模式。
- 验收：`rz`、`sz` 上传下载；传输中取消；传输结束后终端恢复正常。

## P2-21 连接状态与重连

- 依赖：P2-01
- 规模：M
- 参考：`ConnectionHeader.tsx`、`connectionStateUpdater.ts`、`useConnectionNotices.ts`、`LocalShellFailedBar.tsx`、`src/lib/terminalOutputScanner.ts`、`src-tauri/src/config/reconnect.rs`
- 工作：连接中、已断开、重连中等状态的显示；自动重连及其设置；本地 shell 启动失败的提示与操作。
- 验收：断网、服务器重启、认证失败三种情况的表现与 Tauri 版一致。

## P2-22 尺寸变化与重排

- 依赖：P2-02
- 规模：S
- 参考：`useTerminalFit.ts`、`src-tauri/src/core/state.rs` 中的 `AtomicTerminalSize`
- 工作：窗口与分屏尺寸变化时的重排，向 PTY 与 SSH 发送尺寸变化，拖动分隔条时的节流。
- 验收：拖动分隔条时不卡顿；vim 与 tmux 在尺寸变化后显示正确。

## P2-23 终端上下文菜单

- 依赖：P2-05、P3-15
- 规模：S
- 参考：`useTerminalContextMenu.ts`
- 工作：复制、粘贴、全选、清屏、重置终端、搜索、打开链接、在 SFTP 中打开当前目录等菜单项与 Tauri 版一致。重置终端按 `terminalReset.ts` 的做法：发送 DECSTR，并关闭所有鼠标跟踪模式与编码，恢复被乱码控制序列弄坏的终端。
- 验收：菜单项对照表。

## P2-24 兼容性回归套件

- 依赖：P0-11、P2-12
- 规模：M
- 工作：把 P0-11 的夹具扩展为回归套件：回放字节流后比较屏幕文本与属性的快照。覆盖宽字符、组合字符、emoji、备用屏幕、滚动区域、真彩色、自动换行、重排、超链接，以及 P0-15 发现的问题。在 CI 中无界面运行。
- 验收：套件在三平台 CI 中运行；修改终端代码时快照变化必须在审查中说明。

## P2-25 终端性能与内存验收

- 依赖：P2-01 至 P2-22
- 规模：M
- 工作：用 P0-01 的负载与口径完整测量，与基线对比；在无 GPU 环境中复测；测量空闲时的唤醒次数与能耗（同 P0-21），确认光标闪烁等只重绘必要的区域、窗口不可见时停止。
- 验收：达到迁移计划第 9 节的门槛，或逐项说明差距。

## G2 终端可日常使用 `[人工]`

使用者在 GPUI 版中完成一周的日常终端工作（本地 shell、SSH、tmux、vim、AI TUI），记录遇到的问题。阻塞性问题清零后进入 P4。
