# P3 应用外壳、工作区与基础组件

目标：GPUI 版具备完整的应用外壳，可以进行日常 SSH 工作：多标签、分屏、快捷键、主题、会话恢复。切换布局不丢失会话，不误终止后台任务。

前提：P1-12 已完成。本阶段可以与 P2 并行，终端视图先用 P0 的原型代替。

## 现有结构

| 部分             | 位置                                       | 规模（不含测试） |
| ---------------- | ------------------------------------------ | ---------------- |
| 应用外壳与工作区 | `src/components/TTermApp/`、`TTermApp.tsx` | 约 5200 行       |
| 标签栏           | `src/components/TabBar/`、`TabBar.tsx` 等  | 约 1500 行       |
| 全局状态         | `src/contexts/`                            | 约 1800 行       |
| 共享 hooks       | `src/hooks/`                               | 约 1700 行       |
| 基础组件         | `src/components/ui/`                       | 约 2200 行       |

## P3-01 应用窗口与标题栏

- 依赖：P1-12
- 规模：M
- 参考：`src/components/TTermApp/AppTitleBar.tsx`、`TitleBarButtons.css`、`useAppChrome.ts`、`src/hooks/useWindowControls.ts`、`src-tauri/src/main_window.rs`、`window_blur.rs`、`config/window_background.rs`；nyaterm `src/window_chrome/` 与 issue #849、#1093
- 工作：
  1. 主窗口：最小尺寸，窗口位置、大小与最大化状态的保存与恢复，启动时使用保存的背景色以避免闪白。
  2. 标题栏：macOS 保留红绿灯按钮并在标题栏内放置标签；Windows 与 Linux 自绘标题栏按钮；Linux 使用系统窗口装饰时不重复显示标题。
  3. 窗口模糊与透明：macOS 毛玻璃效果，Windows 的 Mica 或 Acrylic，Linux 不透明。设置可在运行时切换。
- 验收：三平台截图；窗口状态在重启后恢复；Wayland 下系统装饰与自绘标题栏两种模式都正常。

## P3-02 全局状态模型

- 依赖：P1-12、P1-11c
- 规模：L
- 参考：`src/contexts/ConfigContext/`、`ThemeContext/`、`KeymapContext.tsx`、`AppActivityContext.tsx`、`TransferContext.tsx`、`UserVerificationContext.tsx`；`src/hooks/useSettingsSave.ts`、`useSyncedState.ts`
- 工作：
  1. 把每个 Context 映射为一个 GPUI 实体，按职责拆分，不做一个包含一切的全局状态。
  2. 配置：读取、修改、保存，以及修改后各视图的即时更新。保存走 core 的 `save_config` 等价接口，配置文件格式不变。
  3. 活动状态：窗口是否聚焦、是否有活动会话、退出时的阻塞项。
- 验收：修改任意设置后相关视图即时更新的测试；配置文件在 Tauri 版与 GPUI 版之间可以互相读取。
- 评估重点：订阅关系是否会造成不必要的整树重绘。

## P3-03 国际化

- 依赖：P1-12
- 规模：M
- 参考：架构约定 A8；`src/i18n/config.ts`、`src/i18n/locales/en.json`、`zh.json`（约 2800 个键，其中约 80 个复数键）；nyaterm 使用的 `rust-i18n`
- 工作：
  1. 在 Rust 中读取现有的两个 JSON 文件，不复制一份翻译。
  2. 支持 i18next 的插值语法 `{{name}}` 与复数后缀（`_one`、`_other` 等），转换在构建时或加载时完成，二者择一并说明理由。
  3. 运行时切换语言；首次启动时按系统语言选择（现有逻辑在 `src/i18n/language.ts`）。
  4. 测试：两个语言文件的键集合相同；GPUI 代码中引用的每个键都存在。
- 验收：上述测试；切换语言后所有可见文本更新。
- 评估重点：插值参数缺失与类型错误能否在编译时或测试中被发现。

## P3-04 动作与快捷键

- 依赖：P1-11e、P3-02
- 规模：L
- 参考：`src/lib/keymap/`、`src/contexts/KeymapContext.tsx`、`src-tauri/src/config/keymap.rs`、`TTermApp/useTabKeymap.ts`、`usePaneKeymap.ts`、`TerminalTab/useTerminalShortcuts.ts`；Zed 的 actions 与 keymap 组织方式
- 工作：
  1. 把现有的每个快捷键动作定义为 GPUI action，按上下文（全局、工作区、终端、SFTP、对话框）分组。
  2. 读取用户在配置中的自定义快捷键，格式不变；冲突检测与平台显示格式沿用 P1-11e 的移植结果。
  3. 终端获得焦点时，应用快捷键与终端输入的优先级与 P2-03 一致。
  4. 焦点管理：菜单、弹出层或面板关闭后焦点回到之前的终端，通知或其他入口可以把焦点送进指定标签的当前窗格（现有逻辑在 `src/lib/terminalFocus.ts`）；窗口重新获得焦点时恢复。
- 验收：一张动作清单，列出每个动作在两个客户端中的默认快捷键；自定义快捷键在两个客户端中效果相同；焦点恢复的测试。

## P3-05 界面主题系统

- 依赖：P1-11f、P3-02
- 规模：M
- 参考：`src/contexts/ThemeContext/`、`src/lib/uiStyle.ts`、`themeDerivation.ts`、`src/styles/`、`UiStyleSetting.tsx`、`src/hooks/useSystemPrefersDark.ts`
- 工作：把现有的 CSS 变量与 Tailwind 颜色映射为 GPUI 主题令牌；浅色、深色与跟随系统；从终端主题派生界面颜色；映射到 gpui-kit 的主题；界面缩放比例、窗口风格与加载动画风格等外观设置（现由 `src/contexts/ConfigContext/useWindowAppearance.ts` 应用）。视觉目标按 README 第 9 节的决定执行。
- 验收：内置主题下主要界面的截图与 Tauri 版对比；跟随系统时切换系统外观即时生效。

## P3-06 基础组件层

- 依赖：P3-05
- 规模：L
- 参考：`src/components/ui/`、`ContextMenu.tsx`、`RenameDialog.tsx`、`src/hooks/use-toast.ts`；gpui-kit 的组件
- 工作：
  1. 对照 `src/components/ui/` 的组件清单，在 `tterm-gpui` 中建立对应的组件：按钮、输入框、密码输入框（显示切换，内容不留在日志与撤销历史中）、文本域、选择框与可搜索选择框、复选框、开关、滑块、标签页、徽标、卡片、提示、弹出层、对话框、确认框、提示条（toast）、虚拟列表、表格、树、滚动区域、颜色选择器与色板、快捷键录入框、路径输入框（`ShellPathInput`，带存在性检查）。
  2. 加载与等待用户的状态动画：螃蟹在加载时夹钳、等待用户操作时敲击，小尺寸时使用简化图形（`src/components/Crab/`，图形来自 `src/assets/crab/` 与 `scripts/app-icon/crab.mjs`）；以及设置中可选的其他加载动画风格（`src/lib/spinnerStyle.ts`）。
  3. 优先封装 gpui-kit 的组件，统一 tTerm 的外观；gpui-kit 没有的再自行实现。
  4. 每个组件支持键盘操作与焦点样式。
- 产出：组件模块与一个展示所有组件的开发用窗口。
- 验收：组件清单的对照表；展示窗口在三平台的截图；键盘导航测试。
- 评估重点：组件 API 是否稳定到足以让 P4 的多个执行者同时使用。P4 开始后组件的破坏性修改需要评估。

## P3-07 标签栏

- 依赖：P3-04、P3-06、P1-11d
- 规模：L
- 参考：`src/components/TabBar/`、`TabBar.tsx`、`TabIcon.tsx`、`TabTitle.tsx`、`TabTooltip.tsx`、`TabAttentionDot.tsx`、`TabLogIndicator.tsx`、`TabIconPicker/`、`src/hooks/useTabs.ts`、`useTabContextMenu.ts`、`src/lib/tabGroups.ts`、`tabMenuActions.ts`
- 工作：标签的图标、标题、提醒点、日志指示、悬停提示；重命名、关闭、关闭其他、关闭右侧；拖动排序与拖到其他窗格；溢出时的滚动；标签分组；右键菜单；新建标签按钮。
- 验收：功能对照表；拖动排序与分组的交互录屏。

## P3-08 分屏与布局

- 依赖：P3-07
- 规模：L
- 参考：`TTermApp/WorkspacePane.tsx`、`WorkspacePanel.tsx`、`WorkspaceSashes.tsx`、`PaneHeader.tsx`、`usePaneDrag.ts`、`usePaneSlots.ts`、`useWorkspaceSplits.ts`、`useWorkspaceDrop.ts`、`workspaceDrop.ts`、`LayoutTemplateDialog/`、`src/lib/paneLayout.ts`、`paneNavigation.ts`、`paneTemplates.ts`
- 工作：水平与垂直分屏、拖动分隔条、拖动标签到窗格边缘创建分屏、窗格间的焦点导航快捷键、布局模板的保存与应用、关闭窗格时的会话处理。
- 验收：切换布局、关闭窗格、移动标签时会话不中断（自动化测试）；布局模板与 Tauri 版互通。
- 评估重点：迁移计划要求“会话由服务持有，不以视图是否存在决定生命周期”。

## P3-09 页面与面板路由

- 依赖：P3-06
- 规模：S
- 参考：`TTermApp/usePageTabs.ts`、`useTitleBarPages.ts`、`TabPanels.tsx`、`src/lib/appNavigation.ts`、`src/routes/`
- 工作：设置、连接、隧道、日志等页面的打开方式与切换，与 Tauri 版一致。
- 验收：从标题栏、快捷键、托盘、通知点击进入各页面都正常。

## P3-10 会话恢复与工作区持久化

- 依赖：P3-08
- 规模：M
- 参考：`TTermApp/useSessionRestore.ts`、`src/hooks/useSessionPersistence.ts`、`src/lib/tabsState.ts`、`tabGroupsPersistence.ts`
- 工作：启动时恢复上次的标签、分组、分屏与会话；保存的格式不变，Tauri 版保存的工作区可以在 GPUI 版中恢复，反之亦然。按 [N-01](new-features.md#n-01-重启后续接-ai-会话) 续接退出时仍在运行的 AI 会话，使用 N-01 移入 core 的续接命令模块，不在界面中另写一份。
- 验收：两个方向的恢复测试，使用真实的保存文件；保存文件中带续接记录的标签在两个客户端中都能续接。

## P3-11 新标签启动器与快速连接

- 依赖：P3-06
- 规模：M
- 参考：`src/components/NewTabLauncher/`、`TTermApp/useQuickConnect.tsx`、`src/lib/quickConnect.ts`、`SettingsDialog/LocalShells/`、`src-tauri/src/terminal/wsl_distros.rs`、`TTermApp/useLaunchDirs.ts`、`src-tauri/src/launch_dir.rs`
- 工作：新标签启动器（本地 shell、WSL 发行版、最近连接、搜索）；快速连接输入 `user@host:port`；从命令行或系统集成传入的启动目录。
- 验收：功能对照表；从资源管理器或 Finder 的“在 tTerm 中打开”进入时打开正确的目录。

## P3-12 退出与关闭保护

- 依赖：P3-02
- 规模：M
- 参考：`src/hooks/useQuitGuard.ts`、`CloseRequestDialog.tsx`、`src/lib/tabClosing.ts`、`TTermApp/useTabClosing.ts`、`src-tauri/src/background/`（`quit-requested`、`close-requested`、`app-will-quit`）
- 工作：关闭窗口时按设置最小化到托盘或退出；有活动会话、进行中的传输、运行中的隧道时确认；退出前等待日志写完等收尾工作。
- 验收：每种阻塞项的确认流程测试；强制退出时数据不损坏。

## P3-13 启动流程

- 依赖：P3-02、P3-05
- 规模：M
- 参考：`src/lib/startup.ts`、`VaultStartupUnlockDialog.tsx`、`TTermApp/useStartupVaultUnlock.ts`、`src-tauri/src/migrate.rs`、`db/legacy_import`、`src/lib/themePreloader.ts`、`ErrorBoundary.tsx`
- 工作：启动顺序（数据目录、旧数据导入、数据库迁移、密钥库解锁、主题、恢复工作区）；主密码解锁对话框；启动失败时显示可操作的错误；panic 时记录日志并显示错误而不是直接消失。
- 验收：首次启动、旧数据导入、主密码错误、数据库损坏四种情况的表现。
- 评估重点：启动时间，与 P0-01 的冷启动基线对比。

## P3-14 通知与提示

- 依赖：P3-06、P1-08
- 规模：M
- 参考：`src/hooks/useTerminalNotifications.tsx`、`src/lib/terminalNotifications.ts`、`notificationBadge.ts`、`notificationSound.ts`、`bellSound.ts`、`src-tauri/src/notification/`
- 工作：应用内提示条；错误的呈现规则沿用 `src/lib/errors.ts`（何时弹提示条、何时只记日志、标题与描述的来源）；系统通知及点击后定位到对应标签（`notification-activated`）；Dock 或任务栏角标；提示音与响铃声。
- 验收：三平台的系统通知点击后定位正确。

## P3-15 上下文菜单基础设施

- 依赖：P3-06
- 规模：S
- 参考：`src/components/ContextMenu.tsx`
- 工作：通用的上下文菜单：分组、分隔线、快捷键提示、子菜单、禁用项、键盘操作。
- 验收：组件测试；在标签、终端、SFTP 中使用的样例。

## P3-16 图标与静态资源

- 依赖：P0-08
- 规模：M
- 参考：`package.json` 中的 `lucide-react`；`src/assets/`（`themes`、`brand-logos`、`crab`）；`public/assets/distro-icons`；`src/lib/brandLogos.ts`、`src/components/SimpleIcon.tsx`、`BrandLogo.tsx`、`AgentLogo.tsx`、`TabIconPicker/glyphs.ts`；`scripts/update-themes.mjs`
- 工作：
  1. 界面图标：列出前端使用的 lucide 图标，导出为 SVG 资源嵌入 GPUI 程序，许可（ISC）登记到 `third-party.md`。
  2. 品牌标志、发行版图标、AI agent 标志、标签图标字形、螃蟹图形：与前端共用同一份源文件，不复制第二份。
  3. 主题库：`src/assets/themes` 由 `scripts/update-themes.mjs` 从 iTerm2-Color-Schemes 生成，两个客户端读取同一份数据；确认其许可的登记方式。
  4. 用 `rust-embed` 或 GPUI 的资源接口嵌入，开发构建可以直接读取文件。
- 产出：资源清单与嵌入代码。
- 验收：资源清单中的每一项在 GPUI 版中可以显示；没有在仓库中复制第二份的资源。

## G3 工作区可日常使用 `[人工]`

与 G2 一起进行：使用者用 GPUI 版完成一周日常工作，确认标签、分屏、快捷键、会话恢复没有阻塞性问题。
