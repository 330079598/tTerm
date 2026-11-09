# P0-24 Tabs 与阴影返工（macOS）

> 当前状态：第 3 轮已完成待评估，最终结果见文末“第 3 轮”。下文保留第 2 轮的历史结果与原因。

## 结论与更正

状态：**需要确认**。本轮已经修复阴影引起的 Button、Input、PasswordInput 回归，七张既有屏幕在深浅两种主题下全部通过。Tabs 两屏的元素集合、盒模型、样式和 SSIM 均通过，但颜色门槛仍未通过，尚未满足“九屏全部通过”。此前报告“既有屏幕保持原有结果”与当时深色五屏失败的事实矛盾，现撤回该说法。对比和结果读取均由执行者完成，没有留给评估者代跑。

## 做法与理由

阴影采用评估意见 **(a)**：在 `surface.rs` 增加 `css_backdrop_color`，先按 `css_color` 的 8 位预乘规则量化，再把半透明 fill 合成到不透明主题背景上，阻止 GPUI 盒底的阴影透过 fill。默认暗色背景为 `[37,38,42,255]`，有数值测试。Input 的 disabled fill 将背景 alpha 与 CSS group opacity 一起合成，边框、输入内容和 shadow 分别应用既有 opacity；避免把已合成的不透明 fill 再以半透明度绘制。outline Button 保留原变体和 disabled 色彩，补上的 shadow-xs 不变。

Tabs 使用 gpui-base Button 提供点击、Enter、Space、焦点和 Tab stop，自绘本地主题背景、shadow-sm 和空心 ring。没有改 gpui-component 全局主题。gpui-component 的预设 Tabs 外观依赖其主题与尺寸系统，不能直接表达本项目三部件的 CSS 几何及独立主题；base Button 保留可复用的真实交互。active 的 CSS shadow-sm 为两层 `0 1px 3px 0 / 10%`、`0 1px 2px -1px / 10%`，GPUI sigma 分别为 1.5、1，已有转换测试。

Tabs 根为 w-full，list 为内容宽度、40px 高、4px padding，trigger 为 12px/6px padding，content 跟随 list，默认只加一次 8px margin。两端 root/list/trigger/content ID 一一对应，React 的 data-slot 保留。生产组件解析的 `TabsStyle` 同时供渲染和布局快照使用；夹具没有写死标签宽度。系统字体通过真实 shape_line 测量：macOS font-kit 把 Medium 的 CoreText 0.23 trait 映射到大于 500 的区间，500 按 CSS 字重匹配规则反而选择 Regular，故 GPUI 请求 501 以选中实际 Medium；布局语义仍记录 CSS 500。所有 trigger 宽度由实际标签和 font size 计算。

第二屏来自 `src/components/TerminalTab/ServerMonitorPanel.tsx` 的 className 用法，现已实际导入其 `ServerMonitorPanel.css`。覆盖 header 高 38、list 高 28/padding 2/gap 2/radius 5、trigger 高 24/padding-x 10/font 11/line-height 13.2，以及 content margin 0、padding 14/16。此前只带 className 而未加载 CSS 的夹具不充分，已修正。两端选择状态由 props/owner state 驱动；GPUI 屏幕点击会更新 selected 并刷新 content。

## macOS 对比

Mac mini M4，DPR 2，800×600，100% 缩放，WebKit 2191。均在沙箱外执行，cargo jobs=3，强制重生成参照；没有启动 Tauri 应用，也没有读写正式或 Tauri dev 数据目录。

```sh
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
```

原始 JSON：[深色](assets/P0-24-tabs/rework/dark/report/result.json)、[浅色](assets/P0-24-tabs/rework/light/report/result.json)。本机另存为 `target/parity/report-dark.json` 与 `target/parity/report-light.json`。JSON 原文保留完整失败原因。下表的计数取 `screenColors.pixelsAboveOne`，不是仅遮文字的 `colors.pixelsAboveOne`。

| 屏幕 | 主题 | 结论 | 布局最大偏差 | 门槛 >ΔE00 1 / 最大ΔE00 | 遮文字 SSIM | 整屏 SSIM | 热力图 |
| --- | --- | --- | ---: | --- | ---: | ---: | --- |
| example.button-input.default | 深色 | 通过 | 0.25 px | 0 px / 0.0000 | 1.000000 | 0.998504 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/example.button-input.default.png) |
| ui.button.variants | 深色 | 通过 | 0.00 px | 0 px / 0.7063 | 0.999909 | 0.990686 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.button.variants.png) |
| ui.button.sizes | 深色 | 通过 | 0.00 px | 0 px / 0.0000 | 0.999997 | 0.998855 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.button.sizes.png) |
| ui.input.default | 深色 | 通过 | 0.00 px | 0 px / 0.6928 | 0.999898 | 0.977403 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.input.default.png) |
| ui.password.default | 深色 | 通过 | 0.00 px | 0 px / 0.6928 | 0.999920 | 0.991676 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.password.default.png) |
| ui.select.default | 深色 | 通过 | 0.00 px | 0 px / 0.6928 | 0.999930 | 0.998832 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.select.default.png) |
| ui.switch.default | 深色 | 通过 | 0.00 px | 0 px / 0.0808 | 0.999994 | 0.999996 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.switch.default.png) |
| ui.tabs.default | 深色 | **失败** | 0.33 px | 44 px / 2.7010 | 0.999937 | 0.998166 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.tabs.default.png) |
| ui.tabs.server-monitor | 深色 | **失败** | 0.08 px | 29 px / 2.7010 | 0.999998 | 0.997984 | [图](assets/P0-24-tabs/rework/dark/report/heatmaps/ui.tabs.server-monitor.png) |
| example.button-input.default | 浅色 | 通过 | 0.25 px | 0 px / 0.0000 | 1.000000 | 0.998897 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/example.button-input.default.png) |
| ui.button.variants | 浅色 | 通过 | 0.00 px | 0 px / 0.3994 | 0.999203 | 0.987847 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.button.variants.png) |
| ui.button.sizes | 浅色 | 通过 | 0.00 px | 0 px / 0.0000 | 0.999998 | 0.998873 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.button.sizes.png) |
| ui.input.default | 浅色 | 通过 | 0.00 px | 0 px / 0.3994 | 0.999299 | 0.976297 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.input.default.png) |
| ui.password.default | 浅色 | 通过 | 0.00 px | 0 px / 0.3994 | 0.999295 | 0.990699 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.password.default.png) |
| ui.select.default | 浅色 | 通过 | 0.00 px | 0 px / 0.3994 | 0.999034 | 0.998606 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.select.default.png) |
| ui.switch.default | 浅色 | 通过 | 0.00 px | 0 px / 0.2741 | 0.999984 | 0.999988 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.switch.default.png) |
| ui.tabs.default | 浅色 | **失败** | 0.33 px | 133 px / 4.4614 | 0.999854 | 0.998292 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.tabs.default.png) |
| ui.tabs.server-monitor | 浅色 | **失败** | 0.08 px | 32 px / 4.4614 | 0.999970 | 0.998184 | [图](assets/P0-24-tabs/rework/light/report/heatmaps/ui.tabs.server-monitor.png) |

## 阴影修复前后数字

“前”是在本 worktree 临时恢复本轮开始时 HEAD 的 Button/Input 两文件并重跑 compare 后保存的原始结果；退出后自动恢复修复。Tabs 使用当前布局，故此表只比较七张既有屏幕。两份 shadow-before.json 可直接审计：[深色](assets/P0-24-tabs/rework/dark/shadow-before.json)、[浅色](assets/P0-24-tabs/rework/light/shadow-before.json)。

| 屏幕 | 主题 | 门槛计数 前 → 后 | 遮文字 SSIM 前 → 后 | 整屏 SSIM 前 → 后 |
| --- | --- | ---: | --- | --- |
| example.button-input.default | 深色 | 0 → 0 | 1.000000 → 1.000000 | 0.998504 → 0.998504 |
| ui.button.variants | 深色 | 91652 → 0 | 0.999948 → 0.999909 | 0.990724 → 0.990686 |
| ui.button.sizes | 深色 | 0 → 0 | 0.999997 → 0.999997 | 0.998855 → 0.998855 |
| ui.input.default | 深色 | 37800 → 0 | 0.999903 → 0.999898 | 0.977399 → 0.977403 |
| ui.password.default | 深色 | 33744 → 0 | 0.999922 → 0.999920 | 0.991676 → 0.991676 |
| ui.select.default | 深色 | 0 → 0 | 0.999930 → 0.999930 | 0.998832 → 0.998832 |
| ui.switch.default | 深色 | 0 → 0 | 0.999994 → 0.999994 | 0.999996 → 0.999996 |
| example.button-input.default | 浅色 | 0 → 0 | 1.000000 → 1.000000 | 0.998897 → 0.998897 |
| ui.button.variants | 浅色 | 0 → 0 | 0.999203 → 0.999203 | 0.987847 → 0.987847 |
| ui.button.sizes | 浅色 | 0 → 0 | 0.999998 → 0.999998 | 0.998873 → 0.998873 |
| ui.input.default | 浅色 | 41232 → 0 | 0.991056 → 0.999299 | 0.969297 → 0.976297 |
| ui.password.default | 浅色 | 36672 → 0 | 0.988816 → 0.999295 | 0.981608 → 0.990699 |
| ui.select.default | 浅色 | 0 → 0 | 0.999034 → 0.999034 | 0.998606 → 0.998606 |
| ui.switch.default | 浅色 | 0 → 0 | 0.999984 → 0.999984 | 0.999988 → 0.999988 |

既有七屏恢复到 GPUI 分支的通过结论，新增阴影存在轻微 SSIM 变化；没有声称像素完全相同。暗色 PasswordInput 图标最差墨量偏差从 7.8% 降到 0.9%，三个图标全部通过。

## 交互与主题隔离

`components::tabs::tests::pointer_keyboard_and_tab_focus_use_real_buttons` 真实渲染 gpui-base Root 和生产 list/trigger/content。先点击建立窗口的初始 focus path，再发送真实 Tab key-down/key-up，断言第一个 trigger 聚焦并绘制 ring；再次 Tab 后第二个聚焦、第一个失焦，ring 的 y 坐标随之移动。发送 Enter、Space 和点击后检查 owner selection。没有用 `focus_next` 代替 Tab。

`two_themes_render_together_without_changing_global_theme` 在一个窗口同时放两套主题，实际绘制两种 active fill 并检查 painted_quads；前后序列化全局 Theme 完全相同。测试使用真实绘制结果，而非只比较两个 token 常量。tterm-ui 共 33 项通过，见 [测试输出](assets/P0-24-tabs/rework/checks/ui-test.txt)。

## 尚未通过的像素与需要确认事项

默认 Tabs 深色仍为 44 px、最大 ΔE00 2.7010；server 第二屏为 29 px、最大 2.7010。浅色分别为 133 px/4.4614 和 32 px/4.4614。所有失败原文都在对应 result.json 的 failures 中，唯一屏幕失败类型是 `screen colour gate`，不满足允许值 0。

已先通过工具同一个 `text_mask`、`screen_mask` 和 ΔE00 函数导出真正计入门槛的坐标，再看热力图与圆弧放大图。暗色 [逐像素证据](assets/P0-24-tabs/rework/dark/gate-pixels.txt) 中，例如物理坐标 `(350,74)` 是 `[8,86,152,255]` → `[11,79,137,255]`，ΔE00 2.7010；`(351,74)` 是 `[25,32,42,255]` → `[27,30,37,255]`，ΔE00 2.2442，位于背景色 offset 的内侧曲线。active 阴影尾部 `(78,74)` 是 `[51,56,65,255]` → `[51,55,65,255]`，ΔE00 1.0246；`(245,85)` 是 `[51,56,65,255]` → `[50,54,64,255]`，ΔE00 1.1540。

修正前 256px 的位置错误、重复间距和整片底色错误已消除。残留沿 offset 内沿与 shadow-sm 尾部出现，不属于决策 013 仅覆盖的最外 ring 轮廓；E-001 也仅适用于 Switch，不能直接用于 Tabs。尝试了空心层叠、独立 annuli、共用颜色过渡边缘三种 GPUI quad 绘制模型，几何正确后仍存在内沿合成差异；采用有语义依据的空心 ring 层叠。没有调整颜色追像素、扩大遮罩、修改门槛或登记新例外。此处需要决策人确认后续处理，不能把计入门槛的差异直接算作通过。

## 检查

- `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` 通过。174 测试文件、1294 项 Vitest、2 项 node layout 测试；1046 文件大小检查通过。package.json 的 test 接入保留在此前 `33867252`，未改 AGENTS.md 或 check.yml。
- `CARGO_BUILD_JOBS=3 cargo fmt --all --check` 通过。
- `CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets -- -D warnings` 通过，见 [输出](assets/P0-24-tabs/rework/checks/clippy.txt)。
- `CARGO_BUILD_JOBS=3 cargo test --workspace` 沙箱外通过，未加 exclude/skip，也未新增 ignore。仓库原本声明的 52 项 ignored 保留；1162 项通过、0 失败。macOS applications-with-icons 测试明确通过。见 [完整输出](assets/P0-24-tabs/rework/checks/workspace-test.txt)。
- 没有新依赖、图标、第三方复制代码或生产 CSS 改动。对比脚本自行关闭 Vite 和 Playwright 浏览器；未留下本轮启动的示例窗口。

## 偏差、未验证与 [人工]

- **需要确认**：Tabs 两屏深浅主题颜色门槛未通过，未达到本轮完成标准。不是“完成待评估”。
- Windows 深浅主题：未验证（需 Windows）。
- [人工] Select 原生展开列表、中文输入法、真实键盘 Caps Lock：沿用第一批遗留；本轮未验证。
- [人工] Switch 已登记 E-001 的阴影外观仍需按既有方法核对；未扩大适用范围。
- 未合并、未 push。


## 第 3 轮（2026-11-09）：多层 ring 与 Tabs active 阴影

### 结论与范围

**完成待评估**。决策人 stone 已批准把 ring-offset 的内轮廓纳入决策 013，把 Tabs 两个 active trigger 纳入 E-001。实现、文档、采集和比较测试已经同步；深浅主题均为 **9/9**，每屏 `screenColors.pixelsAboveOne` 都为 **0**。此前“需要确认”的剩余问题在本轮批准的规则范围内解决。

没有改生产 CSS、颜色、像素门槛、SSIM 门槛或轮廓带宽。E-001 仅新增默认屏 `ui.tabs.active` 和第二屏 `ui.tabs.server-active`，outset=4；focus trigger 没有进入例外。

### 工具与计算样式

- React 从 computed box-shadow 采集全部可见、非 inset、x/y/blur=0、spread>0 的层，按升序写入必填 `ringSpreads`。原 `ringSpread` 保留最大值。Tabs focus 为 `[2,4]`，既有 Button/Switch focus 为 `[3]`，无 ring 为 `[]`。
- GPUI 的 `TabsStyle` 现在包含 `ring_offset` 和 `ring_spread`，`ring_spreads()` 同时供 `offset_ring` 绘制与 parity 快照使用；没有在快照另写 2 和 4。Button、Switch 也从自身绘制计算样式导出数组。
- 每层轮廓按各自 spread 外扩、按 CSS Backgrounds 3 计算圆角，只排除轮廓两侧各 1 个物理像素；ring 与 offset 的填充仍参与颜色门槛。
- 布局 schema **4 → 5**；`ringSpreads` 无 serde default，旧缓存缺字段会报错。空 elements 的旧版本也由 `LayoutDocument::load` 拒绝，cache_is_current 因此会重建。manifest/report schema 不变。
- 数组长度、顺序、值均进入样式检查，0.01px 容差；失败信息含元素名。决策 013 的原文保留，在末尾补充 ring-offset 规则，模糊及 x/y 非零阴影仍不由该规则处理。

### 反向探测与自动测试

[完整 Rust 输出](assets/P0-24-tabs/round3/checks/workspace-test.txt)、[node 采集测试](assets/P0-24-tabs/round3/checks/layout-test.txt)。

| 测试/探测 | 实际验证 | 结果 |
| --- | --- | --- |
| layout.test.mjs 两层 shadow | CSS 顺序 `[4,2]` 采为 `[2,4]`，ringSpread=4；透明、inset、blur、x/y 非零与负 spread 不纳入 | 通过 |
| both_offset_ring_contours_are_edges_but_their_fills_are_not | DPR2 下两条轮廓的两侧及圆角被排除；ring 中间、offset 中间和带外像素仍未遮罩 | 通过 |
| multilayer_ring_control_passes_without_changing_thresholds | 两端完全相同的多层圆角 ring 交给完整 compare_screen | 通过，门槛计数 0 |
| moving_the_painted_ring_one_logical_pixel_fails_the_screen_gate | 只把 GPUI 图像中的 ring 整体向右挪 1 逻辑像素，不修改快照或遮罩 | **按预期失败**，screen colour gate >0 |
| changing_the_offset_fill_fails_the_screen_gate | 只把 GPUI offset 填充换红色，spread 与快照保持原值 | **按预期失败**，screen colour gate >0 |
| a_missing_offset_contour_fails_and_names_the_element | 数组删内层、改内层、交换顺序 | **按预期失败**，信息含 `tabs.focus: ring spreads` |
| ring_spreads_are_required / old_schema_is_rejected | 缺数组以及 schema4 空布局 | **按预期拒绝**，没有静默复用缓存 |

### 干净工作区对比

先提交，再在沙箱外运行下列命令；运行时 `git status --short` 为空。代码提交为 `45a95d8d`、`45765904`，样式比较函数拆分为 `52ed0eab`。最终两种主题参照键均为 `45765904d4939bcef8e13c0f2058d76ad5a73e4b`（前端/parity 清单最后一次变更提交），`dirty:false`；深色目录无后缀，浅色后缀 `-light`，均**没有 `-dirty`**。实际 GPUI 为拆分后的最终代码。

```sh
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
UPDATE_REFERENCE=1 PARITY_PLATFORM=macos PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
```

Mac mini M4，800×600，DPR2，WebKit2191。两次退出码0：[深色 JSON](assets/P0-24-tabs/round3/dark/report/result.json)、[浅色 JSON](assets/P0-24-tabs/round3/light/report/result.json)。原样另存 `target/parity/report-dark.json`、`target/parity/report-light.json`。附件包含 actual PNG、两端布局、热力图、HTML 和 compare 日志；参考 PNG 依仓库规则保留在 target 缓存。

| 屏幕 | 主题 | 结论 | 布局最大偏差 | 门槛计数 第2轮→第3轮 / 当前最大ΔE00 | 遮文字 SSIM | 整屏 SSIM |
| --- | --- | --- | ---: | --- | ---: | ---: |
| example.button-input.default | 深色 | 通过 | 0.25px | 0 → 0 / 0.0000 | 1.000000 | 0.998504 |
| ui.button.variants | 深色 | 通过 | 0.00px | 0 → 0 / 0.7063 | 0.999909 | 0.990686 |
| ui.button.sizes | 深色 | 通过 | 0.00px | 0 → 0 / 0.0000 | 0.999997 | 0.998855 |
| ui.input.default | 深色 | 通过 | 0.00px | 0 → 0 / 0.6928 | 0.999898 | 0.977403 |
| ui.password.default | 深色 | 通过 | 0.00px | 0 → 0 / 0.6928 | 0.999920 | 0.991676 |
| ui.select.default | 深色 | 通过 | 0.00px | 0 → 0 / 0.6928 | 0.999930 | 0.998832 |
| ui.switch.default | 深色 | 通过 | 0.00px | 0 → 0 / 0.0808 | 0.999994 | 0.999996 |
| ui.tabs.default | 深色 | 通过 | 0.33px | 44 → 0 / 0.0000 | 0.999993 | 0.998166 |
| ui.tabs.server-monitor | 深色 | 通过 | 0.08px | 29 → 0 / 0.0000 | 0.999999 | 0.997984 |
| example.button-input.default | 浅色 | 通过 | 0.25px | 0 → 0 / 0.0000 | 1.000000 | 0.998897 |
| ui.button.variants | 浅色 | 通过 | 0.00px | 0 → 0 / 0.3994 | 0.999203 | 0.987847 |
| ui.button.sizes | 浅色 | 通过 | 0.00px | 0 → 0 / 0.0000 | 0.999998 | 0.998873 |
| ui.input.default | 浅色 | 通过 | 0.00px | 0 → 0 / 0.3994 | 0.999299 | 0.976297 |
| ui.password.default | 浅色 | 通过 | 0.00px | 0 → 0 / 0.3994 | 0.999295 | 0.990699 |
| ui.select.default | 浅色 | 通过 | 0.00px | 0 → 0 / 0.3994 | 0.999034 | 0.998606 |
| ui.switch.default | 浅色 | 通过 | 0.00px | 0 → 0 / 0.2741 | 0.999984 | 0.999988 |
| ui.tabs.default | 浅色 | 通过 | 0.33px | 133 → 0 / 0.0000 | 0.999984 | 0.998292 |
| ui.tabs.server-monitor | 浅色 | 通过 | 0.08px | 32 → 0 / 0.4925 | 0.999992 | 0.998184 |

七张既有屏幕门槛计数均保持 0；没有变差。两个 Tabs 屏幕深色从 44/29 降至 0/0，浅色从 133/32 降至 0/0。

### 阴影独立统计与人工核对表

用工具原有的 text_mask、screen_mask 和 ΔE00 函数对真实截图统计，先只应用新的两层轮廓，暂不应用 E-001。这样把 ring 曲线和 shadow-sm 尾部分开：[原始坐标与像素值](assets/P0-24-tabs/round3/checks/shadow-probe.txt)。

| active trigger | 主题 | 仅轮廓规则后的超限像素 / 最大ΔE00 | 应用 E-001 后 | 400% 以上核对 |
| --- | --- | --- | --- | --- |
| ui.tabs.active | 深色 | 18 / 1.224873 | 0 | [放大图](assets/P0-24-tabs/round3/visual/dark-ui.tabs.default-shadow-400.png)：方向向下、范围相近，未见缺失或偏移，深浅接近，通过 |
| ui.tabs.server-active | 深色 | 13 / 1.224873 | 0 | [放大图](assets/P0-24-tabs/round3/visual/dark-ui.tabs.server-monitor-shadow-400.png)：方向向下、范围相近，未见缺失或偏移，深浅接近，通过 |
| ui.tabs.active | 浅色 | 99 / 1.912586 | 0 | [放大图](assets/P0-24-tabs/round3/visual/light-ui.tabs.default-shadow-400.png)：向下浅灰，GPUI 尾部略柔和，范围与深浅接近，未见缺失或偏移，通过 |
| ui.tabs.server-active | 浅色 | 12 / 1.822615 | 0 | [放大图](assets/P0-24-tabs/round3/visual/light-ui.tabs.server-monitor-shadow-400.png)：向下浅灰，尾部略柔和，未见缺失、偏移或明显深浅错误，通过 |

放大图把物理像素放大 4 倍，在 DPR2 下相当于逻辑尺寸 8 倍；上下分别为参照与 GPUI，执行者已逐图查看。实际浅色阴影尾部最大差高于此前深色的 1.0–1.2，已如实记录在 E-001 扩展中。最终 ring 区域无超限像素，E-001 区域只覆盖 active trigger，未把 focus ring 混进阴影例外。

### 完整检查、偏差与未验证项

所有 AGENTS.md 检查均在沙箱外执行。前端六项通过：174 个文件、1294 项 Vitest、2 项 node 测试，1046 文件大小达标；[完整日志](assets/P0-24-tabs/round3/checks/frontend.txt)。fmt 通过；clippy workspace/all-targets/-D warnings 通过；完整 `cargo test --workspace` **1169 passed、0 failed、52 项仓库原有 ignored**，未用 skip/exclude，macOS applications-with-icons 通过。[clippy](assets/P0-24-tabs/round3/checks/clippy.txt)、[workspace](assets/P0-24-tabs/round3/checks/workspace-test.txt)。

首次 clippy 指出 compare_style 为153行，已单独拆出 compare_ring_spreads 后全量复跑通过，没有放宽限制。无其他实现偏差、新依赖、生产 CSS 变化或新增例外编号。未修改 AGENTS.md、check.yml、status.md 或共用 P0-24.md。

- Windows 深浅主题：未验证（需 Windows）。
- [人工] Select 展开列表、中文输入法、真实键盘 Caps Lock：沿用第一批遗留，本轮未验证。
- [人工] 本轮新增 active trigger 阴影已按上表完成截图核对；Switch 既有 E-001 人工核对仍沿用第一批要求。
- 未合并、未 push。临时统计源码已删除；对比脚本关闭浏览器和 Vite，没有本轮启动的窗口或后台服务遗留。
