# P4-15 设置外壳与回滚行数流程（决策 018 提前批）

- 日期：2026-11-23
- 分支 / worktree：`migrate/settings-scrollback-flow`
- 平台：macOS 27.2（Build 26B123），arm64
- 状态：**实现完成，等待主控验收**。只提前实现决策 018 要求的一条设置流程；不把本报告当作 P4-15、G2 或 G3 通过。

## 结论

生产 GPUI 设置路由现在有与 React 原版同条件的页面壳、侧栏、搜索框、搜索状态和回滚行数字段。搜索复用 `tterm-core` 的 `settings::search` 与 `settings::search_index`，只列出本客户端真正实现的设置：

- 侧栏只有 **Terminal** 页；General、Groups、Connection 等页面尚未实现，因此不摆放空入口。
- 搜索索引只有页本身和 `fontSettings.scrollbackLines`。查询 `scrollback` 得到 1 条，查询 `回滚`（中文）得到 1 条；查询 `font` 或 `zzz` 得到空结果，不用硬编码命中补数。
- 搜索框支持真实输入、清除按钮、Escape 先清除再返回、方向键选择、Enter 选择、鼠标选择。
- 回滚行数支持草稿、预设、校验、显式保存和失败提示；不会自动保存无效输入。
- 两条真实隔离 PTY 在 0.35/0.65 的非均分分屏中运行。打开设置、搜索、选择、修改、保存后，运行中的容量立即改变；关闭后原窗格焦点、分屏比例和后台输出保持不变；重新打开读到保存值。
- 800×600 设置内容区的页面壳尺寸、搜索框、侧栏行、内容区、搜索结果行、回滚字段尺寸和颜色 token 与原版 **局部一致**；整页仍有意缺失未实现页面，拉丁字母标题宽度也因字体回退不同而不一致，见“差异”。

## 范围

本卡只覆盖决策 018 的第 2 步：从运行中的终端打开设置，找到回滚行数，修改并保存，再返回。没有实现以下内容，也没有为它们摆放按钮：

- General、连接、代理、外观、Shell、高亮、通知、快捷键、日志、数据迁移、安全、更新等页面。
- Font Family、Font Size、界面缩放、窗口模糊、主题等回滚行数以外的设置。
- 设置页的懒加载、跨客户端配置互通、完整保存队列（除回滚行数已有的保存路径）。
- 浅色主题、`islands` 样式和 125% 缩放。
- 原生窗口模糊、系统钥匙串、原生文件选择器和 Windows 原生标题栏。

## 实现

| 内容 | 位置 |
| --- | --- |
| 设置路由和搜索/导航状态 | `crates/tterm-gpui/src/terminal_demo/window/settings_page/mod.rs` |
| 页面壳、侧栏、搜索结果和空结果 | `crates/tterm-gpui/src/terminal_demo/window/settings_page/shell.rs` |
| 可搜索设置与共享索引的筛选 | `crates/tterm-gpui/src/terminal_demo/window/settings_page/search.rs` |
| 回滚行数字段、数字校验和预设 | `crates/tterm-gpui/src/scrollback_setting.rs` |
| 打开/关闭路由、Escape 优先级、标题栏入口 | `crates/tterm-gpui/src/terminal_demo/window/settings_page/mod.rs`、`window/strip.rs`、`window.rs` |
| 设置页翻译 key | `crates/tterm-core/src/i18n/keys.rs` |
| 齿轮、页签、搜索和清除图标 | `crates/tterm-ui/assets/icons/`、`components/icons.rs` |
| 双 PTY、搜索、保存和失败回归 | `crates/tterm-gpui/src/terminal_demo/window/settings_page_tests.rs` |
| 生产窗口截图脚本 | `scripts/migration/capture-settings-gpui.py` |
| 原版参照屏幕 | `parity/screens.json`、`scripts/parity/capture.mjs`、`src/parity/screens.ts`、`src/parity/fixtures.ts`、`SettingsDialog/ScrollbackLinesField.tsx` |

搜索通过 `settings::search_index::entries_for_page("terminal")` 读取共享索引，并在 `search.rs` 的单元测试中断言 `fontSettings.scrollbackLines` 仍属于该页。页面名、设置名和描述都由现有 `en.json` / `zh.json` 翻译；没有另造配置存储。

## 复现条件

| 项 | 值 |
| --- | --- |
| 机器 | macOS 27.2（Build 26B123），arm64 |
| Node / pnpm | v24.20.0 / 11.25.0 |
| Rust | rustc 1.99.1 (58cf8f9c3 2026-10-13) |
| Playwright | 1.54.0，WebKit revision 2191 |
| 原版视口 | 800×600 CSS，DPR 2，PNG 1600×1200 |
| GPUI 窗口 | 800×636 逻辑像素；36pt 标题栏；裁掉标题栏后内容区 800×600，PNG 1600×1200 |
| 主题 / 风格 / 缩放 | `dark` / `classic` / 100% |
| locale | `en-US`；中文屏为 `zh-CN` |
| 原版参照提交 | `1e2d8c4e`，前端工作树干净 |

原版在仓库根执行：

```sh
node scripts/parity/capture.mjs \
  --screen settings.search.scrollback \
  --screen settings.search.scrollback.open \
  --screen settings.terminal.scrollback \
  --screen settings.search.scrollback-zh \
  --screen settings.search.scrollback.open-zh \
  --out target/parity/settings-ref-final
```

GPUI 生产窗口在仓库根执行：

```sh
cargo build --workspace --bin tterm-gpui --features tterm-gpui/automation
python3 scripts/migration/capture-settings-gpui.py \
  --output target/parity/settings-gpui-postcommit
```

脚本做三件事：

1. 用 `TTERM_GPUI_CHECK_INSTANCE` 和 `TMPDIR` 在输出目录下建立隔离实例，第一次启动只创建数据库。
2. 把 `gpui_window_state` 写成 800×636，再启动正式 `Prototype` 窗口；不是截图专用视图。
3. 通过 loopback automation 用真实按键和文本输入打开设置、输入 `scrollback` / `回滚`、选择结果，再截图；每张图按标题栏 36pt 的边界裁到设置内容区。

布局数据：

```sh
TTERM_GPUI_WRITE_SETTINGS_LAYOUT=1 cargo test -p tterm-gpui --lib \
  terminal_demo::window::settings_page_tests::page_layout_matches_the_reference_metrics \
  -- --exact --nocapture
```

它写入 `target/parity/settings-gpui/layout.json`、`layout-search.json` 和 `layout-empty.json`。测试用同一个生产 `Prototype` 渲染树，窗口 800×636，坐标统一减去 36pt 标题栏。

## 证据

全部资产在：

- [`reports/assets/settings-scrollback-flow/reference/`](assets/settings-scrollback-flow/reference/)
- [`reports/assets/settings-scrollback-flow/gpui/`](assets/settings-scrollback-flow/gpui/)
- [`reports/assets/settings-scrollback-flow/crops/`](assets/settings-scrollback-flow/crops/)

### 原版参照

五屏都来自真实 `SettingsPanel`，不是另画的示意图：

| 屏幕 | 动作 | 文件 |
| --- | --- | --- |
| `settings.search.scrollback` | 在真实搜索框键入 `scrollback` | `reference/settings.search.scrollback.png` + `.layout.json` |
| `settings.search.scrollback.open` | 键入 `scrollback`，点击结果 0，等待 2.2s 的定位框消失并滚到字段 | `reference/settings.search.scrollback.open.png` + `.layout.json` |
| `settings.terminal.scrollback` | 点击侧栏 Terminal，滚到回滚行数 | `reference/settings.terminal.scrollback.png` + `.layout.json` |
| `settings.search.scrollback-zh` | `zh-CN` 下输入 `回滚`，等待 1s 让 SVG 图标稳定 | `reference/settings.search.scrollback-zh.png` + `.layout.json` |
| `settings.search.scrollback.open-zh` | `zh-CN` 下输入 `回滚`，点击结果 0，等待定位框消失并滚到字段 | `reference/settings.search.scrollback.open-zh.png` + `.layout.json` |

原版 manifest：[`reference/reference-screens.json`](assets/settings-scrollback-flow/reference/reference-screens.json)，`commit` 为 `1e2d8c4e`，每条中文屏记录 `locale: "zh-CN"`。两次干净捕获的 PNG 和 layout JSON 的 SHA-256 完全一致。

W0-05 的 `settings.layout.default` 和 `settings.search.results` 仍只代表 General 页和 `font` 搜索；本卡没有用它们冒充 Terminal 页或回滚查询的参照。本卡新增的五屏才是回滚流程的原版对照。

### GPUI 生产视图

| 屏幕 | 状态 | 文件 |
| --- | --- | --- |
| `settings.search.scrollback` | 搜索框聚焦，查询 `scrollback`，结果 1 条 | `gpui/settings.search.scrollback.png` |
| `settings.terminal.scrollback` | 选择结果后回到 Terminal 页，搜索框保留焦点 | `gpui/settings.terminal.scrollback.png` |
| `settings.search.empty` | 查询 `zzz`，无匹配提示 | `gpui/settings.search.empty.png` |
| `settings.search.scrollback-zh` | `zh-CN`，查询 `回滚` | `gpui/settings.search.scrollback-zh.png` |
| `settings.terminal.scrollback-zh` | 中文目标页 | `gpui/settings.terminal.scrollback-zh.png` |

布局数据：`gpui/layout.json`（默认页）、`gpui/layout-search.json`（搜索结果）、`gpui/layout-empty.json`（无匹配）；捕获清单：`gpui/capture.json`。

### 稳定哈希

原版：

| 文件 | SHA-256 |
| --- | --- |
| `settings.search.scrollback.png` | `52f04b90b779701b110821024d51a00987ef35bf223f55cb1c5f65bb6090d82e` |
| `settings.search.scrollback.open.png` | `f5f59b99702b02795c4ce8f7fda275427a91149e21a9f3f95fba575bfdb132e6` |
| `settings.terminal.scrollback.png` | `182e55016a2af3fe05f9d563ed70c6115aaff37f22f0843c5b601559be3e71db` |
| `settings.search.scrollback-zh.png` | `4e60580646a6b338e9447fec7c42b2d963849ba46d9e2ab6a35a93901a0ece39` |
| `settings.search.scrollback.open-zh.png` | `24d0a2e473a2f77126907d6617f7ebc454f16f465203c518dbe2d5eeb431ef8f` |

GPUI：

| 文件 | SHA-256 |
| --- | --- |
| `settings.search.scrollback.png` | `385db61d3de4896c515a82b81fe975a25b47b4f428c3bca284bf8b4f93d49113` |
| `settings.terminal.scrollback.png` | `0f06c1877b5b9e26e9eef8917f07833fa3988de2369e851ac40f34b91eb12665` |
| `settings.search.empty.png` | `d7e8a1a526d510027ec44224e4cede76fbb192913e1fbfa793a18dfcdb0ea7ce` |
| `settings.search.scrollback-zh.png` | `1309e3c8548e222a27d2242bb5e9f71d65925e733d4c435d46ce5657a7cd2c12` |
| `settings.terminal.scrollback-zh.png` | `ad4c89c976cf2d6fc44ed3c55787e36880a1f877b1da7755ed197baef6dc514e` |

GPUI 截图在 `settings-gpui-r11` 与 `settings-gpui-r12` 两次独立运行中 PNG 字节相同；提交 `19c8820c` 后再次运行，像素也相同；焦点重绘提交 `c4e3ca8b` 后再次运行，五张截图仍逐像素相同。脚本会等搜索框失去插入符后再截图，和原版 Playwright 的 `caret: "hide"` 对齐。

## 双端比较

以下坐标均为设置内容区 CSS 像素（去掉 GPUI 标题栏后）。原版数据取自 layout JSON；GPUI 数据取自生产渲染树的 debug bounds。

| 部位 | 原版 | GPUI | 结论 |
| --- | --- | --- | --- |
| 页面 | `0,0 800×600` | `0,0 800×600` | 一致 |
| 标题区 | `0,0 800×73`，padding `24/48/16/24` | 相同 | 一致 |
| 标题位置 / 字号 | `24,31`，18px / 600 | `24,31`，18px / 600 | 位置和字号一致 |
| 标题元素宽度 | `96.33`（图标+文字） | `113` | **不一致**：GPUI 拉丁字体回退更宽 |
| 搜索框 | `496,24 256×32`，圆角 6，左 padding 32 | 相同 | 一致 |
| 侧栏 | `0,73 192×527`，`muted/30`，padding 12 | 相同 | 一致 |
| 侧栏行 | 宽 `159`、高 `32`、圆角 6、padding-x 8 | `12,103 159×32` | 行尺寸一致；原版 Terminal 行在 y=271，因为上面还有两个分组 |
| 选中状态 | `background` 填充、`foreground` 文字 | 相同 | 一致 |
| 内容区 | `192,73 608×527`，padding 24 | 相同 | 一致 |
| 搜索结果行 | `216,118 560×40`，padding `10/12`，border 1，圆角 6 | 相同 | 一致 |
| 搜索结果选中 | `ring` 边框 + `primary/10` 背景 | 相同 token | 一致 |
| 回滚字段 | `536×111` | `216,97 536×111` | 尺寸一致；原版因滚动在 y=281，GPUI 只有该字段所以在页首 |
| 回滚输入框 | `536×36` | `216,119 536×36` | 一致 |
| 预设按钮 | 高 24，圆角 6，间距 6 | 相同 | 一致 |
| 颜色 | `background #1b1d23`、`foreground #cccccc`、`border #3d3d3d`、`ring #0070cc` | 同一组 token | 一致 |

局部对比图：

- 标题和搜索框：[`crops/reference-header-search-en.png`](assets/settings-scrollback-flow/crops/reference-header-search-en.png) / [`crops/gpui-header-search-en.png`](assets/settings-scrollback-flow/crops/gpui-header-search-en.png)
- 搜索结果行：[`crops/reference-search-result-en.png`](assets/settings-scrollback-flow/crops/reference-search-result-en.png) / [`crops/gpui-search-result-en.png`](assets/settings-scrollback-flow/crops/gpui-search-result-en.png)，差异遮罩 [`crops/diff-search-result-en.png`](assets/settings-scrollback-flow/crops/diff-search-result-en.png)
- 回滚字段：[`crops/reference-scrollback-field-en.png`](assets/settings-scrollback-flow/crops/reference-scrollback-field-en.png) / [`crops/gpui-scrollback-field-en.png`](assets/settings-scrollback-flow/crops/gpui-scrollback-field-en.png)，差异遮罩 [`crops/diff-scrollback-field-en.png`](assets/settings-scrollback-flow/crops/diff-scrollback-field-en.png)
- 中文搜索行：[`crops/reference-search-result-zh.png`](assets/settings-scrollback-flow/crops/reference-search-result-zh.png) / [`crops/gpui-search-result-zh.png`](assets/settings-scrollback-flow/crops/gpui-search-result-zh.png)
- 选中侧栏行：[`crops/reference-selected-row-en.png`](assets/settings-scrollback-flow/crops/reference-selected-row-en.png) / [`crops/gpui-selected-row-en.png`](assets/settings-scrollback-flow/crops/gpui-selected-row-en.png)

差异遮罩是放大 3 倍的逐像素差，不是对齐证据；它把字体光栅、数字步进器缺失和文字宽度的差异显出来。

## 功能验证

测试用两条真实隔离 PTY，0.35/0.65 非均分分屏，打开设置前焦点在窗格 1。运行：

```sh
cargo test -p tterm-gpui --lib settings_page_tests -- --nocapture
```

11 项全部通过：

| 用例 | 覆盖 |
| --- | --- |
| `titlebar_click_opens_one_route_and_back_restores_the_exact_pane` | 鼠标开关、会话对象不变、焦点、分屏比例、返回后输入仍进原窗格 |
| `keymap_opens_the_singleton_route_and_escape_returns_input` | 快捷键打开、Escape 返回、焦点在窗格 2 |
| `remapped_settings_key_replaces_the_default_chord` | 改造键位后默认键不再打开 |
| `settings_input_does_not_leak_and_background_output_keeps_running` | 设置中输入不进入 PTY；隐藏 PTY 输出继续；Escape 先清搜索再返回 |
| `edited_scrollback_survives_reopening_and_stays_applied` | 无效值不保存；页面保存按钮；两条 PTY 的运行中容量变化；关闭后重新打开读到 `800` |
| `a_failed_save_keeps_the_page_open_and_the_running_capacity` | 写入失败时保留页面、草稿和运行容量，显示错误 |
| `search_finds_the_scrollback_setting_and_opens_it` | 英文查询、结果计数、结果文本、方向键、Enter、清除后输入值为空 |
| `search_reports_no_match_and_clears_before_closing` | 无结果提示、清除按钮、Escape 两级行为 |
| `search_accepts_chinese_and_mouse_selection` | 中文查询、鼠标选择、切回英文再选择 |
| `arrow_keys_move_between_the_page_and_its_setting` | 两条结果的上下键循环和 Enter |
| `page_layout_matches_the_reference_metrics` | 800×600 下的页面、标题、搜索框、侧栏、侧栏行、内容区、结果行、回滚字段和输入框尺寸 |

其中 `edited_scrollback_survives_reopening_and_stays_applied` 使用页面上的保存按钮，而不是测试私有入口；`set_settings_scrollback_value` 只负责把值填进真实 `InputState`。保存后两个 `Terminal` 模型的 `scrollback()` 都与 `resolve_scrollback_lines` 一致。

## 检查结果

在仓库根执行，全部通过：

| 命令 | 结果 |
| --- | --- |
| `pnpm lint` | 通过 |
| `pnpm format:check` | 通过 |
| `pnpm check:tokens` | 通过 |
| `pnpm typecheck` | 通过 |
| `pnpm test` | 187 个 Vitest 文件、1367 项通过；parity layout/reference 10 项、migration 8 项、perf 24 项通过 |
| `pnpm check:size` | 通过，1500 个文件在限制内 |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 通过，22 个测试目标无失败 |
| `node scripts/perf/gpui-release-frame-budget.mjs` | 通过：前台 p95 145µs（30 个后台会话，253 帧，120 基线 + 120 负载）；后台 30 MiB 在 150.639ms 内应用并确认（199.15 MiB/s） |

`window.rs` 仍在 800 行限制内（798 行），没有新增文件大小基线或豁免。

## 差异与遗留

- **侧栏只列 Terminal。** 原版侧栏有 Workspace、Experience、Data & security、Application 四组共 14 页；GPUI 只有一页。原因是没有实现的页面不摆放入口。因此两端的侧栏分组起点、滚动条和行数不同；行尺寸和选中样式是按同一套值对齐的。
- **默认页不同。** 原版从标题栏打开时默认 General；GPUI 打开唯一实现的 Terminal。没有伪造 General 页或空入口。
- **标题宽度不同。** 两端都是 18px/600，但 GPUI 的拉丁字体回退使 “Settings” 的图标+文字宽 113，原版 96.33。搜索占位文案也有轻微宽度差。没有把字体差异算作匹配。
- **数字输入没有步进器。** 原版 `input[type=number]` 右侧有系统上下按钮；GPUI 用单行文本输入，校验和保存行为一致。预设按钮、输入高度和宽度一致。
- **保存按钮位置。** 原版 Save 在完整字体页底部；GPUI 只有回滚字段，所以 Save 紧跟字段下方。局部字段对比不受影响，但不声明整页通过。
- **焦点策略。** GPUI 打开设置即聚焦搜索框，便于键盘操作；原版从侧栏点击页面时焦点在页签。搜索选择结果后两端都保留搜索框焦点。GPUI 点击已选中的侧栏行会清空搜索并保持搜索框焦点。
- **未移植定位框。** React 在打开搜索结果时画 1.8s 的 `--ring` 定位框并平滑滚动；GPUI 只清空查询、显示目标页并保留搜索焦点。字段本身在唯一页面的页首，不需要滚动。
- **`font` 查询在 GPUI 为空。** 原版会命中 Font Family、Font Size 和 Interface font scale；这些设置在 GPUI 尚未实现，所以搜索不返回假结果。本卡不把这项差异包装成“搜索对齐”。
- **只验证了深色、classic、100%。** 浅色、islands 和 125% 没有参照或实现，未比较。
- **不是原生桌面验收。** GPUI 截图来自真实隔离窗口，但仍是 macOS 上的窗口内容；没有验证原生窗口模糊、系统钥匙串、文件选择器或 Windows 标题栏。原版参照是 WebKit 内容区，不是 Tauri 原生窗口。
- **P4-15 未完成。** 搜索索引只覆盖已实现设置，懒加载和其余页面未做；本次实现可在 P4-15 正式卡中复用并补验，不另做一套设置界面。

## 验收边界

可以据此验收的部分：

- 在给定的 800×600、dark、classic、100%、en-US / zh-CN 条件下，设置页壳、搜索框、侧栏行、内容区、搜索结果行和回滚字段的 **局部几何与 token 颜色**。
- 两条真实隔离 PTY、非均分布局和原焦点下的打开/搜索/选择/修改/保存/返回流程。
- 英文/中文查询、无匹配、清除、Escape 和齿轮/快捷键返回的现有测试。
- 生产窗口截图和稳定布局数据的可复现性。

不能据此验收的部分：

- P4-15 整卡、G2、G3 或 P4 其他设置页。
- 整页像素一致；侧栏页面集合、拉丁字体宽度、数字步进器和 Save 位置均有已知差异。
- 浅色、islands、125%、Windows/macOS 原生窗口装饰、系统钥匙串和真实 SSH/SFTP 设置互通。
- 原版定位框动画和完整字体页的滚动定位。

## 提交

- `2e355275` `:bug: Activate the app when automation focuses the window`
- `eba19459` `:white_check_mark: Register the settings scrollback reference states`
- `6ba3e93e` `:white_check_mark: Capture the Chinese scrollback search reference`
- `a39443ad` `:white_check_mark: Wait for the settings reveal before the target capture`
- `1e2d8c4e` `:white_check_mark: Settle the Chinese search icon before capture`
- `562f84d9` `:bug: Draw the current frame for automation screenshots`
- `19c8820c` `:sparkles: Build the production settings shell around scrollback`
- `bf90ea76` `:white_check_mark: Wait for the save failure and read the cleared search`
- `c4e3ca8b` `:bug: Repaint the settings search field when focus changes`
- 本报告与资产：`:memo: Record the settings scrollback evidence`（见分支末尾）