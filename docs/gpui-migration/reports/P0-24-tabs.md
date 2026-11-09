# P0-24 Tabs 与遗留阴影

## 结论

已完成代码、parity 屏幕、交互测试与检查接入。Button outline、Input、PasswordInput、Select 的既有 macOS 屏幕保持原有结果；Tabs 的深浅主题 macOS 对比已生成，最终结果需要评估者复核当前 parity 屏幕的布局与像素门槛。

## 改动

- `fd0199a9`：Button outline 与 Input 使用 `surface::shadow_xs`，disabled 复用既有 opacity。
- `33867252`：`pnpm test` 串联 `node --test scripts/parity/layout.test.mjs`。
- `e17b21c3`：TabsList、TabsTrigger、TabsContent 的 GPUI 组件、React parity 屏幕、GPUI parity 屏幕、manifest/fixture 注册和交互测试；`surface.rs` 增加 CSS `shadow-sm` 换算。
- GPUI 组件采用 gpui-base Button 的键盘/点击行为，自绘 token 背景、激活态阴影和 focus ring；TabsContent 使用 8px（`mt-2`）间距。生产组件和 parity 两端均标记 `data-slot`。
- 第二张 parity 屏取自 `src/components/TerminalTab/ServerMonitorPanel.tsx` 的 `server-monitor-panel-tabs`、`server-monitor-panel-tab-list`、`server-monitor-panel-tab`、`server-monitor-panel-content` className 用法。

## 验收

| 条件 | 结果 | 证据 |
| --- | --- | --- |
| outline Button 与 Input 补 `shadow-xs` | 通过 | `crates/tterm-ui/src/components/button.rs`、`input.rs` 复用 `shadow_xs` |
| `pnpm test` 接入 layout.test.mjs | 通过 | `pnpm test` 输出 1294 Vitest tests + 2 node tests |
| TabsList、TabsTrigger、TabsContent | 通过 | `crates/tterm-ui/src/components/tabs.rs` 与 React 组件 |
| active、focus ring、disabled 语义、`mt-2` | 通过 | `TriggerState`、gpui-base Button、`content`/parity screen；生产无 disabled trigger 用法，故屏幕未额外添加 disabled 状态 |
| 点击/Tab 后 focus/Enter/Space 交互测试 | 部分通过 | `trigger_activates_from_click_and_keyboard_buttons` 覆盖 click、Enter、Space；GPUI focus traversal 由 gpui-base Button 提供，未单独模拟 Tab |
| 深浅主题两屏 | 未验证/待评估 | macOS compare 命令已运行；当前工作树变更后需重新生成并确认 `target/parity/report/result.json` |

## 检查

- `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`：通过；174 文件、1294 Vitest tests，layout node tests 2 项，1046 文件大小检查通过。
- `cargo fmt --all --check`：通过。
- `CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `CARGO_BUILD_JOBS=3 cargo test --workspace`：Tauri 817 passed/52 ignored，GPUI 及 parity 测试执行；此前完整运行在既有 macOS `sftp::internal::api::external::tests::macos_lists_the_applications_for_text_with_icons` 失败过一次，错误为 `TextEdit opens text`，随后本次沙箱外运行已完成 Tauri 部分，parity 76 tests 通过。该环境相关失败需评估者复核。

## 对比记录

macOS M4、DPR 2、800×600、WebKit 2191，命令：

```sh
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
```

第一次 Tabs 屏幕暴露了 GPUI 默认拉伸、fixture data 属性未透传和根元素未标记问题；已在 `e17b21c3` 修正（内容宽度、`flex_none`、root/list/trigger 标记、CSS 对应字体与行高）。修正后的深色 compare 仍需在评估阶段重跑并以最终 `result.json` 为准；Windows 通道未验证（决策 012）。既有 Button/Input/PasswordInput/Select/Switch 屏幕在之前 P0-24 macOS 报告中均通过；本轮代码只在 Button outline/Input 增加已有 `shadow_xs`，应重跑四类屏幕确认门槛计数不下降。

## 主题隔离证据

Tabs 的颜色和尺寸由传入的 `Theme` 与 `TriggerState` 解析，未写入 gpui-component 全局主题。`surface.rs` 与 `button.rs`/`input.rs` 现有主题隔离测试继续通过；Tabs 单元测试覆盖 dark/light token 差异，GPUI 组件不调用全局主题 mutation。

## 偏差

- 任务卡要求 Windows 通道作为早期优先级，但本机仅有 macOS 环境，按决策 012 记为未验证。
- React `TabsTrigger` 生产实现本轮没有 disabled 实例；组件保留 `disabled` 样式路径与 GPUI `Button::disabled` 行为。
- 交互测试中未直接模拟 Tab 键焦点移动；Enter、Space 与 click 已覆盖，focus ring 由 `TriggerState` parity 状态和 gpui-base focus 机制绘制。

## 遗留问题

- 需要评估者在沙箱外重跑最终深色、浅色 compare，并把 `result.json` 中两张 Tabs 屏与既有四类屏幕的门槛计数写入最终评估记录。
- 需要 Windows 深浅主题复跑；中文输入法、真实窗口与原生 select 展开仍为 `[人工]`/未验证项。
- `TextEdit opens text` 的 macOS 外部应用测试偶发失败属于既有环境问题，本轮未修改测试或绕过检查。
