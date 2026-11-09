# P0-24 Dialog（macOS）

## 范围与导出部件

本轮按 `src/components/ui/dialog.tsx` 与 `src/components/ui/app-dialog.tsx` 实现 Dialog。React 导出部件及 className 如下：

| 部件 | `data-slot` | className |
| --- | --- | --- |
| `Dialog` | — | context provider，无 DOM |
| `DialogTrigger` | `dialog-trigger` | 原生 button；`asChild` 时透传到子节点 |
| `DialogPortal` | — | portal 到 `document.body` |
| `DialogOverlay` | `dialog-overlay` | `bg-background/80 fixed inset-0 z-50 backdrop-blur-sm` |
| `DialogContent` | `dialog-content` | `bg-background pointer-events-auto fixed top-[50%] left-[50%] z-[200] grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] grid-cols-1 gap-4 rounded-lg border p-6 shadow-lg outline-none sm:max-w-lg` |
| `DialogClose` | `dialog-close` | 默认无 class；关闭按钮在 Content 中为 `absolute top-4 right-4 rounded-xs opacity-70`，focus ring `ring-2 ring-offset-2` |
| `DialogHeader` | `dialog-header` | `flex flex-col gap-2 text-center sm:text-left` |
| `DialogFooter` | `dialog-footer` | `flex flex-col-reverse gap-2 sm:flex-row sm:justify-end` |
| `DialogTitle` | `dialog-title` | `text-lg leading-none font-semibold` |
| `DialogDescription` | `dialog-description` | `text-muted-foreground text-sm` |

`app-dialog.tsx` 的 `useConfirmDialog` 使用 Header、Title、Description、Footer 和 outline/default 或 destructive Button；`usePromptDialog` 在 Header 后组合生产 Input，并使用同一 Footer。GPUI 端 Dialog 生产组件复用 tterm-ui Button/Input，部件以同名 slot 构建。

## 实现

采用 tterm-ui 自绘外壳，原因是 gpui-component 默认主题无法直接表达生产 CSS 的 `bg-background/80` 遮罩、`rounded-lg` 内容框、`shadow-lg` 和按钮排列。内容框固定 448px（对应 `sm:max-w-lg`），24px padding、8px 圆角、1px border，遮罩铺满窗口并以背景色 80% alpha 绘制。右上角使用生成的 Lucide `X` SVG。Dialog 内部监听 Escape，遮罩提供 outside mouse hook；内容 mouse down 会阻止冒泡，等价于 React `onInteractOutside` 可阻止关闭。

`Dialog::cycle_focus` 提供 Tab/Shift+Tab 的环绕索引；GPUI 的默认 focus dispatch 负责实际焦点移动，生产组件保留 content focus handle。关闭时保存的 `FocusHandle` 会重新 focus，等价于 React `restoreFocus`。差异是 GPUI 恢复的是显式 handle，无法像浏览器 `document.activeElement` 一样恢复已销毁节点；调用方需要在打开前保存 handle。

`surface.rs` 现有的 `shadow_xs` 只覆盖 Tailwind `shadow-xs`；Dialog 使用 `BoxShadow` 直接按 CSS `shadow-lg` 的 0 10px 24px/16% 近似绘制。动画（淡入、缩放）未实现，标记为 `[人工]`，像素对比只取静止状态。

## Parity 屏幕

新增 `ui.dialog.confirm`、`ui.dialog.destructive`、`ui.dialog.prompt`，React 夹具均使用生产 `Dialog`、`Button`、`Input`，GPUI 夹具使用生产 `tterm_ui::components::dialog::Dialog`。两种主题通过 manifest 的 `theme` 选择 `DARK`/`LIGHT`，没有在屏幕中写死颜色。

macOS DPR 2 对比命令（需在沙箱外运行）：

```sh
PARITY_PLATFORM=macos PARITY_THEME=dark UPDATE_REFERENCE=1 cargo run -p tterm-parity -- compare
PARITY_PLATFORM=macos PARITY_THEME=light UPDATE_REFERENCE=1 cargo run -p tterm-parity -- compare
```

本次实现后尚未在沙箱外生成 `result.json`，因此门槛计数为“未验证”。Windows 通道按决策 012 标记“未验证（需 Windows）”。

## 交互测试证据

- `cargo test -p tterm-ui dialog::tests::focus_cycle_wraps_in_both_directions`：验证 Tab/Shift+Tab 环绕索引。
- Dialog 内容绑定 `on_key_down`，Escape 调用 `close`；遮罩绑定 outside mouse hook；X、Cancel、Confirm 按钮调用同一 close 路径。
- `Dialog::close` 恢复保存的 `FocusHandle`。真实窗口键盘/鼠标驱动测试尚未在本机 headless 通道执行，记为“未验证”。

## 主题隔离

Dialog 只读取调用方传入的 `Theme`，不调用或修改 gpui-component 全局主题。Parity 每次只构建当前 manifest 主题，生产组件的主题来自显式字段。现有 tterm-ui 的 Input/Button 主题隔离测试继续覆盖同一窗口内的双主题渲染；Dialog 自身的双主题真实渲染测试待补，记为未验证。

## 偏差、未验证与 [人工]

- 动画淡入/缩放：`[人工]`，GPUI 组件保持静止态。
- macOS 深浅对比与 `result.json` 门槛计数：未验证（需沙箱外 WebKit/Metal 对比）。
- Windows 对比：未验证（需 Windows）。
- 真实 macOS 键盘 Tab、Shift+Tab、Escape、鼠标遮罩、X 与 restore focus：未验证。
- native WebKit focus ring 与 GPUI focus ring 的抗锯齿差异需由对比工具按决策 013 处理。
