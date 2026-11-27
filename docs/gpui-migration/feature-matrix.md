# 功能矩阵

本表逐一对照现有 Tauri 版的功能与 GPUI 版的原生实现，用于判断 P4 是否完成、G4 是否可以切换。模板由 W0-02 建立，[P4-27](p4-features.md#p4-27-功能对齐审核) 负责填满。

## 判定口径

本表只使用四种状态，不能用“core 里有服务”代替“界面上可用”：

| 状态     | 含义                                                              |
| -------- | ----------------------------------------------------------------- |
| 未开始   | GPUI 没有对应界面；只有共享 core 服务、配置字段或类型定义不算开始 |
| 进行中   | 已有可操作界面，但功能点、平台、视觉或证据尚未达到与 Tauri 版一致 |
| 对齐     | 行为与现有版一致，且“验收证据”列有可复现的对照测试、录屏或截图    |
| 明确放弃 | 决定不在原生版实现，需要决策人确认并写入 `decisions/`             |

- **对齐**的判定按[验收协议第 2 节](acceptance.md#2-每张卡的评估步骤)：范围、检查、验收条件、界面对齐、行为对齐、第三方代码、报告七步都要过。只通过单元测试或只完成局部截图，仍记为“进行中”。
- **未开始**不等于 core 没有能力。`backup`、`sync`、`sftp`、`serial`、`monitor`、`session_log`、`command_library` 等服务已经存在，但 P4 要求的是用户可操作的界面和双端行为一致。
- **明确放弃**目前没有条目。任何一项要转为“明确放弃”，都要先有决策记录和你的确认，不能由执行者自行删除。
- 本表按 Tauri 的 `src/components/` 功能目录、`src/i18n/locales/en.json` 的文案分组和[设置页逐页清单](settings-inventory.md)的 14 页建立。设置页每一页单列，工具功能按用户可感知的完整流程单列。

## 终端

| 功能                             | Tauri 实现                                                                       | GPUI 实现                                                                              | 状态   | 验收证据与剩余工作                                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 本地 PTY 会话                    | `src/components/TerminalTab/`、`src-tauri/src/terminal/`                         | `crates/tterm-gpui/src/terminal_demo/session.rs`、`window/session_workspace/launch.rs` | 进行中 | [P2-01](reports/P2-01.md) 已合入会话绑定；真实桌面长稳和多平台验收待补。                                                                                |
| SSH 会话（密码、私钥、主机密钥） | `src/components/TerminalTab/`、`src-tauri/src/ssh/`                              | `crates/tterm-core/src/ssh/`、`window/saved_connections/`                              | 进行中 | [首台 SSH 主机](reports/P4-01-first-ssh-macos.md) 已实测 macOS 密码与私钥登录；跳板、代理和高级认证界面未接。                                           |
| 跳板机与代理连接                 | `src/components/ConnectionDialog/JumpHost*`、`ProxyCommandPrompt/`               | `crates/tterm-core/src/ssh/`、`proxy/`                                                 | 未开始 | core 已有跳板与代理服务，但 GPUI 连接编辑器没有对应字段，也没有 ProxyCommand 审批界面。                                                                 |
| 输出、背压与 30 会话             | `src/components/TerminalTab/useTerminalLifecycle.ts`                             | `terminal_demo/pipeline.rs`                                                            | 进行中 | [P2-02](reports/P2-02.md) 与独立 release 帧/吞吐门已通过；完整终端桌面测量仍属 G2 缺口。                                                                |
| 键盘输入与输入法                 | `src/components/TerminalTab/terminalInput.ts`、`ime*.ts`                         | `crates/tterm-terminal-view/src/input/`                                                | 进行中 | [P2-03](reports/P2-03.md) 的绑定与自动化已合入；P2-11 的系统中文输入法矩阵未开始。                                                                      |
| 鼠标、选区与链接                 | `src/components/TerminalTab/useTerminalContextMenu.ts`、`mouseReportingGuard.ts` | `crates/tterm-terminal-view/src/view/`                                                 | 进行中 | [P2-04](reports/P2-04.md) 通过代码审查与自动化；真实 vim/tmux/htop 鼠标交互待补。                                                                       |
| 剪贴板、多行粘贴与 OSC 52        | `src/components/TerminalTab/useTerminalClipboard.tsx`                            | `terminal_demo/clipboard/`、`crates/tterm-terminal/src/osc52.rs`                       | 进行中 | [P2-05](reports/P2-05.md) 与 [P4-16 剪贴板](reports/P4-16-clipboard.md) 覆盖读写权限和粘贴确认；整页视觉与其他平台待补。                                |
| 滚动、回滚与终端搜索             | `src/components/TerminalTab/useTerminalSearch.ts`                                | `terminal_demo/window/search/`、`crates/tterm-terminal/src/search/`                    | 进行中 | [P2-06 滚动与内存](reports/P2-06-scroll-memory.md)、[P2-07 搜索栏](reports/P2-07-search-ui.md) 已通过自动验收；物理滚动和桌面输入待补。                 |
| 命令标记与概览                   | `src/components/TerminalTab/commandMarks.ts`                                     | `crates/tterm-terminal/src/command_mark/`、`command_marks_setting.rs`                  | 进行中 | [P2-06 概览标记](reports/P2-06-overview.md)、[P4-16 Shell 集成](reports/P4-16-shell-integration.md) 已接开关与复制脚本；命令跳转和真实 shell 行为待补。 |
| 终端右键菜单与快捷动作           | `src/components/TerminalTab/useTerminalContextMenu.ts`                           | `terminal_demo/window/context_menu/`                                                   | 进行中 | [P0-24 上下文菜单](reports/P0-24-context-menu.md) 已合入复制、粘贴、查找等基础动作；保存命令、重设终端等剩余项待对照。                                  |
| 终端主题与调色板                 | `src/components/TerminalTab/useTerminalTheme.ts`                                 | `crates/tterm-gpui/src/app_window/theme.rs`、`theme_setting.rs`                             | 进行中 | 预设及目录主题可在原窗口即时应用，已有和新开标签沿用连接覆盖规则，绘制与 OSC 查询使用同一调色板。[换肤验收](reports/P4-14-live-theme-library.md)。 |
| 关键词高亮                       | `src/components/TerminalTab/useKeywordHighlight.ts`、`keywordHighlighter.ts`     | `crates/tterm-core/src/keyword_highlight.rs`                                           | 未开始 | core 有匹配服务，GPUI 终端视图没有读取 `keyword_highlight` 配置，也没有 P4-19 页面。                                                                    |
| 预测回显与 sudo 提示             | `src/components/TerminalTab/predictiveEcho*.ts`、`sudoAutofill` 文案             | `crates/tterm-core/src/password_feedback.rs`、`saved_password_prompts.rs`              | 未开始 | core 有提示数据，GPUI 没有预测回显覆盖层或 sudo 提示条。                                                                                                |
| 串口终端                         | `src/components/TerminalTab/Serial*`                                             | `crates/tterm-core/src/serial/`                                                        | 未开始 | 只有 core 串口服务；GPUI 没有控制栏、发送栏、历史或 XMODEM 传输界面。                                                                                   |
| 服务器监控                       | `src/components/TerminalTab/ServerMonitor*`                                      | `crates/tterm-core/src/monitor/`                                                       | 未开始 | 只有 core 监控服务；GPUI 没有监控栏、详细面板或指标设置。                                                                                               |
| 会话日志与回放                   | `src/components/LogsPanel/`                                                      | `crates/tterm-core/src/session_log/`                                                   | 未开始 | 只有 core 日志与回放服务；GPUI 没有日志页、文本查看或回放控制。                                                                                         |
| ZMODEM 传输                      | `src/components/TerminalTab/useZmodemTransfers.ts`                               | `crates/tterm-core/src/zmodem/`                                                        | 未开始 | core 有会话槽位，GPUI 没有发送/接收对话框和状态提示。                                                                                                   |
| 旧版快照恢复                     | `src/components/TerminalTab/terminalSnapshot.ts`                                 | `terminal_demo/window/session_workspace/`                                              | 进行中 | [P2-19 只读恢复](reports/P2-19-read-only.md) 已有普通启动回归；完整 G2 签收仍需按[证据核查](reports/G2-evidence-review.md)复核。                        |
| AI 会话续接                      | `src/components/TerminalTab/agentResume.ts`                                      | `terminal_demo/window/agent_resume/`                                                   | 进行中 | [N-01](reports/N-01.md) 已合入；Windows 本地/SSH 手工验收和设置开关接入待补。                                                                           |

## 工作区

| 功能                             | Tauri 实现                                                                 | GPUI 实现                                                    | 状态   | 验收证据与剩余工作                                                                                                             |
| -------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------ |
| 标签新建、切换与关闭             | `src/components/TabBar.tsx`、`TabBar/`                                     | `terminal_demo/window/tabs.rs`、`strip.rs`                   | 进行中 | [P3-07 首批](status.md) 已有 Windows 无界面窗口回归；关闭确认和 SSH 多会话待补。                                               |
| 标签溢出、翻页与全部标签搜索     | `src/components/TabBar/TabOverflowPanel.tsx`                               | `terminal_demo/window/overflow.rs`                           | 进行中 | 状态记录覆盖 10～12 个本地会话的筛选、回绕和截图；真实 SSH 标签与鼠标翻页待补。                                                |
| 标签拖动排序与让位动画           | `src/components/TabBar/useTabDrag.ts`                                      | `terminal_demo/window/drag.rs`、`motion.rs`                  | 进行中 | [P3-07 桌面对照](reports/P3-07-desktop-comparison.md) 已修正缓动；Windows 桌面观感和跨平台待补。                               |
| 分屏、嵌套与窗格焦点             | `src/components/TTermApp/useWorkspaceSplits.ts`、`usePane*`                | `terminal_demo/window/split_state.rs`、`split_ui.rs`         | 进行中 | [P3-08 macOS 嵌套分屏](reports/P3-08-macos-nested-panes.md) 已通过；跨平台整卡验收未签收。                                     |
| 分隔条拖动                       | `src/components/TTermApp/WorkspaceSashes.tsx`                              | `terminal_demo/window/split_ui.rs`                           | 进行中 | [P3-08 分隔条](status.md) 有 Windows 真实鼠标路径回归；桌面手感和比例持久化待补。                                              |
| 布局模板                         | `src/components/LayoutTemplateDialog/`                                     | `terminal_demo/window/template_actions.rs`、`template_ui.rs` | 进行中 | [P3-08 模板互通](reports/P3-08-macos-template-interop.md) 已通过 macOS；双端桌面和整页视觉待补。                               |
| 工作区恢复                       | `src/components/TTermApp/useSessionRestore.ts`                             | `terminal_demo/window/session_workspace/`                    | 进行中 | 状态记录覆盖 Windows 隔离数据库与真实 PTY；真实 SSH/AI 重启和桌面验收待补。                                                    |
| 新标签启动器（本地 Shell）       | `src/components/NewTabLauncher/`、`TerminalTab/`                           | `terminal_demo/window/shell_launcher.rs`                     | 进行中 | [本地 Shell 与来源选择](reports/P3-11-P4-03-picker-acceptance.md) 已接默认 Shell、排序和启动；最近连接、搜索与快速连接未开始。 |
| 快速连接与最近命令               | `src/components/TTermApp/useQuickConnect.tsx`、`src/lib/recentCommands.ts` | 无                                                           | 未开始 | GPUI 启动器只有本地 Shell 列表，没有快速连接、最近连接或最近命令入口。                                                         |
| 标签与工作区右键菜单             | `src/components/ContextMenu.tsx`、`TabBar/`                                | `terminal_demo/window/context_menu/`                         | 进行中 | 已有终端右键菜单；标签的复制、重命名、移动窗格、重新打开关闭标签和固定连接头未补齐。                                           |
| 标签重命名、复制、移动与关闭保护 | `src/components/RenameDialog.tsx`、`CloseRequestDialog.tsx`                | `terminal_demo/window/failed_tab.rs`                         | 未开始 | 没有重命名、复制标签、移动窗格到新标签；退出/传输/未保存文件的关闭确认未接。                                                   |
| 标签图标与 Agent 状态指示        | `src/components/TabIcon.tsx`、`TabIconPicker/`、`TabAttentionDot.tsx`      | `crates/tterm-core/src/workspace/tab_attention.rs`           | 未开始 | core 有注意状态存储；GPUI 标签没有按 shell/连接显示图标，也没有 working/waiting 样式设置。                                     |
| 启动目录处理                     | `src/components/TTermApp/useLaunchDirs.ts`                                 | `crates/tterm-core/src/launch_dir.rs`                        | 未开始 | core 有启动目录事件；GPUI 没有“目录不存在”提示或把目录应用到新标签的完整流程。                                                 |
| 会话头与重连状态                 | `src/components/TerminalTab/ConnectionHeader.tsx`                          | `terminal_demo/window/failed_tab.rs`、`title.rs`             | 进行中 | 有基本标签标题与失败占位；连接头、重连重试状态、Shell 集成回退原因和等待输入提示未接。                                         |
| 欢迎页与错误空状态               | `src/components/TTermApp/EmptyState.tsx`、`ErrorBoundary.tsx`              | `terminal_demo/window/failed_tab.rs`                         | 进行中 | 有失败标签占位；欢迎页、无连接引导、配置读写失败和会话保存失败的完整文案与重试未接。                                           |
| 窗口、标题栏与窗口状态           | `src/components/TTermApp/AppTitleBar.tsx`                                  | `app_window/`、`terminal_demo/window/title.rs`               | 进行中 | [P3-01](reports/P3-01.md) 首批已合入；原生标题栏和跨平台窗口状态待补。                                                         |
| 设置路由、搜索与三个页面         | `src/components/SettingsDialog/`                                           | `terminal_demo/window/settings_page/`                        | 进行中 | [逐页清单](settings-inventory.md)：外观、终端、Shell 三页部分可操作，其余 11 页无入口。 |
| 国际化与语言切换                 | `src/i18n/`                                                                | `crates/tterm-core/src/i18n/`、`locale.rs`                   | 进行中 | [P3-03](reports/P3-03.md) 共享翻译已合入；真实窗口切换与所有新文案覆盖待补。                                                   |
| 托盘、应用菜单、单实例与系统通知 | `src-tauri/src/system_integration/`、`notification/`                       | `crates/tterm-platform/`                                     | 未开始 | 平台 trait 已有骨架，GPUI 没有托盘、应用菜单、单实例传参或通知点击路由；属 P5-01。                                             |

## 连接

| 功能                           | Tauri 实现                                                                                 | GPUI 实现                                           | 状态   | 验收证据与剩余工作                                                                                                                                      |
| ------------------------------ | ------------------------------------------------------------------------------------------ | --------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 已保存连接列表、分组树与搜索   | `src/components/ProfilesPanel/`                                                            | `terminal_demo/window/saved_connections/listing/`   | 进行中 | [连接列表 macOS 验收](reports/P4-01-ssh-library-macos.md)：树、搜索、折叠和最近使用已实测；长列表与跨平台待补。                                         |
| 分组管理、拖放、多选与批量删除 | `src/components/SettingsDialog/ProfileGroups*`、`ProfileDragRow.tsx`                       | `saved_connections/group_admin.rs`、`move_to_group.rs`、`delete_selected.rs` | 进行中 | 多选、批量移动到已有分组/未分组、二次确认批量删除、分组新建/重命名/删除已接入并有 UI 测试；拖放移动和完整桌面验收仍待补。                        |
| 连接编辑器基础字段             | `src/components/ConnectionDialog/`                                                         | `saved_connections/editor.rs`、`profile/`           | 进行中 | 已支持名称、主机、端口、用户名、分组、密码/私钥；与 Tauri 保存字段的完整对照待补。                                                                      |
| 连接编辑器高级字段             | `src/components/ConnectionDialog/`                                                         | `saved_connections/editor.rs`                       | 未开始 | 证书、agent、跳板机、代理、编码、终端主题、图标、会话自定义、串口与连接测试均无界面。                                                                   |
| SSH 配置导入                   | `src/components/SshConfigImportDialog/`                                                    | `saved_connections/import/`                         | 进行中 | [P4-03](reports/P4-03-ssh-config-import.md) 与[来源选择](reports/P3-11-P4-03-picker-acceptance.md) 已通过自动验收；系统选择窗口本身和跨平台桌面待复核。 |
| 认证、主机密钥与跳板信息提示   | `TerminalTab/HostKeyPromptDialog.tsx`、`SshAuthPromptDialog.tsx`、`JumpHostInfoDialog.tsx` | `terminal_demo/prompt.rs`                           | 进行中 | 首台 SSH 主机已实测主机密钥与密码提示；密钥变化、证书、`@revoked`、跳板链信息和 ProxyCommand 审批未验收。                                               |
| 代理服务器管理                 | `src/components/SettingsDialog/ProxySettingsTab.tsx`、`ProxyServerDialog.tsx`              | `crates/tterm-core/src/proxy/`                      | 未开始 | 只有 core 代理服务；GPUI 没有默认代理、服务器列表或凭据编辑界面。                                                                                       |
| 已知主机、SSH 密钥与密钥库     | `src/components/SettingsDialog/KnownHostsCard.tsx`、`SshKeysCard.tsx`                      | `crates/tterm-core/src/ssh/store.rs`、`key_file.rs` | 未开始 | core 存储存在；GPUI 没有已知主机、密钥导入、密钥库解锁或密钥存储方式界面。                                                                              |

## 工具

| 功能                     | Tauri 实现                                                                 | GPUI 实现                                             | 状态   | 验收证据与剩余工作                                                                                                |
| ------------------------ | -------------------------------------------------------------------------- | ----------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------- |
| 端口转发只读页           | `src/components/TunnelsPanel/`                                             | `terminal_demo/window/tunnel_page/`                   | 进行中 | [P3-09 端口转发](reports/P3-09-tunnels.md)：列表、状态、主机和路由结构与 Tauri 一致；新建、编辑、启停和删除未接。 |
| 隧道创建、启停与后台运行 | `src/components/TunnelsPanel/`                                             | `crates/tterm-core/src/tunnel/`                       | 未开始 | 只有 core 隧道服务；GPUI 没有编辑对话框、自动启动、凭据、通知或托盘入口。                                         |
| SFTP 浏览器              | `src/components/SftpDrawer/`                                               | `crates/tterm-core/src/sftp/`                         | 未开始 | 只有 core SFTP 服务；GPUI 没有目录列表、路径栏、选择、上传下载、搜索、压缩或 sudo 模式界面。                      |
| 传输管理器               | `src/components/TransferManager/`                                          | `crates/tterm-core/src/services/sftp.rs`              | 未开始 | 没有传输列表、进度、速度、剩余时间、取消、重试或清除已完成界面。                                                  |
| 远程文件编辑器           | `src/components/RemoteFileEditor.tsx`、`CodeMirrorEditor.tsx`              | `crates/tterm-core/src/external_editor.rs`            | 未开始 | 没有按连接/身份/路径打开的远程文件标签、保存冲突检测、编码与换行保留、查找替换或语法高亮。                        |
| 命令库、编写器与变量     | `src/components/CommandLibrary/`、`CommandComposer/`、`CommandVariables*`  | `crates/tterm-core/src/command_library/`、`composer/` | 未开始 | 只有 core 仓储与变量替换；GPUI 没有命令库列表、编辑器、标签管理、编写器或变量对话框。                             |
| 广播输入                 | `src/components/BroadcastManager.tsx`、`TTermApp/useBroadcast*.ts`         | 无                                                    | 未开始 | 没有广播目标选择、开始/停止、视觉提示或编写器多终端发送。                                                         |
| 主题库、编辑与导入       | `src/components/ThemeEditor/`、`ThemeGallery.tsx`、`ThemeImportDialog.tsx` | `theme_setting/library.rs`、`library_view.rs` | 进行中 | 可搜索、筛选、预览和应用共享目录主题；自定义编辑、导入和复制不在本批。 |

## 设置

设置页按[逐页清单](settings-inventory.md)的 14 个实际路由逐页列出。只有同时具备可操作控件、双端共享存储和整页视觉证据，才能记为“对齐”。

| 页面       | Tauri 页面                                        | 迁移卡              | GPUI 实现                                                                                                                             | 状态   | 验收证据与剩余工作                                                                                                                                                         |
| ---------- | ------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 连接分组   | `ProfileGroupsSettingsTab.tsx`                    | P4-01、P4-15        | 无                                                                                                                                    | 未开始 | 侧栏无该页；P4-01 的分组管理仍在进行。                                                                                                                                     |
| 连接       | `ConnectionSettingsTab.tsx`                       | P4-17               | 无                                                                                                                                    | 未开始 | 自动重连、预测回显、Shell 集成、跳板信息、sudo 与监控设置均未接。                                                                                                          |
| 代理       | `ProxySettingsTab.tsx`                            | P4-17               | 无                                                                                                                                    | 未开始 | 默认代理、服务器列表、凭据和删除确认均未接。                                                                                                                               |
| 外观       | `AppearanceSettingsTab.tsx`                       | P4-16、P4-14        | `theme_setting.rs`、`config/theme_file.rs` | 进行中 | 支持六套预设和完整主题目录，当前窗口无需重启，支持跟随系统。自定义主题、界面缩放、窗口模糊、标签和分屏控件仍待补。 |
| 终端       | `FontSettingsTab.tsx`、`ScrollbackLinesField.tsx` | P4-16               | `settings_page/`、`font_setting/`、`appearance_setting/`、`clipboard_setting.rs`、`command_marks_setting.rs`、`scrollback_setting.rs` | 进行中 | 16/16 个原生适用字段已接入；整页视觉、浅色/缩放、macOS/Linux Shell 行为和双客户端互写待补。见 [P4-16](reports/P4-16-shell-integration.md)。                                |
| Shell      | `LocalShells/LocalShellSettings.tsx`              | P4-17、P3-11        | `local_shells_setting/`、`settings_page/shell.rs`                                                                                     | 进行中 | Auto、检测、自定义、默认、隐藏、复制、排序、删除与路径检查已接；整页视觉、双端隔离配置桌面读写和另一平台复验待补。见[来源选择](reports/P3-11-P4-03-picker-acceptance.md)。 |
| 关键词高亮 | `HighlightSettingsTab.tsx`                        | P4-19               | 无                                                                                                                                    | 未开始 | 规则组、规则编辑、颜色、预览和恢复内置均未接。                                                                                                                             |
| 通知       | `NotificationSettingsTab.tsx`                     | P4-20、P4-26        | 无                                                                                                                                    | 未开始 | 通知开关、声音、权限、命令完成、终端请求、agent 集成和铃声均未接。                                                                                                         |
| 快捷键     | `KeymapSettingsTab.tsx`                           | P4-21               | 无                                                                                                                                    | 未开始 | GPUI 能按 `config.keymap` 分发，但没有录制、冲突检测、解绑或恢复默认界面。                                                                                                 |
| 日志       | `LoggingSettingsTab.tsx`                          | P4-20               | 无                                                                                                                                    | 未开始 | 日志开关、目录、格式、命名模板、容量、保留和状态均未接。                                                                                                                   |
| 数据迁移   | `DataMigrationSettingsTab.tsx`                    | P4-22、P4-23、P4-24 | 无                                                                                                                                    | 未开始 | 备份、WebDAV、导入和历史四个视图均无界面；core 已有 backup/sync 服务。                                                                                                     |
| 安全       | `SecuritySettingsTab.tsx`                         | P4-18               | 无                                                                                                                                    | 未开始 | 主密码、密钥库、密钥存储方式、已保存密码、SSH 密钥和已知主机均未接。                                                                                                       |
| 通用       | `GeneralSettingsTab.tsx`                          | P4-16、P4-17、P4-26 | 无                                                                                                                                    | 未开始 | 语言、更新渠道和 agent 续接开关等通用项未接。                                                                                                                              |
| 更新       | `UpdateSettingsTab.tsx`                           | P4-25               | 无                                                                                                                                    | 未开始 | 更新渠道、自动下载、手动检查、下载安装和关于/许可证均未接；依赖 P5-03。                                                                                                    |

## 应用与平台

| 功能                 | Tauri 实现                                                             | GPUI 实现                                   | 状态   | 验收证据与剩余工作                                                                                      |
| -------------------- | ---------------------------------------------------------------------- | ------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------- |
| 关于与第三方许可     | `src/components/AboutDialog/`                                          | 无                                          | 未开始 | 没有版本、项目链接、致谢或许可证页；P4-25 还要求从 Rust 依赖生成许可数据。                              |
| 自动更新             | `src/components/AppUpdateManager.tsx`、`src/lib/updater.ts`            | 无                                          | 未开始 | 没有 `latest.json` 读取、minisign 校验、下载安装或渠道切换；属 P5-03。                                  |
| 系统集成             | `src/components/SettingsDialog/SystemIntegrationSection.tsx`           | `crates/tterm-core/src/system_integration/` | 未开始 | 没有 shell 命令、文件夹菜单、PATH 或失败重试界面。                                                      |
| AI agent 集成安装    | `src/components/AgentIntegrationRow/`、`RemoteAgentIntegrationDialog/` | `crates/tterm-core/src/agent_integration/`  | 未开始 | core 有安装/钩子服务；GPUI 没有安装/卸载、配置目录、远程环境或通知开关界面。                            |
| 备份、同步与数据迁移 | `src/components/SettingsDialog/Backup*`、`WebDav*`                     | `crates/tterm-core/src/backup/`、`sync/`    | 未开始 | 只有 core 服务；没有导出、导入、历史、自动备份、WebDAV 或差异应用界面。                                 |
| 无障碍               | 现有 WebView 语义                                                      | 无                                          | 未开始 | 没有 VoiceOver/Narrator/Orca 验收，也没有标签、按钮、设置项和对话框的可访问性记录；属 P5-05。           |
| 打包、签名与更新发布 | `src-tauri/tauri.conf.json`、`.github/workflows/`                      | 无                                          | 未开始 | GPUI 仍由 `cargo build -p tterm-gpui` 构建，没有 NSIS/dmg/AppImage/deb/rpm 或签名流程；属 P5-02/P5-03。 |

## 覆盖统计

本表共列出 **75** 个功能行。按当前证据，状态分布为：

| 状态     | 数量 | 说明                                                                                  |
| -------- | ---- | ------------------------------------------------------------------------------------- |
| 对齐     | 0    | 目前没有一项满足整卡对齐门槛；局部通过都记在“进行中”。                                |
| 进行中   | 36   | 终端与调色板、主题库、工作区、连接、设置外壳与三页、分组管理切片、只读隧道页。 |
| 未开始   | 39   | 高级连接、SFTP、传输、远程编辑、命令库、广播、11 个设置页、平台能力。 |
| 明确放弃 | 0    | 目前没有经决策人确认的放弃项。                                                        |

因此 **P4-27 不能签收**。P4-27 的验收要求“矩阵中没有未开始”，当前仍有大量未开始项；这不是文档遗漏，而是实现缺口。进入 P5-04 之前必须先消除这些行，或为每一项取得明确的放弃决策。

## 主要缺口

按用户可感知的完整流程排序，影响最大的缺口是：

1. **SFTP、传输管理器和远程文件编辑**没有界面，三条流程都只有 core 服务。
2. **设置页 14 页中只有 3 页部分可操作**，连接、安全、快捷键、日志、数据迁移和更新都无入口。
3. **连接编辑器只覆盖基础字段**，跳板机、代理、证书、agent、串口和高级终端字段未接。
4. **工具类功能缺失**：命令库、编写器、广播、自定义主题编辑与导入、服务器监控、串口、日志回放和 ZMODEM。
5. **应用级能力缺失**：关于/许可、更新、系统集成、agent 集成、备份同步、无障碍和打包。

## 填写

- “现有实现位置”写 `src/` 中的组件目录或模块路径，必要时到文件。
- “原生实现位置”写 `crates/` 中的 crate、视图或模块路径。
- 功能划分参考 `src/components/` 的功能目录与 `src/i18n/locales/en.json` 的文案分组，至少覆盖设置页的每个设置项。
- P4-27 完成后，本表没有“未开始”；每个“对齐”都有证据。
