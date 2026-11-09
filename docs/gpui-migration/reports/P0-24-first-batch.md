# P0-24 第一批：PasswordInput、Select、Switch

## 结论

本轮只完成 Switch 与 Lucide 图标脚本返工。Button/Input 的既有屏幕继续通过；Switch 布局和 SSIM 达标，但屏幕颜色门槛仍需确认。

## 改动

- `switch` 改为基于 `gpui_base::Toggle` 的真实交互控件，Parity 屏逐元素标记四种状态。
- `scripts/generate-lucide-icons.mjs` 从已安装 `lucide-react` 1.48.0 的 `dist/esm/icons/*.mjs` 读取 `__iconData` 生成四个 SVG。

## 做法与理由

PasswordInput 复用 tterm-ui Input 的编辑状态和已通过的 ghost `icon-sm` Button，右侧按 `pr-9` / `pr-16` 预留 Eye 与大写锁定提示位置。Select 使用原生参考端 `<select>`；GPUI 端绘制 36px 收起控件和可展开选项层。Switch 采用自绘 36×20 track、16px `bg-background` thumb 与 3px focus ring，避免修改全局主题。

## 验收

| 条件 | 结果 | 证据 |
| --- | --- | --- |
| PasswordInput 显示/隐藏、Eye/EyeOff、大写锁定图标与内边距 | 部分完成 | `components/password_input.rs`；真实输入和 caps lock 为 `[人工]` |
| Select 收起态 default/focus/disabled/长值截断 | 未验证 | React parity 屏已注册；macOS compare 未完成 |
| Select 展开列表键盘上下、回车、Esc、点外关闭 | 未完成 | 当前 GPUI 层先提供绘制和点击选项，键盘/外点击待后续修订 |
| Switch checked/unchecked/focus/disabled，深浅主题 | 需要确认 | 深色 macOS：布局通过、SSIM 0.998393；屏幕颜色门槛 2039 px 超限。浅色未验证 |
| Lucide 来源、版本与生成脚本 | 通过 | `third-party.md`、`scripts/generate-lucide-icons.mjs` |
| 主题隔离 | 通过（代码路径） | 组件只接收局部 `Theme`；沿用 Button/Input 主题隔离测试模式 |

## 检查

- `cargo fmt --all` 通过。
- `cargo check -p tterm-ui`、`cargo check -p tterm-parity` 通过。
- `pnpm typecheck`、`pnpm format:check` 通过。
- `cargo test -p tterm-parity --lib` 的既有 manifest 断言已更新为 7 屏并通过；完整 workspace 检查待评估前集中运行。

## 未验证与 [人工]

- macOS 深浅主题 parity 截图与 SSIM/颜色门槛：未验证（需要沙箱外 Playwright WebKit 与 GPUI capture）。
- Windows 通道：未验证（需 Windows）。
- 中文输入法、大写锁定真实键盘：`[人工]`。
- Select 键盘上下、回车、Esc、点外关闭：未完成，需后续修订。
