# W0-05 工作区与设置首页参照首批

## 结论

状态：**完成待评估**。本批只交付 React 参照端的三屏，不修改 GPUI 产品代码，不推送、不合并，也不把这三屏当成 W0-05 完成。

- 复现了已有的 `workspace.titlebar.default`，新增 `settings.layout.default` 与 `settings.search.results`。
- 设置页复用生产 `SettingsPanel`，走 `WorkspacePanel` 为 `settings` 页签使用的同一组件和 `.workspace-panel` 页面容器；搜索词由捕获动作真实键入，没有预置结果。夹具只在 `src/parity/` 的类型检查边界内提供假数据。
- 三屏都在 macOS 上用 Playwright WebKit 1.54.0（revision 2191）以 800×600 CSS、DPR 2、深色、`classic`、100%、`en-US` 捕获。两次干净捕获的 PNG、layout JSON 和 manifest 逐字节相同。
- B1–B4 的 29 个 `workspace.*` 场景中，本批只有 `workspace.titlebar.default` 可复现；2 个须真实窗口行为（失焦、窗口模糊）人工确认；其余 26 个仍缺夹具或整屏入口。详细盘点见下文。
- 前端检查全部通过；Rust `fmt` 通过，`clippy` 与 `cargo test --workspace` 在未改动的 Rust 代码上有既有失败，见“检查结果”。

## 改动

| 内容                              | 位置                                                                                                                   |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 设置页的稳定布局标记              | `src/components/SettingsDialog/{index,SettingsSidebar,SettingsTabPanel,SettingsSearchField,SettingsSearchResults}.tsx` |
| 生产设置页的 parity 入口          | `src/parity/screens/SettingsReference.tsx`                                                                             |
| 类型正确的合成夹具                | `parity/fixtures/settings.reference.json`、`src/parity/screens/settingsReferenceFixture.ts`                            |
| 屏幕注册                          | `src/parity/screens.ts`、`src/parity/fixtures.ts`、`parity/screens.json` 的 `referenceScreens`                         |
| 搜索结果的点击回归                | `src/parity/screens/settingsReference.test.tsx`                                                                        |
| 主题 provider 所需的窗口桩        | `src/parity/tauri/webviewWindow.ts`、`docs/gpui-migration/parity/README.md`                                            |
| 截图、layout JSON 与捕获 manifest | `docs/gpui-migration/reports/assets/W0-05-settings-reference/`                                                         |

`data-parity` 只增加属性，没有改 class、事件、焦点顺序或文本。标记如下：

- `settings.layout.root`、`settings.layout.header`、`settings.layout.sidebar`、`settings.layout.panel`：整页、标题/搜索区、侧栏和内容区。
- `settings.layout.tab.<页签>`：侧栏每一行。行位置是设置页的主要布局，逐行标记后才能比出间距和截断。
- `settings.search.field`、`settings.search.results`、`settings.search.result.<索引>`：搜索框、结果列表和结果行；索引来自真实搜索结果顺序。

设置页的 `SettingsPanel` 会导入全部页签组件，因此 parity 入口包了生产 `ConfigProvider`、`ThemeProvider`、`AppActivityProvider`。主题 provider 会调用 `getCurrentWebviewWindow().setBackgroundColor()`；原来的 `webviewWindow` 桩一调用就报错，所以只把这一方法改成空操作，并补 `save_window_background` 夹具。其余窗口方法仍然报错。这是让生产设置页能渲染的最小工具扩展，不改变真实应用行为。

## 复现条件

| 项                 | 值                                                                           |
| ------------------ | ---------------------------------------------------------------------------- |
| 机器               | macOS 27.2（Build 26B123），arm64                                            |
| Node / pnpm        | v24.20.0 / 11.25.0                                                           |
| Playwright         | 1.54.0，WebKit revision 2191（`~/Library/Caches/ms-playwright/webkit-2191`） |
| 捕获提交           | `17d5a379edc1dbce82a6276ec1b114eb0c1e0eab`，干净工作树                       |
| 视口               | 800×600 CSS 像素，DPR 2，输出 PNG 1600×1200                                  |
| 主题 / 风格 / 缩放 | `dark` / `classic` / 100%                                                    |
| locale / 时区      | `en-US` / `UTC`                                                              |
| 其他上下文         | `colorScheme: dark`，`reducedMotion: reduce`                                 |
| 前端入口           | `vite --mode parity`，由捕获脚本自己启动，不占用 1420/1423                   |

在仓库根执行：

```sh
rm -rf target/parity/w0-05-clean-1 target/parity/w0-05-clean-2
node scripts/parity/capture.mjs \
  --screen workspace.titlebar.default \
  --screen settings.layout.default \
  --screen settings.search.results \
  --out target/parity/w0-05-clean-1
node scripts/parity/capture.mjs \
  --screen workspace.titlebar.default \
  --screen settings.layout.default \
  --screen settings.search.results \
  --out target/parity/w0-05-clean-2
```

产物在 `target/parity/w0-05-clean-{1,2}/`。报告资产是第一次运行的副本：

- `workspace.titlebar.default.png` / `workspace.titlebar.default.layout.json`
- `settings.layout.default.png` / `settings.layout.default.layout.json`
- `settings.search.results.png` / `settings.search.results.layout.json`
- `reference-screens.json`：记录 `commit`、主题、风格、缩放和视口。

两次运行的 `shasum -a 256` 完全一致：

| 文件                                     | SHA-256（两次相同）                                                |
| ---------------------------------------- | ------------------------------------------------------------------ |
| `settings.layout.default.png`            | `60bffb75333a18e4635429113d1f5f037a753e98ba5790dfdf29a748fccf17aa` |
| `settings.layout.default.layout.json`    | `d4dea2d7711654f9e832bcdef0e28d000d69a608cf175f265fca7009d827fb94` |
| `settings.search.results.png`            | `915425b3389f077c93636acd0c88d356fb757443ed4a516f2e53266b38e58518` |
| `settings.search.results.layout.json`    | `04a775c4ec91e395fa5f95369dcbf95f398db962b8ac52d7a9fa0b7b7212037f` |
| `workspace.titlebar.default.png`         | `eed9b2a57c8c084fe391a80576db6a2d43d94c4bafa75727f53c0c5fbb41121b` |
| `workspace.titlebar.default.layout.json` | `d8fecee4e9132433b5a8d81e58cfaf7449ce124b07f68ecec106a0cd0e3e5572` |
| `reference-screens.json`                 | `2050a09b138b90b9fddbf00f8d9194ffed415f73c8bea860573d39e21d88a918` |

验证命令：

```sh
diff -u \
  <(cd target/parity/w0-05-clean-1 && shasum -a 256 * | sort) \
  <(cd target/parity/w0-05-clean-2 && shasum -a 256 * | sort)
```

输出为空。`reference-screens.json` 中的 `commit` 为 `17d5a379edc1dbce82a6276ec1b114eb0c1e0eab`。

## 三屏

| 屏幕                         | 生产入口                                                                  | 夹具                                                           | 动作                                              | 元素 / 文字区 |
| ---------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------- | ------------- |
| `workspace.titlebar.default` | `AppTitleBar` 使用的 `TitleBarTabs` + `TitleBarActions`，两个合成本地标签 | `workspace.titlebar.default.json` → `workspaceTitleBarFixture` | 无                                                | 13 / 4        |
| `settings.layout.default`    | `SettingsPanel`（`WorkspacePanel` 的 `settings` 分支）                    | `settings.reference.json` → `settingsReferenceFixture`         | 无                                                | 152 / 123     |
| `settings.search.results`    | 同一个 `SettingsPanel`，真实 `useSettingsSearch` → `searchSettings`       | `settings.reference.json` → `settingsReferenceFixture`         | 聚焦搜索框后键入 `f`、`o`、`n`、`t`，等待结果列表 | 25 / 28       |

### 默认页签

生产入口有两层：

- `useTitleBarPages.ts` 在标题栏打开设置时调用 `openSettingsTab(settingsTabTitle)`，不传 section。
- `useTabs.ts` 的 `openPageTab` 把 `settingsSection` 原样保存；`WorkspacePanel.tsx` 再把它传给 `SettingsPanel` 的 `defaultTab`。
- `SettingsPanel` 的 `defaultTab` 默认值是 `"general"`，`normalizeSettingsTab` 只把旧的 `"font"` 映射到 `"terminal"`。

所以从标题栏打开设置时的真实默认页签是 **General**。截图和回归测试都验证了 `aria-selected` 落在 `settings-tab-general`，内容区为 `settings-panel-general`。

### 搜索与点击

`settings.search.results` 没有在入口里写死结果或查询词。捕获动作对 `settings.search.field` 发送四个真实按键；`SettingsSearchField` 的 `onChange` 更新 `useSettingsSearch`，结果由生产 `searchSettings` 从 `settingsSearchIndex.ts` 排序。`en-US` 下输入 `font` 得到三条：

1. `Terminal` → Font Family
2. `Terminal` → Font Size
3. Appearance → Interface font scale

结果截图是在上述输入后、点击任何结果之前截取的；第一条的高亮是 `activeIndex = 0` 的生产样式，不是点击后的页面。

点击回归在 `settingsReference.test.tsx` 中另有一步，不把点击后的页面当作结果截图：

```sh
pnpm exec vitest run src/parity/screens/settingsReference.test.tsx --reporter=verbose
```

测试做三件事：

- 断言 `SettingsPanel` 首次打开时 `General` 页签被选中，`settings-panel-general` 存在。
- 用 `fireEvent.change` 在真实搜索框里输入 `font`，断言结果列表的三行文本与 `settings.search.result.0` 标记。
- 点击第一行 `Font Family`，断言侧栏 `Terminal` 变为 `aria-selected="true"`、`General` 变为 `false`，`settings-panel-terminal` 内出现 `Font Family`，结果列表消失，并调用 `list_fonts`。

测试输出：3 个用例全部通过。该测试只渲染生产页面和真实搜索逻辑，不复制设置布局。

### 平台边界

- `workspace.titlebar.default` 是 **在 macOS 的 WebKit 中渲染的模拟 Windows 标题栏**：入口固定 `useWindowControls(true)`，右侧预留 128px 原生控件位；`isLinux={false}`，不画 Linux 按钮；没有 macOS 红绿灯；两个标签是 Windows 风格的合成本地 shell（`pwsh`、`cmd`）。它不是 macOS 原生标题栏的证据，也不是完整工作区截图。
- `settings.layout.default` 与 `settings.search.results` 是设置页的 **Web 内容区**，不包含原生标题栏、交通灯、窗口模糊或系统装饰。设置页在真实应用里由 `WorkspacePanel` 放在工作区页面中，parity 入口用同一个 `.workspace-panel` 容器提供全高页面。
- Playwright WebKit 与 Tauri 的 WKWebView 在 macOS 上同为 WebKit/CoreText，但捕获没有原生窗口壳；窗口级模糊、原生标题栏、系统文件选择器和系统通知仍只能人工确认。

## B1–B4 盘点

依据：`parity/screens.json`（本批后 16 个 paired 屏、5 个 reference-only 屏）、`docs/gpui-migration/parity/screens.md` 的 P3 段、`src/parity/screens.ts` 的已注册入口，以及实际组件目录。状态只有三种：

- **可复现**：当前 manifest、屏幕入口、夹具、动作都能生成稳定截图。
- **缺夹具/入口**：生产组件存在，但没有该状态的 parity 屏幕、夹具或整屏组合；不等于组件已对齐。
- **须人工**：依赖真实窗口、系统或原生装饰，浏览器捕获不能代表。

### B1 P3 外壳

| ID                                | 生产组件                                                     | 状态               | 依据与缺口                                                                           |
| --------------------------------- | ------------------------------------------------------------ | ------------------ | ------------------------------------------------------------------------------------ |
| `workspace.startup.loading`       | `TTermApp.tsx`、`ConfigContext`                              | 缺夹具/入口        | 需要完整应用壳和 `load_config`/`load_app_state` pending 指令；没有注册屏。           |
| `workspace.startup.empty`         | `EmptyState.tsx`、`WorkspacePanel.tsx`                       | 缺夹具/入口        | 需要空 `AppState.tabs`、空 `list_profiles` 的整屏；没有注册屏。                      |
| `workspace.startup.error`         | `ConfigContext`、`ErrorBoundary`                             | 缺夹具/入口        | 需要首个 `load_config` reject 和重试交互；没有注册屏。                               |
| `workspace.titlebar.default`      | `AppTitleBar.tsx`、`TitleBarTabs.tsx`、`TitleBarActions.tsx` | **可复现（部分）** | 已有入口和夹具，本批重新捕获。只是标题栏，且是模拟 Windows chrome；见“平台边界”。    |
| `workspace.titlebar.tunnel-badge` | `TitleBarActions.tsx`                                        | 缺夹具/入口        | 徽标是子部件；现有标题栏屏把 `runningTunnelCount` 固定为 0，没有 1/100 的变体屏。    |
| `workspace.window.unfocused`      | `TabPanels.tsx`、`AppActivityContext`                        | **须人工**         | 需要真实窗口失焦后再回来，浏览器动作不能稳定模拟系统焦点。                           |
| `workspace.window.blur`           | `useWindowAppearance.ts`、`themePreloader.ts`                | **须人工**         | 原生窗口级模糊；按 [ui-parity 第 4.1 节](../ui-parity.md) 只能人工判定，且平台不同。 |
| `workspace.toast.error`           | `ui/toast.tsx`、`useProfileActions.tsx`                      | 缺夹具/入口        | 可以用 command reject 触发生产 toast，但没有注册的整屏入口。                         |

### B2 P3 标签

| ID                                | 生产组件                                                   | 状态        | 依据与缺口                                                               |
| --------------------------------- | ---------------------------------------------------------- | ----------- | ------------------------------------------------------------------------ |
| `workspace.tab.single`            | `TabPanels.tsx`、`WorkspacePane.tsx`、`TabBar/TabItem.tsx` | 缺夹具/入口 | 需要标签树和终端会话的整屏；`TabItem` 单独截图不能代替。                 |
| `workspace.tab.modified`          | `TabBar/TabItem.tsx`、`TabTitle.tsx`                       | 缺夹具/入口 | 需要 `isModified=true` 的标签和整屏。                                    |
| `workspace.tab.attention`         | `TabAttentionDot.tsx`、`TabLogIndicator.tsx`               | 缺夹具/入口 | 需要 `ssh-auth-prompt-tab-*` 或日志状态事件；事件 mock 已有，但没有屏。  |
| `workspace.tab.long-title`        | `TabTitle.tsx`、`TabTooltip.tsx`                           | 缺夹具/入口 | 需要超长标题和悬停动作；动作 DSL 支持 hover，但没有屏。                  |
| `workspace.tab.overflow`          | `TabBar/TabOverflowPanel.tsx`、`useTabOverflowMenu.ts`     | 缺夹具/入口 | 需要 12 个以上标签和点击溢出按钮；没有屏。                               |
| `workspace.tab.overflow-filtered` | 同上                                                       | 缺夹具/入口 | 还需要在溢出面板里键入过滤词；可用现有 key 动作，但没有屏。              |
| `workspace.tab.context-menu`      | `ContextMenu.tsx`、`TabBar/TabItem.tsx`                    | 缺夹具/入口 | `ui.context-menu.*` 只是原语展示，不是标签右键菜单；不能拿它冒充本状态。 |
| `workspace.tab.rename`            | `RenameDialog.tsx`、`TabIconPicker/index.tsx`              | 缺夹具/入口 | 需要标签、重命名对话框、图标列表和 `set_profile_tab_icon` 夹具。         |
| `workspace.tab.icon-picker`       | `TabIconPicker/index.tsx`                                  | 缺夹具/入口 | 同上；图标选择器是子视图，不能单独代表整屏。                             |

### B3 P3 分屏

| ID                                      | 生产组件                                                        | 状态        | 依据与缺口                                                         |
| --------------------------------------- | --------------------------------------------------------------- | ----------- | ------------------------------------------------------------------ |
| `workspace.pane.single`                 | `WorkspacePane.tsx`、`PaneHeader.tsx`                           | 缺夹具/入口 | 窗格内含真实终端；需要终端会话/PTY 的呈现方案，当前没有屏。        |
| `workspace.pane.split-horizontal`       | `TabPanels.tsx`、`WorkspaceSashes.tsx`、`useWorkspaceSplits.ts` | 缺夹具/入口 | 需要 split tree、sash 位置和两个窗格；没有屏。                     |
| `workspace.pane.zoomed`                 | `PaneHeader.tsx`、`TabPanels.tsx`                               | 缺夹具/入口 | 需要 `zoomedTabId` 和分屏树；没有屏。                              |
| `workspace.pane.drop-preview`           | `TabPanels.tsx`、`usePaneDrag.ts`、`useWorkspaceDrop.ts`        | 缺夹具/入口 | 需要拖拽动作和落点预览；动作 DSL 有 drag，但没有屏。               |
| `workspace.layout-template.default`     | `LayoutTemplateDialog/index.tsx`                                | 缺夹具/入口 | 需要 `load_layout_templates` 的空/非空夹具；没有屏。               |
| `workspace.layout-template.saved`       | `LayoutTemplateDialog/index.tsx`、`LayoutShapePreview.tsx`      | 缺夹具/入口 | 需要已保存模板、应用/删除按钮和名称输入；没有屏。                  |
| `workspace.quick-connect.default`       | `NewTabLauncher/index.tsx`、`useLauncherData.ts`                | 缺夹具/入口 | 需要 `list_profiles`、`list_ssh_config_hosts` 和最近连接；没有屏。 |
| `workspace.quick-connect.filtered`      | `NewTabLauncher/index.tsx`、`buildLauncherItems.ts`             | 缺夹具/入口 | 需要长 profile 列表和键盘选择；没有屏。                            |
| `workspace.quick-connect.empty`         | `NewTabLauncher/index.tsx`                                      | 缺夹具/入口 | 需要空 profiles/recents 和不存在的查询；没有屏。                   |
| `workspace.quick-connect.clear-confirm` | `NewTabLauncher/index.tsx`                                      | 缺夹具/入口 | 需要最近连接 localStorage 和确认框；没有屏。                       |

### B4 P3 保护与基础浮层

| ID                                 | 生产组件                                       | 状态        | 依据与缺口                                                     |
| ---------------------------------- | ---------------------------------------------- | ----------- | -------------------------------------------------------------- |
| `workspace.close-request.running`  | `CloseRequestDialog.tsx`、`AppActivityContext` | 缺夹具/入口 | 需要 `close-requested` 事件、运行会话/隧道计数和整屏；没有屏。 |
| `workspace.close-request.remember` | `CloseRequestDialog.tsx`                       | 缺夹具/入口 | 需要 `close_behavior`、`save_config` 和记住勾选；没有屏。      |

B4 在批次计划里还包括首启解锁和 Confirm/Prompt。清单中首启解锁落在 P4 设置段，ID 是 `settings.first-start.vault`（`VaultStartupUnlockDialog.tsx`、`useStartupVaultUnlock.ts`）；它同样缺夹具/入口，并且系统钥匙串/主密码部分属于 `[人工]`。`ui.dialog.confirm`、`ui.dialog.prompt` 是已完成的原语屏，不能代表关闭保护或首启解锁的整屏流程。

### 计数

- P3 的 29 个 `workspace.*` 状态：1 个可复现（仅标题栏）、2 个须人工、26 个缺夹具/入口。
- 本批新增的设置屏是 P4 设置页参照，不属于 B1–B4，也不计入上述 29 个状态的完成度。
- manifest 中另有 `tunnels.empty`、`tunnels.list.running` 两个已存在的 P3-09 参照；它们不属于 B1–B4，本批没有改动。

## 检查结果

在仓库根、干净工作树上执行。前端命令全部通过：

| 命令                | 结果                                                                                                                               |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm lint`         | 通过                                                                                                                               |
| `pnpm format:check` | 通过                                                                                                                               |
| `pnpm check:tokens` | 通过                                                                                                                               |
| `pnpm typecheck`    | 通过                                                                                                                               |
| `pnpm test`         | 187 个文件、1367 项 Vitest 通过；`layout.test.mjs`、`reference-screens.test.mjs` 9 项通过；`scripts/migration/*.test.mjs` 8 项通过 |
| `pnpm check:size`   | 1498 个文件在限制内                                                                                                                |

针对性回归：

```sh
pnpm exec vitest run src/parity/screens/settingsReference.test.tsx --reporter=verbose
```

3 个用例通过，见上文“搜索与点击”。

### Rust

`cargo fmt --all --check` 通过。以下失败都在未改动的 Rust 代码上；本批没有修改任何 `.rs`、`Cargo.toml`、`Cargo.lock` 或 `clippy.toml`（`git diff --stat 954e4261 -- '*.rs' Cargo.toml Cargo.lock clippy.toml` 为空）。

`cargo clippy --workspace --all-targets -- -D warnings`：失败，7 个既有 `dead_code` 错误，全部在 `crates/tterm-core/src/ssh/test_sshd.rs`：

- `TempDir` 未构造；
- `TempDir::new` 未使用；
- `absolute_path` 未使用；
- `TestSshd` 未构造；
- `TestSshd::{spawn, spawn_with_host_key, wait_until_ready, connect, sftp_session, key_method}` 未使用；
- `AcceptAnyServerKey` 未构造；
- `current_user` 未使用。

`cargo test --workspace --no-fail-fast`：失败 2 项，其余测试通过。

- `terminal_demo::pipeline::tests::thirty_background_parsers_keep_the_foreground_within_its_frame_budget`：30 个后台解析器把前台 p95 推到 69.70ms，超过 16.7ms 预算；性能/机器负载相关。
- `view::pointer_tests::alt_without_the_macos_policy_reports_in_mouse_mode`：期望鼠标模式报告 `ESC [ < 8 ; 2 ; 1 M`，实际为空列表 `[]`。

按要求没有修复或绕过这些无关 Rust 失败；它们是否阻塞本卡由评估者判断。

## 偏差与遗留

- 两个设置屏共用一份 `settings.reference.json`。它们渲染同一个设置页、读取同一组命令，拆成两份 JSON 会产生重复和漂移；manifest 仍为每个状态单独登记。夹具不含真实连接、账号、主机名、字体列表或密钥。
- 本批只捕获深色、`classic`、100%、`en-US`。新屏在 `referenceScreens`，没有进入 paired 集，也没有改 `parity/exceptions.md`、门槛或 16 个 paired 屏的默认捕获。浅色、孤岛和 125% 的组合尚未生成本批参考。
- `webviewWindow` 的 `setBackgroundColor()` 现在在 parity 模式下是空操作，并补了 `save_window_background` 夹具。真实应用不使用这个桩；该改动只为让生产 `ThemeProvider` 在浏览器里渲染。
- `workspace.titlebar.default` 是现有 Windows 标题栏参照在 macOS WebKit 中的渲染，不能证明 macOS 原生标题栏或交通灯；Windows 原生窗口仍需在 Windows 上用 Edge 通道或真实窗口核对。
- 其余 B1–B4 缺口、macOS 原生标题栏、窗口失焦/模糊、系统钥匙串、原生文件选择器和拖放都是后续批次或人工项。

## 提交

- `17d5a379` `:white_check_mark: Capture the settings page and its search results`（生产标记、parity 入口、夹具、工具桩、回归）
- 报告与资产提交见本分支末尾（`:memo:`）。
