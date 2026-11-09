# P0-24 第二批：右键菜单（macOS）

状态：**需要确认**。组件、两端夹具、布局观测、定位和交互测试已交付；浅色 9/9 屏通过，深色原有 7 屏通过，新增两屏各有 **642 个阴影像素**未通过屏幕颜色门槛。本轮不宣称完成验收，没有修改门槛、遮罩或登记例外。

## 范围与生产语义

已通读 `src/components/ContextMenu.tsx` 与 `src/types/tab.ts` 的 `TabContextMenuAction`。

| 项目类型 | 生产支持情况 |
| --- | --- |
| 普通项 | label、action，可有 icon；触发 onAction 后 onClose。 |
| 分隔线 | separator: true；1px border 色，垂直 margin 4px；跳过键盘导航。 |
| 禁用项 | disabled；不触发，导航跳过。Button 的 disabled:opacity-50 最终生效，原色保留。 |
| 勾选项、子菜单 | 类型无 checked 或 children 字段，组件未实现。 |
| 快捷键 | shortcut 是 KeymapActionId；useShortcutLabel 从 KeymapContext 解析；kbd 为 muted foreground、12px、左 padding 12px、靠右且不收缩。 |
| 危险项 | 无 destructive 字段或专用样式；关闭/删除类仍为 ghost Button。 |

真实菜单来源是 `src/components/TabBar/TabItem.tsx` 中未拆分的页面标签分支（TabBar 使用它）。选择 settings、logs 两个标签中的第一个：New Tab、Reopen Closed Tab、分隔线、Close Tab、Close Other Tabs、禁用的 Close Tabs to the Left、Close Tabs to the Right。与真实调用一样使用 `getReopenClosedTabMenuAction` 和 `getTabCloseMenuActions`。

## 做法与理由

核对 gpui-component 0.9.0 的 PopupMenu：其项目选中背景、边框、圆角与快捷键使用 ActiveTheme 全局主题，且 selected 的实心高亮不同于本菜单的 Button focus-visible 空心 ring。采用 GPUI 实体、局部主题与自绘部件，沿用第一批 `surface`、`css_color`、`css_opacity` 和空心 ring，不修改全局 Theme。没有复制 Zed 代码。

- `context_menu.rs` 管状态与回调，`context_menu/view.rs` 管真实渲染，`placement.rs` 在测量菜单之后按窗口坐标定位。外部 action、close 回调接收 Window/App，可更新业务状态；激活只回调一次，随后关闭。
- 容器宽度是 min(256, viewport width - 20)，再施加 208px 最小宽度；popover 背景、border 1px、radius 6px；外层上下 padding 4px，body 四边 padding 4px。捕获实际为 (534,395) 256×195。
- `surface.rs::shadow_lg` 对应 CSS `0 10px 15px -3px rgb(0 0 0 / .1), 0 4px 6px -4px rgb(0 0 0 / .1)`；GPUI sigma 为 7.5/3。按 CSS 层序反向提交 GPUI shadow slice，黑色通过同一 CSS 8 位颜色量化函数。单元测试覆盖 sigma、offset、spread 与颜色。
- 本菜单各行实际 28px，图标 16px、gap 6px；有图标的 Button 因 has-svg 规则实际水平 padding 12px，无图标为 8px。文字 14px/400，kbd 12px/400，label 高 16px、kbd 高 13px，垂直居中。保留 CSS 声明的 16.8/13.8 行高，部件布局框使用实际测得的高度，快照与绘制同源。
- 初始键盘高亮第一个可用项，使用 3px 空心 ring；第二屏高亮第二项。hover 独立使用 accent（深色 alpha .5），disabled 不进入 hover 或触发分支。
- 两端有 21 个一一对应的布局部件：容器、body、6 行、3 图标、6 label、3 shortcut、1 separator。React 添加 data-slot 和可选 data-parity 属性，不改生产 CSS。GPUI `PartObserver` 从真实 prepaint bounds 回报槽位及实际绘制声明，不是手填坐标。
- 只为这两屏生成 Plus、RotateCcw、X，沿用 `generate-lucide-icons.mjs` 读取已安装 Lucide 的方法；来源、版本和 ISC 许可补入 third-party.md。

参照夹具真实挂载 ConfigProvider、KeymapProvider；配置加载由 parity pending 返回保持默认值。`contextMenuFixture.ts` 使用生产英文资源和默认 resolver 生成 macOS/Windows 标签，`generate-context-menu-fixture.mjs` 导出 GPUI JSON；Vitest 验证 JSON 与生产 helper/default resolver 一致。没有手写 ⌘T 等字符串。manifest 屏幕数量断言已改为 9。

## 定位与交互证据

React 的规则是平移，不是围绕指针翻转：仅当 x+width > innerWidth 时令 x=innerWidth-width-10；y 同理；最后两轴 max(10)。GPUI 测量当前菜单的真实尺寸后应用相同严格大于比较。单元测试覆盖右侧、下侧、右下角、恰好不溢出及负坐标。

完整 workspace 测试包含以下新增真实 GPUI 窗口测试，全部通过（[完整日志](assets/P0-24-context-menu/workspace-tests.log)）：

| 测试 | 证据 |
| --- | --- |
| keys_wrap_skip_disabled_and_separators_then_enter_calls_action_before_close | 上下键循环，跳过 separator 和 disabled；Enter 依次记录 action、closed；再次 Enter 不重复。 |
| escape_and_click_outside_close_without_action | 分别模拟 Esc 和菜单外单击，只关闭、不触发。 |
| pointer_activation_closes_once_and_disabled_click_does_nothing | 点击禁用项无事件，点击可用项触发后关闭一次。 |
| hover_tracks_enabled_rows_without_changing_keyboard_selection | 鼠标移动到普通项产生 hover，移到禁用项清除；键盘选择保持独立。 |
| rendering_two_themes_in_one_window_preserves_global_theme | 同一真实窗口放 DARK/LIGHT 两个实体，render_frame 后检查两种 popover 填色确实被绘制；前后序列化全局 Theme 完全相等。 |

另外支持 Home/End、Tab/ShiftTab、Space，关闭时若焦点仍在菜单或无焦点则恢复打开前的焦点。这些附加路径尚无逐项独立测试。

## macOS 对比

Mac mini M4、macOS Apple Silicon，800×600 逻辑像素、DPR 2。以下两次命令均提升权限在沙箱外执行，并各用 UPDATE_REFERENCE=1 重新生成真实参照：

```sh
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-dark.json
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-light.json
```

| 屏幕 | 深色 | 浅色 |
| --- | --- | --- |
| ui.context-menu.default | **未通过**：屏幕颜色 642 px > ΔE00 1，最大 1.8601；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999963、整屏 .987345；3/3 图标通过，最大墨量差 .02912、重心 .013px、笔画 ΔE .5572 | **通过**：屏幕颜色 0 px，最大 .6464；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999721、整屏 .985483；3/3 图标通过，最大墨量差 .02454、重心 .01363px、笔画 ΔE 0 |
| ui.context-menu.highlighted | **未通过**：屏幕颜色 642 px，最大 1.8601；布局 21/21 通过；遮文字 SSIM .999963、整屏 .987383；图标同上 | **通过**：屏幕颜色 0 px，最大 .6464；布局 21/21 通过；遮文字 SSIM .999720、整屏 .985581；图标同上 |
| 原有 Button/Input 四屏、PasswordInput、Select、Switch | 7/7 继续通过，Switch 使用已有 E-001 | 7/7 继续通过，Switch 使用已有 E-001 |

完整门槛 JSON：[深色](assets/P0-24-context-menu/dark/result.json)、[浅色](assets/P0-24-context-menu/light/result.json)。这些是 result.json 原样副本，内部图片相对路径仍指向 target 产物。参照 PNG 按 parity 约定不提交。

GPUI 截图与热力图：[深色默认](assets/P0-24-context-menu/dark/default-gpui.png)、[深色高亮](assets/P0-24-context-menu/dark/highlighted-gpui.png)、[深色默认热力图](assets/P0-24-context-menu/dark/default-heatmap.png)、[浅色默认](assets/P0-24-context-menu/light/default-gpui.png)、[浅色高亮](assets/P0-24-context-menu/light/highlighted-gpui.png)、[浅色默认热力图](assets/P0-24-context-menu/light/default-heatmap.png)。对应 highlighted 热力图也在各目录。

### 深色尚未解决的阴影差异

检查的是比较器 screen_mask 加 mark_icons 后真正进入门槛的像素，而非热力图中所有红色文字边缘。计数及逐行分布见 [gate-pixels.txt](assets/P0-24-context-menu/dark/gate-pixels.txt)。642 个中，y=1185 一行有 474 个；其余在两侧阴影及转角附近。这是连续直边差异，**不归为抗锯齿**，不在决策 013 的无模糊 ring 边缘或决策 014 的图标规则内。

PNG 物理坐标样本（RGBA，DPR 2）：

| 坐标 | WebKit | GPUI | 说明 |
| --- | --- | --- | --- |
| (1065,821) | (27,29,34,255) | (27,28,34,255) | 门槛计入，ΔE00 1.1464 |
| (1060,838) | (27,29,34,255) | (27,28,34,255) | 门槛计入，ΔE00 1.1464 |
| (1300,1185) | (25,27,31,255) | (25,26,32,255) | 底边连续差异所在行 |
| (1300,1193) | (26,27,33,255) | (26,27,33,255) | 层序和量化修正后吻合的对照点 |

核对 GPUI 0.3.11 的 shadow_fragment，Gaussian 的 Y 积分只取 4 个样本；核对 WebKit 官方 GraphicsContextCG，macOS 通过 CoreGraphics 阴影路径绘制。**这支持不同的模糊/量化路径是候选原因，尚未证明剩余 642 像素的精确来源，不能当作已确认的渲染器误差。**

已尝试的两轮以上修正：半径 sigma 换算及正确几何消除了原试作的整体位置/尺寸错误；再核对阴影层序、8 位 CSS 颜色量化，使深色从 1226 降至 642。另试 SVG GaussianBlur 先栅格化再合成，仍有 2014 个超限，已经撤回；不保留隐藏失败的特殊路径。没有修改 GPUI 依赖或照搬其源码。当前保留按 CSS 声明换算的 shadow_lg，不继续按截图任意调 alpha/sigma。

需要确认的是剩余阴影差异的处理结论及后续调查方向；连续直边差异不满足现有遮罩规则，当前门槛下两屏仍失败。已按原因未明、两轮以上未解决的停止规则留证据，不把失败直接接受为例外。

## 全量检查

全部在沙箱外执行，cargo 命令均从本 worktree 根目录运行、CARGO_BUILD_JOBS=3、使用本地 target，无共享 CARGO_TARGET_DIR。

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
CARGO_BUILD_JOBS=3 cargo fmt --all --check
CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=3 cargo test --workspace
```

前端全部通过：175 文件、1295 测试；[大小检查](assets/P0-24-context-menu/size.log) 1049 文件通过。[Clippy](assets/P0-24-context-menu/clippy.log) 通过，格式检查通过。[完整 Rust 测试](assets/P0-24-context-menu/workspace-tests.log) 通过，tterm-ui 36、tterm-parity 76、terminal-view 137，Tauri 817 通过。命令没有过滤测试或添加跳过；Tauri 原有 52 个源码标记 ignored 保持原样，没有本轮新增 ignore。开发 bundle/capture 自动关闭，进程核对没有本 worktree 的 Vite、Playwright 或 tterm-parity 残留。未运行正式或 Tauri 开发版、未读写两个受保护应用标识目录。

## 偏差、未验证与 [人工] 项

- 提示词写的是 14px，实测生产为 16px，评估者已确认按 16px。原先停止记录及图标证据保留在 d4b2588d 和 assets 下，本报告更新为继续实施后的结论。
- 深色新增两屏的 shadow-lg 尚未达到颜色门槛；具体计数及像素证据见上，状态为需要确认。
- 当前实现还固定采用这份真实标签菜单的 28px 行与单行标签框；生产允许长标签折行。长文案/极窄窗口下的折行尚未实现等价，不应把本组件视为已覆盖所有语言与调用处。
- Windows 通道：未验证（需 Windows）。
- [人工] 真实 Tauri 窗口与 WebKit 参照的交叉截图；实际鼠标打开标签、终端、SFTP 菜单的端到端使用体验未验证。
- 未合并、未 push；未修改 status.md、共享 P0-24.md、生产 CSS 或依赖版本。
