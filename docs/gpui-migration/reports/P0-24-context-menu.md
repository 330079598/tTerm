# P0-24 第二批：右键菜单（macOS）

状态：**完成待评估**。第 3 轮按决策人 stone（2026-11-09）批准，将 E-001 扩展至两屏右键菜单的容器外阴影。先提交例外登记和屏幕引用为 `ac2eaf6f`，确认工作区干净后，在沙箱外用 UPDATE_REFERENCE=1 分别重跑深浅主题，均 **9/9 屏通过**。两份结果均为 `dirty: false`，参照目录不带 `-dirty`。没有修改生产实现、CSS、比较器或门槛数值。

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

Mac mini M4、macOS Apple Silicon，800×600 逻辑像素、DPR 2。本轮对比前先提交 `ac2eaf6f0b4f38dd891c9fa70fac2fba98dd9c9b`，`git status --porcelain` 输出为空。两次均提升权限在沙箱外执行，参照目录分别是：

- 深色：`target/parity/reference/ac2eaf6f0b4f38dd891c9fa70fac2fba98dd9c9b`
- 浅色：`target/parity/reference/ac2eaf6f0b4f38dd891c9fa70fac2fba98dd9c9b-light`

```sh
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-dark.json
# 在浅色运行前，将 result.json 和两张 GPUI 截图一起另存至 target/parity/context-menu-r3-dark/。
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-light.json
# 立即将 result.json 和两张 GPUI 截图一起另存至 target/parity/context-menu-r3-light/。
```

两份原始结果：[深色 result.json](assets/P0-24-context-menu/round3/dark/result.json)、[浅色 result.json](assets/P0-24-context-menu/round3/light/result.json)。每屏的 `exceptions` 字段均存在，使用情况如下；内部图片相对路径仍对应 target 产物。

| 屏幕 | 深色 | 浅色 | 两主题使用的例外 |
| --- | --- | --- | --- |
| example.button-input.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.button.variants | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.button.sizes | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.input.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.password.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.select.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | 无（[]） |
| ui.switch.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | E-001（原登记，outset 3） |
| ui.context-menu.default | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | E-001（本轮扩展，outset 25） |
| ui.context-menu.highlighted | 通过，屏幕颜色 0 px | 通过，屏幕颜色 0 px | E-001（本轮扩展，outset 25） |

| 新增屏幕 | 深色 macOS | 浅色 macOS |
| --- | --- | --- |
| ui.context-menu.default | 屏幕颜色最大 ΔE00 0，超限 0；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999937、整屏 .987345；3/3 图标通过，最大墨量差 .02912、重心 .013px、笔画 ΔE .5572 | 屏幕颜色最大 ΔE00 0，超限 0；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999804、整屏 .985483；3/3 图标通过，最大墨量差 .02454、重心 .01363px、笔画 ΔE 0 |
| ui.context-menu.highlighted | 屏幕颜色最大 ΔE00 0，超限 0；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999937、整屏 .987383；图标同上 | 屏幕颜色最大 ΔE00 0，超限 0；布局 21/21 通过，最大偏差 .97px；遮文字 SSIM .999802、整屏 .985581；图标同上 |

本轮四张完整 GPUI 截图：[深色默认](assets/P0-24-context-menu/round3/dark/default-gpui.png)、[深色高亮](assets/P0-24-context-menu/round3/dark/highlighted-gpui.png)、[浅色默认](assets/P0-24-context-menu/round3/light/default-gpui.png)、[浅色高亮](assets/P0-24-context-menu/round3/light/highlighted-gpui.png)。旧轮 JSON 与截图保留在原 assets/dark、assets/light 目录作为历史证据。

### 已登记例外 E-001

评估者复核第 2 轮像素证据后，决策人 stone 于 2026-11-09 批准把 E-001 扩展至 `ui.context-menu.default`、`ui.context-menu.highlighted` 的 `ui.context-menu`。`outset` 为 25，即 shadow-lg 第一层 y 偏移 10px + CSS 模糊 15px。只排除容器 border box 外的阴影区域，元素内部仍参与比较；原因与人工判定方法沿用 E-001 原文。没有改动 E-001 标题、编号或原有 Switch 段落。

干净复跑后的深色两屏菜单框外均有 **4632 个物理像素差 1 级、4 个差 2 级**（按距圆角 border box 外 >1 至 50 个物理像素的区域统计，避开既有 1px 边缘带）。本轮两主题的参照与旧轮参照像素逐字节一致，深色两张 GPUI 截图也与旧轮逐字节一致；因此旧门槛审计中的 **642 个超限像素、其中 y=1185 一行 474 个**仍可复核，见 [原始门槛像素记录](assets/P0-24-context-menu/dark/gate-pixels.txt)。该行位于菜单下沿外约 2.5 CSS px。应用已批准的扩展后，深浅两屏计入门槛的超限像素均为 **0**。

这属于 E-001 的低透明度模糊阴影尾部取整，不是图标或 ring 的抗锯齿说明。像素样本仍为：(1065,821) WebKit (27,29,34,255)、GPUI (27,28,34,255)；(1300,1185) WebKit (25,27,31,255)、GPUI (25,26,32,255)。本轮没有继续调整 alpha、sigma 或几何来迎合特定浏览器。

下表是本轮对四个主题/屏幕组合逐一进行的视觉核对。使用 PNG 物理像素最近邻 **4 倍放大（400%）**，分别查看菜单下沿左角、直边中段、右角；每个小图均为左参照、右 GPUI。两屏在相同主题下的下沿截图一致，仍分别查看并记录。

| 屏幕 | 主题 | 400% 核对结论 | 并排证据（左参照，右 GPUI） |
| --- | --- | --- | --- |
| ui.context-menu.default | 深色 | 阴影向下延伸，范围与深浅一致；无缺失、偏移或明显偏深偏浅；尾部色阶差符合 E-001。 | [左角](assets/P0-24-context-menu/round3/dark/default-inspect-left-4x.png)、[中段](assets/P0-24-context-menu/round3/dark/default-inspect-middle-4x.png)、[右角](assets/P0-24-context-menu/round3/dark/default-inspect-right-4x.png) |
| ui.context-menu.highlighted | 深色 | 阴影方向、范围和深浅一致；键盘高亮不改变容器阴影，符合 E-001。 | [左角](assets/P0-24-context-menu/round3/dark/highlighted-inspect-left-4x.png)、[中段](assets/P0-24-context-menu/round3/dark/highlighted-inspect-middle-4x.png)、[右角](assets/P0-24-context-menu/round3/dark/highlighted-inspect-right-4x.png) |
| ui.context-menu.default | 浅色 | 阴影方向、范围和深浅一致；圆角附近可见栅格边缘差，未见阴影缺失、错位或明显深浅差。 | [左角](assets/P0-24-context-menu/round3/light/default-inspect-left-4x.png)、[中段](assets/P0-24-context-menu/round3/light/default-inspect-middle-4x.png)、[右角](assets/P0-24-context-menu/round3/light/default-inspect-right-4x.png) |
| ui.context-menu.highlighted | 浅色 | 阴影方向、范围和深浅一致；与默认屏容器阴影相同，符合 E-001。 | [左角](assets/P0-24-context-menu/round3/light/highlighted-inspect-left-4x.png)、[中段](assets/P0-24-context-menu/round3/light/highlighted-inspect-middle-4x.png)、[右角](assets/P0-24-context-menu/round3/light/highlighted-inspect-right-4x.png) |

下沿完整裁剪使用 PNG 坐标 `(1040,1156)..(1600,1200)`，即菜单底边 y=1180 上方 24px 和下方可见的 20px；截图到窗口底部为止。每张从 560×44 放大至 2240×176，未调色或插值平滑。并排图各半保持相同 4 倍比例，原尺寸查看可核对：

| 主题/屏幕 | 参照裁剪 ×4 | GPUI 裁剪 ×4 | 并排（左参照，右 GPUI） |
| --- | --- | --- | --- |
| 深色默认 | [参照](assets/P0-24-context-menu/round3/dark/default-bottom-reference-4x.png) | [GPUI](assets/P0-24-context-menu/round3/dark/default-bottom-gpui-4x.png) | [并排](assets/P0-24-context-menu/round3/dark/default-bottom-pair-4x.png) |
| 深色高亮 | [参照](assets/P0-24-context-menu/round3/dark/highlighted-bottom-reference-4x.png) | [GPUI](assets/P0-24-context-menu/round3/dark/highlighted-bottom-gpui-4x.png) | [并排](assets/P0-24-context-menu/round3/dark/highlighted-bottom-pair-4x.png) |
| 浅色默认 | [参照](assets/P0-24-context-menu/round3/light/default-bottom-reference-4x.png) | [GPUI](assets/P0-24-context-menu/round3/light/default-bottom-gpui-4x.png) | [并排](assets/P0-24-context-menu/round3/light/default-bottom-pair-4x.png) |
| 浅色高亮 | [参照](assets/P0-24-context-menu/round3/light/highlighted-bottom-reference-4x.png) | [GPUI](assets/P0-24-context-menu/round3/light/highlighted-bottom-gpui-4x.png) | [并排](assets/P0-24-context-menu/round3/light/highlighted-bottom-pair-4x.png) |

![深色默认菜单下沿，左参照右 GPUI，均为最近邻 4 倍放大](assets/P0-24-context-menu/round3/dark/default-bottom-pair-4x.png)

![浅色默认菜单下沿，左参照右 GPUI，均为最近邻 4 倍放大](assets/P0-24-context-menu/round3/light/default-bottom-pair-4x.png)

## 全量检查

全部在沙箱外执行，cargo 命令均从本 worktree 根目录运行、CARGO_BUILD_JOBS=3、使用本地 target，无共享 CARGO_TARGET_DIR。

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
CARGO_BUILD_JOBS=3 cargo fmt --all --check
CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=3 cargo test --workspace
```

第 3 轮重新执行以上全部检查，均通过。前端：175 文件、1295 测试；[大小检查](assets/P0-24-context-menu/size.log) 1049 文件通过。[Clippy](assets/P0-24-context-menu/round3/clippy.log) 通过，格式检查通过。[完整 Rust 测试](assets/P0-24-context-menu/round3/workspace-tests.log) 通过，tterm-ui 36、tterm-parity 76、terminal-view 137，Tauri 817 通过。命令没有过滤测试或添加跳过；Tauri 原有 52 个源码标记 ignored 保持原样，没有本轮新增 ignore。开发 bundle/capture 自动关闭，进程核对没有本 worktree 的 Vite、Playwright 或 tterm-parity 残留。未运行正式或 Tauri 开发版、未读写两个受保护应用标识目录。

## 偏差、未验证与 [人工] 项

- 提示词写的是 14px，实测生产为 16px，评估者已确认按 16px。原先停止记录及图标证据保留在 d4b2588d 和 assets 下，本报告更新为继续实施后的结论。
- 深色 shadow-lg 的尾部取整已按 stone 批准登记 E-001，干净工作区复跑及 400% 视觉核对通过；没有待确认事项。
- 当前实现还固定采用这份真实标签菜单的 28px 行与单行标签框；生产允许长标签折行。长文案/极窄窗口下的折行尚未实现等价，不应把本组件视为已覆盖所有语言与调用处。
- Windows 通道：未验证（需 Windows）。
- [人工] 真实 Tauri 窗口与 WebKit 参照的交叉截图；实际鼠标打开标签、终端、SFTP 菜单的端到端使用体验未验证。
- 未合并、未 push；未修改 status.md、共享 P0-24.md、生产 CSS 或依赖版本。
