# 设置页逐页清单（14 页）

本清单以 React 端**实际路由**的 14 个设置页为目录单位，供 GPUI 设置页逐页交付与验收。每页分开记录四件事：用户可见的控件与动作、GPUI 端的交互、双端共用的存储、视觉证据。缺少证据的一律写「待补」，不从前一项推断后一项。

- 路由来源：[`src/components/SettingsDialog/settingsTabs.ts`](../../src/components/SettingsDialog/settingsTabs.ts)（侧栏分组与顺序）、[`SettingsTabPanel.tsx`](../../src/components/SettingsDialog/SettingsTabPanel.tsx)（页面容器）、[`index.tsx`](../../src/components/SettingsDialog/index.tsx)（每一页对应的组件）。
- 迁移卡：[P4-15 至 P4-26 设置](p4-features.md#p4-15-至-p4-26-设置)；跨页的还有 P4-01、P4-14、P4-15。
- 存储字段以共享核心 [`AppConfig`](../../crates/tterm-core/src/config/mod.rs) 为准。[Rust 搜索索引](../../crates/tterm-core/src/settings/search_index.rs) 从 `settingsSearchIndex.ts` 派生，保留 14 页的目录，但排除 WebView 专有的 `terminal_renderer` 与 `hidden_renderer_release_secs`。GPUI 页面再按已接入的控件和平台过滤；索引有条目不代表页面已实现。见 [P4-16 渲染器设置报告](reports/P4-16-renderer-settings.md)。
- 本清单不是验收结论。共享配置结构已存在、搜索索引已有条目、页面壳已实现，都不能替代某一页的交互与视觉验收。
- 本清单以代码审计为基础。Shell 子集的自动验证见[本批验收](reports/P3-11-P4-17-local-shells.md)；其他页不借用这次测试作为验收证据。

## 1. 判定口径

| 维度      | 判为「对齐」的最低条件                                                                                       | 本清单如何写                                       |
| --------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| GPUI 交互 | 侧栏能进入该页；页面上的每个控件可操作；键盘、焦点、禁用、校验、失败重试都有实现；无「只摆入口不接线」的控件 | 按页写「未实现 / 进行中 / 局部」，并列出缺的控件   |
| 双端存储  | 两端读写同一处；一端保存后重启另一端能读到同一值；另一端改动不会被旧快照静默覆盖                             | 列出 config 键、SQLite 表、密钥库或 app state      |
| 视觉一致  | 有该页 React 参照屏，有同状态 GPUI 截图，比较结果覆盖整页；局部区域比较不能算整页通过                        | 无屏写「待补」；只有局部写「局部」并注明区域与差异 |
| 证据      | 报告或截图路径可复现；只写「已实现」不算证据                                                                 | 每页给报告/屏/资产链接；缺就写「待补」             |

状态词只用于本清单：`无` = 没有该页；`进行中` = 当前工作树正在实现；`局部` = 只覆盖列明的子集；`待补` = 缺证据。

## 2. 共同外壳

14 页共用一个外壳，验收时先确认外壳未退化，再逐页看内容：

- 侧栏四组、14 行，顺序固定为 Workspace（Groups、Connection、Proxies）、Experience（Appearance、Terminal、Shells、Highlight、Notifications、Keymap）、Data & security（Logging、Data migration、Security）、Application（General、Updates）。图标见 `settingsTabs.ts`。
- 页面容器 `SettingsTabPanel` 提供 `role="tabpanel"`、`aria-labelledby`、`tabIndex=0` 与 `data-parity="settings.layout.panel"`；`terminal` 与 `shells` 两页由容器自身滚动（`scroll`），其余页在页内放 `ScrollArea`。
- 搜索：NFKC + 小写归一；查询的每个词都要命中标题、描述或页名，且至少一个词命中标题或描述本身；结果含「页」行与「设置」行；`font` 作为旧值归一为 `terminal`（`settingsLoaders.ts`）。搜索框支持清除、方向键、Enter、Escape 两级返回。
- 懒加载只覆盖主题编辑器、主题库、主题导入三个对话框（`settingsLazy.ts`）；14 个页面组件在 `index.tsx` 中静态导入。P4-15 要求的页面级懒加载没有实现，不能写成已有。
- 保存：除字体/终端页的整页 Save（GPUI 页内另有回滚行数的 Save）外，控件大多即时调用 `useSettingsSave` → `ConfigContext.saveConfig` → `save_config`，失败弹 toast 并保留界面。React 字体/终端页把 18 个字段作为一次草稿保存（`useTerminalSettingsDraft.ts`）。
- 打开设置默认页是 General（React），GPUI 默认是 Terminal；`defaultTab` 支持深链。
- 设置入口在 GPUI 是单例路由，打开时隐藏窗格但保留标签、分屏和会话；Escape 先清搜索再返回原窗格。详见 [P4-15 报告](reports/P4-15.md)。

### 2.1 GPUI 当前页面集合

当前工作树里只有 2 个可操作的页签，都不等于整页对齐：

| 页面     | GPUI 状态              | 实际内容                                                                                    | 说明                                                                                  |
| -------- | ---------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Terminal | 局部 | 字体与间距、光标、剪贴板行为与 OSC 52 权限、macOS Option、命令标记与本机 Shell 集成、回滚、三侧内边距及预览 | 16/16 个与 GPUI 相关的字段已迁移；另有 2 个 WebView 渲染器字段不适用；见[第六批证据](reports/P4-16-shell-integration.md) |
| shells   | 局部（`82b8ee12`） | Auto、自动发现、自定义 shell 列表、新增/编辑（程序、名称、参数）、设为默认、隐藏/显示、复制、排序、删除、路径检查、显式保存、失败保留草稿、Escape/Enter 处理 | 双端隔离配置桌面读写、Shell 页同状态整页视觉和另一平台复验待补 |

侧栏只出现 Experience 组与这两行；搜索列出已实现的字体、预览、回滚和 Shell 条目。因此“搜索能打开某页”不能当作该页实现。

## 3. 总览

| #   | 页面（value）    | React 页面组件                                                                                     | 迁移卡              | 存储概览                    | GPUI 交互 | 视觉证据 |
| --- | ---------------- | -------------------------------------------------------------------------------------------------- | ------------------- | --------------------------- | --------- | -------- |
| 1   | `profile-groups` | [`ProfileGroupsSettingsTab.tsx`](../../src/components/SettingsDialog/ProfileGroupsSettingsTab.tsx) | P4-01、P4-15        | SQLite                      | 无        | 待补     |
| 2   | `connection`     | [`ConnectionSettingsTab.tsx`](../../src/components/SettingsDialog/ConnectionSettingsTab.tsx)       | P4-17               | config                      | 无        | 待补     |
| 3   | `proxies`        | [`ProxySettingsTab.tsx`](../../src/components/SettingsDialog/ProxySettingsTab.tsx)                 | P4-17               | config + SQLite + 密钥库    | 无        | 待补     |
| 4   | `appearance`     | [`AppearanceSettingsTab.tsx`](../../src/components/SettingsDialog/AppearanceSettingsTab.tsx)       | P4-16、P4-14        | config + SQLite             | 无        | 待补     |
| 5   | `terminal`       | [`FontSettingsTab.tsx`](../../src/components/SettingsDialog/FontSettingsTab.tsx)                   | P4-16               | config                      | 局部      | 局部     |
| 6   | `shells`         | [`LocalShellSettings.tsx`](../../src/components/SettingsDialog/LocalShells/LocalShellSettings.tsx) | P4-17、P3-11        | config                      | 局部      | 待补     |
| 7   | `highlight`      | [`HighlightSettingsTab.tsx`](../../src/components/SettingsDialog/HighlightSettingsTab.tsx)         | P4-19               | config                      | 无        | 待补     |
| 8   | `notifications`  | [`NotificationSettingsTab.tsx`](../../src/components/SettingsDialog/NotificationSettingsTab.tsx)   | P4-20、P4-26        | config + 应用数据           | 无        | 待补     |
| 9   | `keymap`         | [`KeymapSettingsTab.tsx`](../../src/components/SettingsDialog/KeymapSettingsTab.tsx)               | P4-21               | config                      | 无        | 待补     |
| 10  | `logging`        | [`LoggingSettingsTab.tsx`](../../src/components/SettingsDialog/LoggingSettingsTab.tsx)             | P4-20               | config + 日志目录           | 无        | 待补     |
| 11  | `data-migration` | [`DataMigrationSettingsTab.tsx`](../../src/components/SettingsDialog/DataMigrationSettingsTab.tsx) | P4-22、P4-23、P4-24 | SQLite + config + WebDAV    | 无        | 待补     |
| 12  | `security`       | [`SecuritySettingsTab.tsx`](../../src/components/SettingsDialog/SecuritySettingsTab.tsx)           | P4-18               | 密钥库 + SQLite + config    | 无        | 待补     |
| 13  | `general`        | [`GeneralSettingsTab.tsx`](../../src/components/SettingsDialog/GeneralSettingsTab.tsx)             | P4-16、P4-17、P4-26 | config                      | 无        | 待补     |
| 14  | `updates`        | [`UpdateSettingsTab.tsx`](../../src/components/SettingsDialog/UpdateSettingsTab.tsx)               | P4-25               | config + app state + 更新器 | 无        | 待补     |

## 4. 逐页清单

### 4.1 `profile-groups` — 连接分组

- **迁移卡**：P4-01；页面框架 P4-15。
- **React**：[`ProfileGroupsSettingsTab.tsx`](../../src/components/SettingsDialog/ProfileGroupsSettingsTab.tsx)、[`ProfileGroupsToolbar.tsx`](../../src/components/SettingsDialog/ProfileGroupsToolbar.tsx)、[`ProfileGroupTree.tsx`](../../src/components/SettingsDialog/ProfileGroupTree.tsx)、[`ProfileDragRow.tsx`](../../src/components/SettingsDialog/ProfileDragRow.tsx)、[`useProfileGroupsData.ts`](../../src/components/SettingsDialog/useProfileGroupsData.ts)、[`useProfileGroupActions.ts`](../../src/components/SettingsDialog/useProfileGroupActions.ts)、[`useProfileSelection.ts`](../../src/components/SettingsDialog/useProfileSelection.ts)、[`useProfileListActions.ts`](../../src/components/SettingsDialog/useProfileListActions.ts)。
- **GPUI 交互**：无。侧栏没有该页；已保存连接面板（P3-09）只负责浏览与启动，不提供分组维护。
- **双端存储**：SQLite 的 `profile_groups` / `profiles`；命令 `list_profile_groups`、`list_profiles`、`create_profile_group`、`rename_profile_group`、`move_profile_group`、`delete_profile_group`、`move_profiles_to_group`、`delete_profile`。GPUI 未接这些命令。
- **视觉/证据**：待补。无该页参照屏，也无 GPUI 页面。

控件与动作：

- 工具栏：新组名称输入 + 新增；搜索连接；选择匹配项；未分组区与根放置区。
- 分组树：新增子组、重命名（就地输入）、删除（确认，显示组内连接数）、展开/折叠、拖拽移动组、拖拽连接跨组、拖到根、拖拽数量提示；组名校验（不能含分隔符、同级不能重名）；`groupBusy` 时禁用。
- 连接行：连接、编辑、删除、勾选；本地/串口徽标；空搜索结果的专用空状态。
- 批量条：已选数量、清除选择、删除选中（确认）；`profileSelectionBusy` 时禁用。

待补：上述每一组动作的 GPUI 实现、拖拽落点、错误与空状态、该页参照屏与 GPUI 截图。

### 4.2 `connection` — 连接

- **迁移卡**：P4-17。
- **React**：[`ConnectionSettingsTab.tsx`](../../src/components/SettingsDialog/ConnectionSettingsTab.tsx)、[`PredictiveEchoSection.tsx`](../../src/components/SettingsDialog/PredictiveEchoSection.tsx)、[`ShellIntegrationSection.tsx`](../../src/components/SettingsDialog/ShellIntegrationSection.tsx)、[`SudoPromptPatternsSetting.tsx`](../../src/components/SettingsDialog/SudoPromptPatternsSetting.tsx)、[`MonitorMetricRow.tsx`](../../src/components/SettingsDialog/MonitorMetricRow.tsx)、[`monitorMetricOrder.ts`](../../src/components/SettingsDialog/monitorMetricOrder.ts)。
- **GPUI 交互**：无。相关字段在 GPUI 一个都没有读取。
- **双端存储**：全部在 `config.json`：`reconnect_enabled`、`reconnect_max_attempts`、`predictive_echo`、`predictive_echo_threshold_ms`、`ssh_shell_integration`、`show_jump_host_connection_info`、`password_feedback`、`sudo_prompt_patterns`、`monitor_refresh_interval_secs`、`monitor_visible_metrics`。
- **视觉/证据**：待补。

控件与动作：

- 连接恢复：自动重连开关 `[config: reconnect_enabled]`；尝试上限 1–99 `[config: reconnect_max_attempts]`（关闭时禁用，失焦/Enter 提交，越界回夹）。
- 输入：预测回显 `auto/always/off` `[config: predictive_echo]`；阈值 10–2000 ms `[config: predictive_echo_threshold_ms]`（仅 `auto` 可用）。
- Shell 集成：SSH 会话默认注入集成开关 `[config: ssh_shell_integration]`。
- 跳板机：显示跳板连接信息开关 `[config: show_jump_host_connection_info]`。
- Sudo：密码反馈开关 `[config: password_feedback]`；额外提示正则（默认折叠，摘要显示已有条数，多行编辑，逐行校验，失焦保存）。
- 监控：刷新间隔 1–60 s `[config: monitor_refresh_interval_secs]`；状态栏指标多选（至少保留 1 个）与拖拽排序 `[config: monitor_visible_metrics]`，另有“恢复默认”。

待补：GPUI 交互与存储接线、该页参照屏。

### 4.3 `proxies` — 代理

- **迁移卡**：P4-17。
- **React**：[`ProxySettingsTab.tsx`](../../src/components/SettingsDialog/ProxySettingsTab.tsx)、[`ProxyServerDialog.tsx`](../../src/components/SettingsDialog/ProxyServerDialog.tsx)、[`ProxyServerFields.tsx`](../../src/components/SettingsDialog/ProxyServerFields.tsx)、[`useProxyServers.ts`](../../src/hooks/useProxyServers.ts)。
- **GPUI 交互**：无。
- **双端存储**：默认项 `default_proxy_id` 在 config；服务器条目在 SQLite `proxies`（`list_proxies`、`save_proxy`、`delete_proxy`）；代理密码在已保存密码存储。
- **视觉/证据**：待补。

控件与动作：

- 默认代理：下拉（无、各服务器；默认项已被删除时额外列出该值，并在说明中标红）。
- 列表：条目名、摘要、用户名、默认徽标；加载失败行；空列表文案；新增、编辑、删除。
- 编辑对话框：名称、类型（HTTP / SOCKS5 / 命令）、主机、端口、用户名、密码（已保存时显示徽标，可清除）、命令（`command` 类型）；保存与取消。
- 删除：确认，显示“被 N 个连接使用”；删除默认项时清空默认值；失败 toast。
- 说明：首跳说明固定显示。

待补：GPUI 交互、密码存储互通、该页参照屏，以及代理在真实 SSH 会话中的连通性与失败提示（人工）。

### 4.4 `appearance` — 外观

- **迁移卡**：P4-16；主题库/编辑/导入为 P4-14。
- **React**：[`AppearanceSettingsTab.tsx`](../../src/components/SettingsDialog/AppearanceSettingsTab.tsx)、[`ThemeLibrarySettings.tsx`](../../src/components/SettingsDialog/ThemeLibrarySettings.tsx)、[`WindowBlurSection.tsx`](../../src/components/SettingsDialog/WindowBlurSection.tsx)、[`useWindowBlurDraft.ts`](../../src/components/SettingsDialog/useWindowBlurDraft.ts)、[`UiStyleSetting.tsx`](../../src/components/SettingsDialog/UiStyleSetting.tsx)、[`TabSettingsSection.tsx`](../../src/components/SettingsDialog/TabSettingsSection.tsx)、[`TabIconSettings.tsx`](../../src/components/SettingsDialog/TabIconSettings.tsx)、[`TabAgentIndicatorSettings.tsx`](../../src/components/SettingsDialog/TabAgentIndicatorSettings.tsx)、[`PaneSettingsSection.tsx`](../../src/components/SettingsDialog/PaneSettingsSection.tsx)。
- **GPUI 交互**：无。GPUI 只从 `app_meta` 的已保存背景色选择内置深/浅色板，不读 `theme`、`ui_style`、`ui_scale_percent`、窗口模糊、标签或窗格配置。
- **双端存储**：`config.json` 的 `theme`、`theme_follow_system`、`theme_light`、`theme_dark`、`favorite_themes`、`ui_scale_percent`、`ui_style`、`window_blur`、`window_blur_material`、`window_blur_radius`、`window_opacity_percent`、`tab_width_mode`、`tab_standard_width`、`tab_hover_tooltip`、`tab_icons`、`tab_agent_indicator`、`panes.show_headers`、`panes.dim_inactive_percent`、`panes.templates`；自定义主题与预设覆盖在 SQLite `custom_themes`。GPUI 只读写 `panes.templates`（且入口在 P3-08 布局模板对话框，不在本页）。
- **视觉/证据**：待补。无该页参照屏与 GPUI 页面。

控件与动作：

- 主题：跟随系统开关；预设主题卡（选中、编辑、复制、覆盖后“还原预设”、“显示更多/更少”）；主题库卡（收藏/取消收藏、复制并编辑）；自定义主题（选中、编辑、复制、删除）；打开主题库、导入主题、新建主题三个对话框入口。
- 界面：界面字号缩放 80–200%（步长 10，拖动即时预览，提交时保存，失败回滚，另有重置 100%）`[config: ui_scale_percent]`；界面风格 classic/islands `[config: ui_style]`。
- 窗口（条件项）：模糊开关与不透明度 30–95% 仅在 macOS/Windows 显示；材质 Acrylic/Mica 仅 Windows；模糊半径 1–60 仅 macOS；均为即时预览、失焦/抬手提交。
- 标签：宽度模式 adaptive/standard；标准宽度 80–300 px 仅在 standard 模式出现；悬停提示开关；标签图标（总开关；开启后才显示 Agent 徽标与按 shell 的图标行，可逐个改图标/颜色或重置）；Agent 指示（working/waiting 各自样式与颜色）。
- 分屏：显示窗格标题；非活动窗格变暗百分比（0 为关）；已保存布局模板列表与删除。

平台/条件：窗口模糊整段按平台出现；材质与半径各自再分平台；标签图标的两行依赖总开关。

待补：GPUI 全部控件、主题与自定义主题互通、浅色/islands/125% 证据、该页参照屏。

### 4.5 `terminal` — 终端

- **迁移卡**：P4-16。
- **React**：[`FontSettingsTab.tsx`](../../src/components/SettingsDialog/FontSettingsTab.tsx)、[`FontFamilyPicker.tsx`](../../src/components/SettingsDialog/FontFamilyPicker.tsx)、[`CursorStylePicker.tsx`](../../src/components/SettingsDialog/CursorStylePicker.tsx)、[`ScrollbackLinesField.tsx`](../../src/components/SettingsDialog/ScrollbackLinesField.tsx)、[`TerminalPaddingFields.tsx`](../../src/components/SettingsDialog/TerminalPaddingFields.tsx)、[`useTerminalSettingsDraft.ts`](../../src/components/SettingsDialog/useTerminalSettingsDraft.ts)。
- **GPUI 交互**：局部。16 个适用字段显式保存；其中 Option 作为 Meta 仅在 macOS 可见，本机 Shell 集成仅在非 Windows 可见并在命令标记关闭时禁用。默认 OSC 52 可写不可读。
- **双端存储**：React 可一次保存 18 个终端字段；GPUI 对已实现的 16 个字段用一次文件更新，只改用户修改的字段。`terminal_renderer` 与 `hidden_renderer_release_secs` 仍保留在共享配置格式中供 React/WebView 使用；GPUI 不读取也不写入，但页面保存会保留另一端写入的值。隔离文件读回已测，双客户端桌面互写待验。
- **视觉/证据**：局部。5 张 GPUI 截图只比较标题/搜索区与回滚字段；侧栏页面集合、拉丁字体宽度、数字步进器、Save 位置均有已知差异，整页未通过。见 [P4-15 报告](reports/P4-15.md) 与 [资产](reports/assets/settings-scrollback-flow/)。

控件与动作：

- 文本渲染：字号预设按钮 + 6–72 输入；行高 1–2（步长 0.05）；字距 -5–10；字体选择器（系统字体加载中/失败提示）。终端渲染器 WebGL/Canvas 与隐藏终端释放渲染器时限（0/10/30/60/300 秒）仅存在于 React/WebView；GPUI 使用原生渲染，本页不提供这两个控件。
- 终端行为：光标样式；macOS 的 Option 作为 Meta；多行粘贴确认；选中即复制；右键粘贴；OSC 52 写剪贴板；命令标记（含复制 POSIX/Fish 集成脚本）；本机 shell 集成（Windows 不显示，命令标记关闭时禁用）；回滚行数（预设/自由/无限）；左/右/下内边距。
- 预览：正文预览与 Nerd Font 预览，按当前字体、字号、行高、字距渲染。
- 保存：整页一个 Save，保存中禁用并显示进度文案。

平台/条件：`mac_option_is_meta` 仅 macOS；`local_unix_shell_integration` 仅非 Windows 且依赖命令标记。

待补：无（命令标记、本机 Shell 集成及复制 POSIX/Fish 脚本按钮均已接入，见[本批报告](reports/P4-16-shell-integration.md)）。WebView 渲染器选择与隐藏释放时限不计入 GPUI 待补项，共享配置兼容见 [P4-16 渲染器设置报告](reports/P4-16-renderer-settings.md)。另需复核整页布局、浅色、多缩放和跨平台桌面。

### 4.6 `shells` — Shell

- **迁移卡**：P4-17；与 P3-11 新标签启动器共用子集。
- **React**：[`LocalShells/LocalShellSettings.tsx`](../../src/components/SettingsDialog/LocalShells/LocalShellSettings.tsx)、[`LocalShellEditor.tsx`](../../src/components/SettingsDialog/LocalShells/LocalShellEditor.tsx)、[`LocalShellRow.tsx`](../../src/components/SettingsDialog/LocalShells/LocalShellRow.tsx)、[`useLocalShellSettings.ts`](../../src/components/SettingsDialog/LocalShells/useLocalShellSettings.ts)、[`src/lib/localShells.ts`](../../src/lib/localShells.ts)。
- **GPUI 交互**：`82b8ee12` 已接入 Auto、检测与自定义条目、设为默认、隐藏/显示、复制、排序、删除和缺程序提示。窗口测试与真实桌面证据分开记录。
- **双端存储**：`config.json`：`local_shells`、`hidden_shells`、`terminal_shell`、`terminal_shell_id`、`terminal_shell_custom_path`、`terminal_shell_custom_args`、`terminal_shell_wsl_distro`。GPUI 保存时先读最新文件再应用共享规则；检测与路径检查不创建配置条目。
- **视觉/证据**：待补。GPUI 截图工具仍只生成 Terminal 页的 5 个状态，没有 Shell 页。

控件与动作：

- 列表：Auto（推荐）；检测到的 shell（按路径或 WSL 发行版，含复制、隐藏/显示，默认项不能隐藏）；自定义 shell（名称、程序与参数摘要、默认星标、可上移/下移、编辑、删除）；徽标（默认、已检测、已隐藏）；路径失效提示。
- 排序：自定义项上下移动；默认项不能隐藏；删除不询问但只删自定义项。
- 编辑器：程序路径（带文件选择）、名称（占位为程序名）、参数、设为默认（已是默认时显示说明）、取消/保存（程序为空时保存禁用）。
- 数据来源：`list_available_terminal_shells`、`check_local_shell`。

程序文件选择及启动器菜单复核见[执行与验收记录](reports/P3-11-P4-03-picker-acceptance.md)。待补：双端隔离配置桌面读写、Shell 页同状态整页视觉与另一平台复验。

### 4.7 `highlight` — 关键词高亮

- **迁移卡**：P4-19。
- **React**：[`HighlightSettingsTab.tsx`](../../src/components/SettingsDialog/HighlightSettingsTab.tsx)、[`HighlightSetToolbar.tsx`](../../src/components/SettingsDialog/HighlightSetToolbar.tsx)、[`HighlightRuleList.tsx`](../../src/components/SettingsDialog/HighlightRuleList.tsx)、[`HighlightRuleEditor.tsx`](../../src/components/SettingsDialog/HighlightRuleEditor.tsx)、[`HighlightColorPicker.tsx`](../../src/components/SettingsDialog/HighlightColorPicker.tsx)、[`HighlightPreview.tsx`](../../src/components/SettingsDialog/HighlightPreview.tsx)、[`useHighlightSettings.ts`](../../src/components/SettingsDialog/useHighlightSettings.ts)。
- **GPUI 交互**：无。GPUI 不读 `keyword_highlight`。
- **双端存储**：`config.json` 的 `keyword_highlight`：`enabled`、`default_set`、`alternate_screen`、`sets`（每套 `id/name/rules`，每条 `id/enabled/pattern/kind/case_sensitive/whole_word/foreground/background`）。
- **视觉/证据**：待补。

控件与动作：

- 顶部：启用开关、默认规则组下拉、备用屏是否高亮开关。
- 规则组工具栏：组选择、新建、重命名、复制、删除（确认）、恢复内置组（确认）。
- 规则列表：选择、逐条启用/停用、上移/下移、删除、空列表文案；新增规则（达到上限时禁用）。
- 规则编辑：匹配类型 keyword/regex、区分大小写、全词匹配、前景色、背景色（含“无”和自定义色）。
- 预览：可编辑的示例文本 + 预览区（沿用当前主题色与字体）。

待补：整套 GPUI 交互、非法正则提示、上限与恢复内置的确认流程、参照屏。

### 4.8 `notifications` — 通知

- **迁移卡**：P4-20；AI agent 集成为 P4-26。
- **React**：[`NotificationSettingsTab.tsx`](../../src/components/SettingsDialog/NotificationSettingsTab.tsx)、[`NotificationSoundRow.tsx`](../../src/components/SettingsDialog/NotificationSoundRow.tsx)、[`AgentIntegrationSection.tsx`](../../src/components/SettingsDialog/AgentIntegrationSection.tsx)、[`AgentIntegrationRow`](../../src/components/AgentIntegrationRow/index.tsx)。
- **GPUI 交互**：无。GPUI 不读通知、铃声或 agent 配置。
- **双端存储**：`config.json`：`notifications_enabled`、`notify_sound`、`notify_sound_name`、`notify_sound_file`、`toast_max_visible`、`notify_command_finished`、`notify_command_min_secs`、`notify_terminal_requests`、`notify_agent_waiting`、`notify_agent_done`、`agent_config_dirs`、`bell_style`、`bell_notify`。自定义提示音由 `import_notification_sound` 复制进应用数据，config 只存文件名；权限查询用 `notification_permission`；agent 状态与安装用 `agent_integration_status`、`install_agent_integration`、`uninstall_agent_integration`。
- **视觉/证据**：待补。

控件与动作：

- 通知：总开关；声音开关；声音选择（系统/内置音/自定义文件）、试听、选择文件；提示框最多条数 1–5；发送测试通知；权限或回退提示的告警条（按权限状态出现）。
- 事件：命令完成开关与最短时长 1–3600 s（关闭时禁用）；本机 shell 未启用命令标记时的警告与“打开终端设置”跳转（按平台换文案）；终端请求通知开关。
- Agent 集成：每个 agent 的安装/卸载、配置目录选择与重置、等待/完成通知开关（按状态与错误显示重试）。
- 铃声：样式 visual/sound/none；通知开关（样式为 none 时禁用）。

条件/平台：声音与权限提示依赖系统权限；命令完成提示与本地 shell 集成状态相关。

待补：GPUI 交互、系统通知权限与自定义提示音的实际路径、agent 集成命令、参照屏。

### 4.9 `keymap` — 快捷键

- **迁移卡**：P4-21。
- **React**：[`KeymapSettingsTab.tsx`](../../src/components/SettingsDialog/KeymapSettingsTab.tsx)、[`src/lib/keymap/actions.ts`](../../src/lib/keymap/actions.ts)、[`src/lib/keymap/keymap.ts`](../../src/lib/keymap/keymap.ts)、[`KeymapContext.tsx`](../../src/contexts/KeymapContext.tsx)。
- **GPUI 交互**：无。GPUI 只按 `config.keymap` 路由按键，没有编辑界面。
- **双端存储**：`config.json` 的 `keymap.bindings`（动作 id → 组合键数组或 `null` 表示解绑）。
- **视觉/证据**：待补。

控件与动作：

- 动作分组列表；逐项录制新组合键（录制中屏蔽全局分发，Escape 取消）。
- 清除绑定（Backspace/Delete）、恢复单项默认、恢复全部默认。
- 冲突检测：冲突时拒绝保存并指出占用动作；`tabs.switchToNth` 必须录 1–9，录完后一次性生成九个绑定；CLI 冲突提示（`getCliConflictHint`）。
- 平台：macOS 与 Windows/Linux 的组合键显示与解析不同。

待补：录制交互、冲突提示、平台键位显示、解绑与重置、参照屏。

### 4.10 `logging` — 日志

- **迁移卡**：P4-20。
- **React**：[`LoggingSettingsTab.tsx`](../../src/components/SettingsDialog/LoggingSettingsTab.tsx)、[`logNameTemplate.ts`](../../src/components/SettingsDialog/logNameTemplate.ts)、[`useTerminalLogStatus.ts`](../../src/components/SettingsDialog/useTerminalLogStatus.ts)。
- **GPUI 交互**：无。GPUI 不读 `terminal_log_*`。
- **双端存储**：`config.json`：`terminal_log_enabled`、`terminal_log_record_input`、`terminal_log_directory`、`terminal_log_format`、`terminal_log_plain_timestamps`、`terminal_log_name_template`、`terminal_log_max_file_size_mb`、`terminal_log_compress`、`terminal_log_retention_days`、`terminal_log_max_total_mb`；日志文件在所选目录；状态用 `get_terminal_log_status` 读取，操作用 `open_terminal_log_directory`、`retry_terminal_logging`；“浏览日志”跳到日志页。
- **视觉/证据**：待补。

控件与动作：

- 开关：启用日志、记录输入；安全告警条（随“记录输入”切换文案）；浏览日志按钮。
- 存储：目录只读显示 + 选择目录 + 打开目录（无目录时禁用）；格式 raw/plain/both 三选一；纯文本时间戳（raw 格式时禁用）；文件名模板（非空、≤180 字符、目录层级上限，显示示例名，失焦保存）；单文件上限 1–1024 MiB；压缩。
- 清理：保留天数 0–3650；总大小上限 0–1,048,576 MiB；非法值就地提示、不保存。
- 状态：记录中/已停止、活动会话数、磁盘占用；最近错误与重试（`get_terminal_log_status`）。

条件：纯文本时间戳依赖格式；状态与错误由后端返回（可能为空）。

待补：GPUI 交互、目录选择器、状态刷新与失败重试、旧日志数据兼容、参照屏。

### 4.11 `data-migration` — 数据迁移

- **迁移卡**：P4-22（备份）、P4-23（同步）、P4-24（迁移页）。同步卡嵌在 WebDAV 视图内，不另开页面，所以仍是 14 页。
- **React**：[`DataMigrationSettingsTab.tsx`](../../src/components/SettingsDialog/DataMigrationSettingsTab.tsx)、[`MigrationViewTabs.tsx`](../../src/components/SettingsDialog/MigrationViewTabs.tsx)、[`dataMigration.ts`](../../src/components/SettingsDialog/dataMigration.ts)、[`backupShared.ts`](../../src/components/SettingsDialog/backupShared.ts)、[`AutomaticBackupCard.tsx`](../../src/components/SettingsDialog/AutomaticBackupCard.tsx)、[`BackupExportCard.tsx`](../../src/components/SettingsDialog/BackupExportCard.tsx)、[`BackupImportCard.tsx`](../../src/components/SettingsDialog/BackupImportCard.tsx)、[`BackupHistoryCard.tsx`](../../src/components/SettingsDialog/BackupHistoryCard.tsx)、[`WebDavBackupPanel.tsx`](../../src/components/SettingsDialog/WebDavBackupPanel.tsx)、[`WebDavBackupOptions.tsx`](../../src/components/SettingsDialog/WebDavBackupOptions.tsx)、[`WebDavServerFields.tsx`](../../src/components/SettingsDialog/WebDavServerFields.tsx)、[`WebDavSyncCard.tsx`](../../src/components/SettingsDialog/WebDavSyncCard.tsx)。
- **GPUI 交互**：无。共享核心有 backup/sync 服务，但没有设置页界面。
- **双端存储**：自动备份设置与同步状态在 SQLite（`app_meta`、`sync_state`）；备份历史是备份目录中的文件列表（含自动备份目录）；WebDAV 密码在已保存密码存储的 `webdav:password`；导出文件在用户选择的位置。
- **视觉/证据**：待补。

控件与动作：

- 视图切换：备份 / WebDAV / 导入 / 历史（四个按钮，非页签语义），一次只显示一个视图。
- 备份视图：自动备份卡（开关、频率 off/daily/weekly、保留份数、目录选择、立即备份）；导出卡（勾选内容：设置、连接、会话、已知主机、SFTP 目录、命令库、主题、日志、密钥；密码与确认密码、显示密码；导出）。
- WebDAV 视图：服务器与账号字段、测试连接、保存、上传、下载、删除所选、清理旧备份、下载并恢复；未配置/加载中/锁定/空列表状态；已配置后显示同步卡（同步类别、推送/拉取间隔、立即同步、重置同步状态）。
- 导入视图：选择备份文件、输入密码、验证、检查结果摘要（版本、创建时间、连接数、密钥数、缺密钥文件、设置变化）、冲突策略 merge/replace、导入、重启提示。
- 历史视图：备份列表（时间、大小、来源）、用于导入、删除（确认）；空列表文案。

条件：自动备份仅在设置读取成功时渲染；WebDAV 未配置时不显示列表与同步；密码验证通过后才允许导入。

待补：GPUI 全部交互、真实旧数据上的密钥与备份验证、WebDAV 测试与失败路径、参照屏。

### 4.12 `security` — 安全

- **迁移卡**：P4-18。
- **React**：[`SecuritySettingsTab.tsx`](../../src/components/SettingsDialog/SecuritySettingsTab.tsx)、[`SecretStorageSection.tsx`](../../src/components/SettingsDialog/SecretStorageSection.tsx)、[`SecurityCards.tsx`](../../src/components/SettingsDialog/SecurityCards.tsx)、[`ChoosePasswordModeCard.tsx`](../../src/components/SettingsDialog/ChoosePasswordModeCard.tsx)、[`UnlockVaultCard.tsx`](../../src/components/SettingsDialog/UnlockVaultCard.tsx)、[`SystemUnlockFailedCard.tsx`](../../src/components/SettingsDialog/SystemUnlockFailedCard.tsx)、[`MasterPasswordCard.tsx`](../../src/components/SettingsDialog/MasterPasswordCard.tsx)、[`SavedSecretsCard.tsx`](../../src/components/SettingsDialog/SavedSecretsCard.tsx)、[`SshKeysCard.tsx`](../../src/components/SettingsDialog/SshKeysCard.tsx)、[`ImportSshKeyDialog.tsx`](../../src/components/SettingsDialog/ImportSshKeyDialog.tsx)、[`KnownHostsCard.tsx`](../../src/components/SettingsDialog/KnownHostsCard.tsx)、[`useVaultActions.ts`](../../src/components/SettingsDialog/useVaultActions.ts)、[`useSshKeyActions.ts`](../../src/components/SettingsDialog/useSshKeyActions.ts)。
- **GPUI 交互**：无。GPUI 没有密钥库、已保存密码、SSH 密钥或已知主机界面。
- **双端存储**：存储模式在 `secret_storage_mode`、启动解锁提示在 `prompt_unlock_vault_on_startup`；密码/密钥材料在密钥库（secrets 表 + 包装密钥）；SSH 密钥在共享 `ssh::key_vault`；已知主机在 SQLite `known_hosts`；命令含 `list_vault_keys`、`import_vault_key`、`rename_vault_key`、`set_vault_key_certificate`、`export_vault_key`、`delete_vault_key`、`list_known_hosts`、`delete_known_hosts`。
- **视觉/证据**：待补。触达真实系统凭证，必须保留人工验收，不能用合成数据代替。

控件与动作：

- 存储：状态徽标（已解锁/已锁定/禁用/迁移中）；存储模式 system/password/memory（system 在钥匙串不可用时禁用，迁移中禁用）；钥匙串不可用告警；切换到密码模式（新密码 + 确认）；解锁卡（迁移/设置密码/解锁/用恢复密码解锁，按状态出现）；系统解锁失败卡（重试、重置）；锁定。
- 密码：修改主密码；设置/移除恢复密码；启动时提示解锁。
- 已保存密码：列表（种类 sudo/ssh/proxy/jump host）、显示/隐藏、删除、空列表。
- SSH 密钥：导入对话框（名称、密钥文件、口令、记住口令、证书）；列表（复制公钥、重命名、设置/替换/移除证书、导出、删除、使用位置、加密/口令已保存徽标）；锁定或 memory 模式的不可用提示。
- 已知主机：搜索、列表（主机、信任时间、跳板标记）、删除（确认）、空列表/无匹配、加载失败。
- 危险动作走系统验证（`withVerification`）。

条件：解锁卡与主密码卡按模式、是否已设密码、是否解锁、是否迁移中分别出现；system 模式与钥匙串可用性相关；memory 模式不显示主密码卡，但已保存密码与 SSH 密钥卡仍在（显示不可用或空状态）。

待补：GPUI 交互、真实钥匙串/系统认证、迁移与恢复路径、旧密钥数据、参照屏与人工验收记录。

### 4.13 `general` — 通用

- **迁移卡**：P4-16；SFTP 部分 P4-17；系统集成为 P4-26。
- **React**：[`GeneralSettingsTab.tsx`](../../src/components/SettingsDialog/GeneralSettingsTab.tsx)、[`SftpSettingsSection.tsx`](../../src/components/SettingsDialog/SftpSettingsSection.tsx)、[`BandwidthLimitInput.tsx`](../../src/components/SettingsDialog/BandwidthLimitInput.tsx)、[`SystemIntegrationSection.tsx`](../../src/components/SettingsDialog/SystemIntegrationSection.tsx)、[`useSystemIntegration.ts`](../../src/components/SettingsDialog/useSystemIntegration.ts)。
- **GPUI 交互**：无页面。GPUI 启动时读取 `language`、`restore_terminal_content`、`resume_agent_sessions`，但没有对应开关或其他控件。
- **双端存储**：`config.json`：`language`、`sftp_*`、`zmodem_*`、`startup_session_restore_mode`、`restore_terminal_content`、`resume_agent_sessions`、`close_behavior`、`background_throttling`。系统集成走后端命令：`shell_command_status` / `set_shell_command`（带管理员提示文案）、`folder_menu_status` / `set_folder_menu`；“关于”打开独立对话框；清除会话调用后端。
- **视觉/证据**：待补（React 有该页参照屏 `settings.layout.default`，GPUI 无该页）。

控件与动作：

- 语言：英文/中文按钮（当前项勾选）。
- SFTP：起始目录下拉；粘贴上传开关；并行传输通道 1–16；上传/下载限速（KB/MB + 数值，0 为不限，失焦/Enter 提交）；外部编辑器路径（选择/清除）；外部修改后回传 ask/auto。
- ZMODEM：自动检测开关；下载目录（只读显示 + 选择）。
- 启动：恢复全部会话连接；恢复终端内容；重启后续接 AI 会话。
- 后台：关闭主窗口行为 ask/tray/quit；macOS 的隐藏时后台策略 throttle/disabled/suspend，改动后出现“重启以生效”。
- 系统集成：shell 命令与文件夹菜单两个开关（按后端支持与加载状态出现，失败可重试，文案分 macOS）。
- 高级：打开开发者工具。
- 维护：清除会话数据（危险按钮）、关于。

平台/条件：后台策略仅 macOS；系统集成按平台与后端能力显示；关闭行为在 Linux 有额外说明。

待补：GPUI 全部交互（尤其 SFTP 与系统集成）、重启生效流程、清除会话的确认与失败、GPUI 该页与参照屏对比。

### 4.14 `updates` — 更新

- **迁移卡**：P4-25（依赖 P5-03 更新器接口）。
- **React**：[`UpdateSettingsTab.tsx`](../../src/components/SettingsDialog/UpdateSettingsTab.tsx)、[`src/lib/updater.ts`](../../src/lib/updater.ts)、[`UpdateReleaseNotes.tsx`](../../src/components/UpdateReleaseNotes.tsx)、[`AboutDialog`](../../src/components/AboutDialog/index.tsx)。
- **GPUI 交互**：无。GPUI 不读更新配置，也没有更新器界面。
- **双端存储**：`config.json`：`update_channel`、`auto_download_updates`、`update_check_frequency`；上次检查时间在 app state（`lastUpdateCheckAt`）；下载/安装与发布说明由更新器运行时提供。
- **视觉/证据**：待补。依赖真实更新源与签名，自动化只能覆盖状态机与文案。

控件与动作：

- 渠道：stable / beta-dev 两个按钮（当前项徽标，beta 有提示）。
- 自动下载开关；自动检查频率 daily / every-3-days / weekly / never；上次检查时间。
- 手动检查；检查中、无更新、有更新（下载并安装，显示进度与发布说明）、已下载（安装/重启）、准备就绪（重启）、失败（重试）、已下载但不可用等状态。
- 发布说明区块。

条件：按钮按更新状态切换；重启动作由应用活动上下文提供。

待补：GPUI 交互、真实更新源与签名验证、各状态截图与人工验收。

## 5. 证据索引

| 证据                                                                      | 覆盖                                                                                                         | 不覆盖                                                             |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| [P4-15 报告](reports/P4-15.md)                                            | 设置路由、外壳、侧栏、搜索、回滚行数流程；双 PTY 交互；局部几何与像素误差                                    | 其余 13 页、整页视觉、浅色/islands/125%、桌面窗口                  |
| [W0-05 设置参照报告](reports/W0-05-settings-reference.md)                 | React `settings.layout.default`（General 页）与 `settings.search.results`（font 查询）                       | 任何 GPUI 设置页、页面内容控件                                     |
| [settings-scrollback-flow 资产](reports/assets/settings-scrollback-flow/) | 5 张 React 参照与 5 张 GPUI 截图、布局 JSON、误差清单                                                        | Section 4 其余页；整页 SSIM                                        |
| [`parity/screens.json`](../../parity/screens.json)                        | `settings.layout.default`、`settings.search.results`、`settings.terminal.scrollback`、四张 scrollback 搜索屏 | 12 个未实现页面                                                    |
| [P3-08 状态记录](status.md)                                               | 布局模板的保存/应用/删除已接入 GPUI                                                                          | 它不是设置页 Appearance 的通过证据；模板列表仍缺页面控件与视觉证据 |
| [`docs/gpui-migration/parity/screens.md`](parity/screens.md)              | 设置页相关状态清单与 `data-parity` 标记                                                                      | 不能据此判定 GPUI 交互或视觉                                       |

现有设置截图只支持设置外壳的局部几何与 General 页的 React 参照。搜索/回滚流程和[自定义 Shell 的保存与启动](reports/P3-11-P4-17-local-shells.md)另有自动验证；这些不是整页视觉证据。

## 6. 共同待补与风险

1. **写整份配置会覆盖另一端的改动。** React 的 `saveConfig` 把内存中的整份 `AppConfig` 发给 `save_config`；GPUI 的回滚行数、Shell、布局模板保存则先读最新文件再只改自己负责的字段或条目。后者不会覆盖前者，但前者在旧快照上保存会覆盖 GPUI 刚写入的值。字体/终端页一次保存 18 个字段，最容易出现“改了终端回滚行数，另一端保存字体页又把它改回去”。每页验收时要包含一次跨端写入后重开的检查。
2. **侧栏与搜索不是完整索引。** GPUI 侧栏只有 Experience 组的 2 行，搜索只列已实现条目；其余 12 页在 React 的搜索索引中可搜到，但 GPUI 不列。
3. **Shell 页只交付自定义条目子集。** 已提交 `9eb24d8b`；双客户端桌面读写与同状态整页截图仍待补。
4. **视觉证据只有一页且只是局部。** Terminal 页的对比只覆盖标题/搜索区与回滚字段，且已知侧栏集合、字体宽度、步进器、Save 位置差异；General 只有 React 参照屏，没有 GPUI 截图；其余 12 页连参照屏也没有。每页至少需要默认态 + 空/错误/禁用态 + 平台条件态，不能只截默认页。
5. **平台专有条件没有自动证据。** 窗口模糊与材质、后台策略、系统钥匙串与系统解锁、系统集成菜单、目录/文件选择器、更新器、通知权限，都需要在 macOS 与 Windows 上人工核对；Linux 按项目现状另记。本文不把这些列为已验。
6. **浅色、islands、125% 未验。** 现有 GPUI 截图与参照都只是 dark/classic/100%；P4-15 报告已明确这一点，本文沿用。
7. **任务卡不等于页面。** [`status.md`](status.md) 的 P4 汇总行只记录已交付的子集；判定单页还缺什么，以本清单列出的控件和证据为准。

## 7. 不在本清单内

- 标题栏的回滚行数编辑器（`scrollback_setting.rs` 的对话框模式）与新标签启动器（P3-11）本身：它们复用设置数据，但不是 14 页之一。
- 布局模板对话框（P3-08）中的保存/应用/删除：它写 `panes.templates`，与 Appearance 页共用配置，但不属于该页的控件验收。
- 设置之外的对话框：主题库/编辑器/导入（P4-14 的独立对话框）、关于、确认/输入/信息对话框、代理与密钥导入对话框只在所属页内作为子流程记录。
- 备份、同步、密钥库、更新器的服务层与真实外部资源：本清单只记它们与页面的接线与用户可见结果。
