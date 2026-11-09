# P0-24 Dialog（macOS）

## 结论

**需要确认。** 返工后已在沙箱外实际运行深浅两种主题的完整 compare，已有 7 屏均通过；三个 Dialog 屏的布局、样式、图标和 SSIM 均通过，屏幕颜色门槛仍失败，全部剩余超限位于内容框外的 `shadow-lg` 区域。没有改变门槛、扩大遮罩或新增例外。下文保留真实失败数字，不以“未验证”替代本机对比。

## 范围与导出部件

本轮按 `src/components/ui/dialog.tsx` 与 `src/components/ui/app-dialog.tsx` 实现 Dialog。React 导出部件及 className 如下：

| 部件                | `data-slot`          | className                                                                                                                                                                                                                   |
| ------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Dialog`            | —                    | context provider，无 DOM                                                                                                                                                                                                    |
| `DialogTrigger`     | `dialog-trigger`     | 原生 button；`asChild` 时透传到子节点                                                                                                                                                                                       |
| `DialogPortal`      | —                    | portal 到 `document.body`                                                                                                                                                                                                   |
| `DialogOverlay`     | `dialog-overlay`     | `bg-background/80 fixed inset-0 z-50 backdrop-blur-sm`                                                                                                                                                                      |
| `DialogContent`     | `dialog-content`     | `bg-background pointer-events-auto fixed top-[50%] left-[50%] z-[200] grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] grid-cols-1 gap-4 rounded-lg border p-6 shadow-lg outline-none sm:max-w-lg` |
| `DialogClose`       | `dialog-close`       | 默认无 class；关闭按钮在 Content 中为 `absolute top-4 right-4 rounded-xs opacity-70`，focus ring `ring-2 ring-offset-2`                                                                                                     |
| `DialogHeader`      | `dialog-header`      | `flex flex-col gap-2 text-center sm:text-left`                                                                                                                                                                              |
| `DialogFooter`      | `dialog-footer`      | `flex flex-col-reverse gap-2 sm:flex-row sm:justify-end`                                                                                                                                                                    |
| `DialogTitle`       | `dialog-title`       | `text-lg leading-none font-semibold`                                                                                                                                                                                        |
| `DialogDescription` | `dialog-description` | `text-muted-foreground text-sm`                                                                                                                                                                                             |

`app-dialog.tsx` 的 `useConfirmDialog` 使用 Header、Title、Description、Footer 和 outline/default 或 destructive Button；`usePromptDialog` 在 Header 后组合生产 Input，并使用同一 Footer。GPUI 端 Dialog 生产组件复用 tterm-ui Button/Input，部件以同名 slot 构建。

## 做法与修正

沿用第一批的局部主题自绘方式；Input 继续复用 gpui-component 的编辑状态，Button 复用 tterm-ui 生产组件。Dialog 的状态与行为位于 `components/dialog.rs`，绘制位于 `dialog/presentation.rs`，真实事件测试位于 `dialog/tests.rs`。没有修改 gpui-component 全局 Theme、React 生产 CSS、设计令牌或依赖。

本轮修复了前轮实现中的实际问题：

- React 夹具改为调用生产 `useConfirmDialog` / `usePromptDialog`，在 hook 打开 portal 后只添加标记，不修改样式。破坏性确认框包含生产 `AlertTriangle` 图标。
- 宽度按 app-dialog 的 `sm:max-w-md` 为 448px，窗口较小时取 viewport 减 32px；居中由真实 viewport 的 flex 布局计算，删除 800×600 坐标常量。
- 遮罩使用背景 token 的 80% alpha，先量化 sRGB token，再用整数执行 8-bit premultiplication。避免 GPUI 浮点合成时背景多差一个通道等级；两端透明度声明仍为 0.8。
- `surface.rs::shadow_lg` 按 Tailwind v4 原值绘制两层：`0 10px 15px -3px black/10` 与 `0 4px 6px -4px black/10`；GPUI sigma 分别为 7.5px 和 3px。`shadow_lg_converts_both_css_layers_to_sigma` 校验偏移、spread 与 sigma。
- 标题 18px/600/18px；描述 14px/400/16.8px，布局块按 WebKit 当前实际高度 16px 排布，避免 GPUI 向上取整成 17px 后使整框和 X 偏半像素。当前夹具为单行描述；多行描述的排版仍需后续覆盖。
- Footer 水平、右对齐、gap 8px。GPUI 在本机系统字体把 500 匹配为 Regular，导致自动宽度丢失；Dialog 使用 400 与 600 的 advance 中值估算 500 的 intrinsic width，不使用 fixture 文本对应的宽度常量。当前最大布局偏差为 0.25px。
- X 使用脚本生成的 Lucide SVG，实际按钮为 16×16、距内容框 padding-box 顶/右 16px、opacity 0.7；删除原来的 ghost 32px 按钮。破坏性 TriangleAlert 同样来自生成脚本，来源已登记。
- 每个内部 slot 的实际 prepaint bounds 由生产组件 observer 提供；快照的字号、颜色、边框、padding 与绘制共享 `PartStyle`。两端包含 overlay、content、header、title、description、footer、close、cancel、confirm，Prompt 另含 input。

## macOS 深浅对比

环境：Mac mini M4，macOS Apple Silicon，800×600 逻辑像素，DPR 2，WebKit 2191，100% UI scale。使用 GPUI dev bundle 的独立 `com.stone.tTerm.gpui-dev`，未运行 Tauri 正式版或访问真实数据目录。参考端 `reducedMotion=reduce`，截图禁用动画，只比较静止态。

在沙箱外依次执行，cargo 均 `CARGO_BUILD_JOBS=3`，不设置共享 target：

```sh
UPDATE_REFERENCE=1 PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-dark.json
UPDATE_REFERENCE=1 PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
cp target/parity/report/result.json target/parity/report-light.json
```

compare 两次均退出 1。完整原始 JSON、截图和热力图提交在 [assets/P0-24-dialog](assets/P0-24-dialog/)；本地保留 `target/parity/report-dark.json`、`target/parity/report-light.json` 及 `target/dialog-{dark,light}-compare.log`。参考缓存键为 `b649cda56f162934db144528201d5a1faceb51a6[-light]-dirty`，均用 `UPDATE_REFERENCE=1` 重新生成。

| 屏幕                    | 深色 macOS                                                                                                     | 浅色 macOS                                                                                        |
| ----------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `ui.dialog.confirm`     | 失败：布局 9 元素、最大 0.17px、0 failure；颜色门槛 **2288px**；full 0.994687、masked 0.999949；图标 1/1 通过  | 失败：布局最大 0.17px、0 failure；颜色门槛 **2px**；full 0.994139、masked 0.999585；图标 1/1 通过 |
| `ui.dialog.destructive` | 失败：布局 9 元素、最大 0.25px、0 failure；颜色门槛 **2288px**；full 0.994007、masked 0.999946；图标 2/2 通过  | 失败：布局最大 0.25px、0 failure；颜色门槛 **2px**；full 0.993470、masked 0.999543；图标 2/2 通过 |
| `ui.dialog.prompt`      | 失败：布局 10 元素、最大 0.14px、0 failure；颜色门槛 **2496px**；full 0.990410、masked 0.999927；图标 1/1 通过 | 失败：布局最大 0.14px、0 failure；颜色门槛 **2px**；full 0.989855、masked 0.999543；图标 1/1 通过 |
| 已有 7 屏               | **7/7 通过**，每屏 screenColors.pixelsAboveOne=0                                                               | **7/7 通过**，每屏 screenColors.pixelsAboveOne=0                                                  |

已有屏使用原有 E-001 的 Switch 范围，没有将它扩到 Dialog。例外登记正确位置是仓库根目录 [parity/exceptions.md](../../../parity/exceptions.md)。

## 剩余颜色失败与需要确认

先读取 `screenColors.pixelsAboveOne`，再使用比较器的 `text_mask`、`screen_mask`、`icons::mark_icons` 相同调用链列出真正计入门槛的像素。诊断源代码与输出分别保存在 `assets/P0-24-dialog/gate-pixels-source.rs.txt` 和两主题的 `gate-pixels.txt`，未改比较器。

- 深色确认框例：物理坐标 **(349,489)**，React `(26,29,34,255)`，GPUI `(26,28,34,255)`，ΔE00 **1.164110**。直边下方也存在连续 4 个物理像素高的尾部色阶差异，不能称为“单像素圆弧抗锯齿”。内容框原点、尺寸和两层阴影参数一致；overlay 远离阴影处两端均 `(26,29,35)`，排除了遮罩整体颜色错误。残差是 GPUI 圆角阴影解析近似与 WebKit 模糊后量化不同。
- 浅色确认框只有 **(366,747)** 与 **(1233,747)** 两点：React `(234,234,234,255)`，GPUI `(239,239,239,255)`，ΔE00 **1.060197**。它们位于内容框下圆角外侧阴影，未落在现有元素边缘带。
- 深色的成带差异已核对两层 `shadow-lg` 的 offset、negative spread 与 blur/2；没有靠改变 CSS 参数、换更深浅的阴影或主题特判拟合 WebKit。

这些像素不属于决策 013 的无模糊 ring，也不属于决策 014 的图标；E-001 仅授权四个 Switch。需要评估者确认如何处理 Dialog 的模糊阴影差异。在确认前保留失败并停止，不自行新增例外。

截图：[深色 actual](assets/P0-24-dialog/dark/confirm-actual.png)、[深色 reference](assets/P0-24-dialog/dark/confirm-reference.png)、[深色热力图](assets/P0-24-dialog/dark/confirm-heatmap.png)；[浅色 actual](assets/P0-24-dialog/light/confirm-actual.png)、[浅色 reference](assets/P0-24-dialog/light/confirm-reference.png)、[浅色热力图](assets/P0-24-dialog/light/confirm-heatmap.png)。另外两屏的对应三张图也在同目录。

## 交互与主题隔离证据

以下均为 `#[gpui_kit::test]`，通过 `TestAppContext` / `VisualTestContext` 渲染真实 Dialog，再派发按键或鼠标事件；没有以常量断言替代交互。

| 行为                                        | 测试名                                                     |
| ------------------------------------------- | ---------------------------------------------------------- |
| Esc 关闭                                    | `escape_closes_dialog`                                     |
| 点击遮罩关闭                                | `overlay_click_closes_dialog`                              |
| onInteractOutside.preventDefault 阻止关闭   | `outside_hook_can_prevent_dismissal`                       |
| 点击内容不关闭                              | `content_click_keeps_dialog_open`                          |
| X 点击关闭                                  | `close_button_closes_dialog`                               |
| Tab 与 Shift+Tab 环绕、不逃逸               | `tab_and_shift_tab_cycle_focus`                            |
| Prompt 输入、Shift+Tab 到 X、Tab 回到 Input | `prompt_tab_cycle_includes_editable_input`                 |
| 关闭后归还 opener 焦点                      | `closing_restores_the_saved_focus_handle`                  |
| Enter 激活取消/确认/X                       | `keyboard_activates_each_dialog_button`                    |
| 同窗口深浅 Dialog 渲染且全局 Theme 不变     | `rendering_two_dialog_themes_does_not_mutate_global_theme` |

GPUI 打开时从 `window.focused(cx)` 保存真实 handle，Prompt 初始 focus 输入，否则 focus 内容；关闭时恢复。React `restoreFocus` 还包含终端焦点栈/DOM 活性判断，GPUI 当前没有该应用级后备栈，恢复的是仍存在的 handle。React 原始 Dialog 并未实现 Tab trap；本轮按明确要求在 GPUI 的 capture-key 边界实现循环，报告该行为差别。

主题隔离测试前后序列化全局 Theme，在同一窗口放置 DARK/LIGHT 两个真实 Dialog，observer 确认两个 content 均以各自背景主题完成 prepaint，然后断言全局 Theme 完全相同。

## 检查

沙箱外完整前端检查：lint、format:check、check:tokens、typecheck、test、check:size。Vitest **174 文件、1294 测试通过**，无跳过。完整 Rust `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo test --workspace`，cargo 均带 `CARGO_BUILD_JOBS=3`；原始日志在 `target/dialog-clippy-final.log`、`target/dialog-workspace-tests-final.log`。应用枚举 `macos_lists_the_applications_for_text_with_icons` 在沙箱外通过。

没有给 `cargo test --workspace` 传过滤器、skip 或 ignored 参数。仓库本来标记的 52 个需要外部服务/真实设备的 ignored 测试仍由默认 cargo 行为保留；不能宣称它们已执行。本轮没有新增 ignored 测试。最终 tterm-ui 为 **40/40** 通过；完整 Rust 原始输出也附在 `assets/P0-24-dialog/workspace-tests.log`，clippy 输出附在同目录 `clippy.log`。

## 偏差、未验证与 [人工]

- **需要确认：** 上述 `shadow-lg` 颜色门槛仍失败，未满足三屏全部通过，不宣称完成待评估。
- Windows 通道：未验证（需 Windows）。
- `[人工]` 淡入/缩放动画未实现；当前 React Dialog 本身也未提供这两项入场动画 class，静止比较不包含动画。
- `[人工]` 真实键盘中文输入法、系统辅助功能及 Tauri 真窗口交叉截图；自动化交互测试已执行，不再标为未验证。
- 多行描述与较窄窗口的完整视觉矩阵尚未覆盖。当前描述高度按单行 WebKit 的实际 16px 处理，扩展多行需单独校验；本轮不把这点掩饰为通用完整排版。
