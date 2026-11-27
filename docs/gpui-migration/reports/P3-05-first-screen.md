# 首屏、SSH 连接列表与设置：视觉验收

本批以 Tauri 客户端为设计来源，不另做一套配色。主题编辑、导入及尚不可操作的设置分类不在本批。不要为了截图添加无效按钮。

## 方案与签收条件

1. **首屏**：用隔离配置分别启动默认深色与浅色工作区；核对标题栏、活动标签、终端背景、前景、光标和 ANSI 色。换主题不能只改窗口底色；新标签应使用相同配置，无法解析的主题不应覆写用户选择。
2. **连接列表**：在两端装入相同的合成主机和分组，不建立连接。对照分组层级、连接名称与地址、选中及悬停状态；键盘选择、搜索、取消后返回原窗格仍然可用。
3. **设置**：以可操作的终端设置页核对侧栏、当前项、搜索框、内容标题及第一组字段的间距和字体。已有保存、取消及焦点行为不得退化；不把缺少的分类算作已对齐。
4. **取证**：用同尺寸、同像素密度、同语言和数据的深浅色截图比较，不混用旧截图和本轮产物。记录截图是原生桌面、无界面原生渲染还是 WebKit 参照。按[界面一致性 §5.2](../ui-parity.md#52-通过门槛windows100-缩放macosdpr-2)区分局部改善与整屏通过。
5. **质量门禁**：工作区全量检查通过后，在空闲机器上单独跑 release 帧预算及吞吐测量。数值不过则报告实测，不放宽门槛。

实现由 Pi + DS 4.1 Flash 负责，协调者独立复核。结论分别记录功能、视觉与性能；缺证据不签收。

## 基准预检

在改动前分别以 `--theme dark`、`--theme light` 运行 `scripts/parity/capture.mjs`。标题栏参照有变化，但 `settings.terminal.font` 两张 PNG 的 SHA-1 均为 `9a595584d5e5c15c813d410ca9c60a07d766aa66`：设置页挂载的 `ThemeProvider` 会读夹具里的已保存主题，覆盖入口设置的外观。不能把这组文件当作两种主题的证据；验收需要检查实际显示的颜色，而不只检查目录名。

## 2026-11-27 协调者复验：功能通过，整页视觉不通过

macOS，英文、classic、100%、800×600 逻辑像素、2×。GPUI 使用真实隔离窗口；参照是 WebKit 中的 React 组件，不是系统桌面截图。复现：先运行 `node scripts/parity/first-screen.mjs`，再运行 `cargo run -p tterm-parity --example first_screen_scores`。后者使用仓库现有的整屏 SSIM 算法和 0.97 门槛，为未达标的屏生成热力图并以非零退出。图片、夹具摘要和热力图在 `target/parity/first-screen/`。

| 屏幕 | 深色整屏 SSIM | 浅色整屏 SSIM | 主要未解决差异 |
| --- | ---: | ---: | --- |
| 工作区、标题栏与终端 | 0.911830 | 0.913473 | 标签与工具入口的排列、终端文字栅格与光标。 |
| 已保存 SSH 连接 | 0.809220 | 0.788839 | 弹窗与列表空间已修正；行内操作、认证摘要、底部操作区和遮罩仍不同。 |
| 设置 → 终端 | 0.606141 | 0.614567 | 侧栏 3 页对 14 页；原版 WebView 渲染器字段在原生端不适用，后续内容的起点不同。 |

**签收范围**：六套内置主题的读取与选择、连接调色板覆盖、OSC 查询、标题栏入口、SSH 弹窗自适应，以及已实现设置的操作。主题选择写共享配置，须重启 GPUI；不支持运行中换肤、主题库或自定义主题编辑。不签收整屏、跨平台或 G2/G3。未添加空控件、图像豁免或更宽松门槛。

协调者复跑前端六项、Rust 格式、workspace 严格 Clippy、工作区测试，以及 automation 截图示例的严格 Clippy。工作区测试保持默认并发，使用独立临时目录避免其他测试进程删除同名 50 MiB 夹具。独立 release 门禁：前台加载 p95 **0.299 ms**（上限 16.667 ms）；后台 30 MiB 全部应用并确认，**154.855 ms / 193.73 MiB/s**（上限 500 ms）。下一步优先处理 SSH 行内操作与设置侧栏；原生端不适用的字段不能伪装成可操作项来换取截图分数。

## 第二轮复核（2026-11-27）

首轮整屏 SSIM 为暗 0.8965、亮 0.8969（16×16 灰度窗口，未遮文字），未达到 §5.2。复核确认不是 GPUI 读错了配置：`first_screen_capture/seed.rs` 写入的是 `AppConfig::default()`（14 px、行高 1、左右 6、下 4），与前端默认值一致。差异来自参照夹具和表头几何。

- 参照的 `FirstScreenTerminal` 传入了 `padding: 0`，又没有加载 renderer，xterm.js 因此落到 DOM 渲染器；`.terminal-surface .xterm .xterm-rows { font-family: inherit }` 又把字体换成系统 UI 字体，正文于是变成比例字体、16 px 行高。现在改为读取 `load_config` 的 `font_family`、`font_size`、`terminal_line_height`、`terminal_letter_spacing`、光标、回滚和 `terminal_renderer`，加载生产默认的 WebGL（失败回退 Canvas），并使用生产 `.terminal-surface` 的 6/6/4 内边距。DOM 回退也不再被产品 CSS 抢走字体。
- 两端都把活动 terminal 设为焦点，并去掉夹具输出末尾多余的一个 CRLF。最后一行就是等待输入的提示符，光标不再落在空行上；两端仍使用完全相同的字节。
- GPUI 表头补上 React `.tab-list` 的 4 px 内边距、`.tab-item` 的 1 px 透明边框、活动标题 500 字重和真实 `SquareTerminal` 13 px 图标。`+` 改成 28×28 的 Lucide plus（16 px、muted 前景）；原先的 `+` 使用 gpui-component 的深色文字，在深色表头上几乎不可见。
- GPUI terminal 单元格高度改用 WebKit/Chromium 报告、xterm.js 相加的整数 ascent/descent（JetBrains Mono 14 px 从 18.5 改为 18 px）；宽度继续按 WebGL 渲染器的设备像素向下取整。

同一组官方截图（`node scripts/parity/first-screen.mjs`，1600×1200）重测：暗 0.8965 → 0.9118，亮 0.8969 → 0.9135。整屏仍低于 0.97；以下差异没有用例外或门槛调整掩盖。

- **字体可见性。** Playwright WebKit 读不到 `~/Library/Fonts` 里的 JetBrains Mono，参照回退到 Menlo（16 px 行高）；GPUI 通过 CoreText 解析到 JetBrains Mono（18 px 行高）。字形和纵向节奏仍是主要差异。真实 Tauri WKWebView 能读取用户字体，这是取证环境差异，不把参照改成 Menlo 去迁就原生。
- **单标签的关闭按钮。** React 的活动标签始终显示关闭按钮；GPUI 只有一个标签时 `closable = false`，不显示。活动标签因此窄约 31 px，`+` 位置左移。没有为了对齐加一个不存在的按钮。
- **标签条动作。** React 在标签旁有 Profiles 和 Command Library 两个动作；GPUI 没有 Command Library 实现，Profiles 由右侧 saved 动作承担。右侧因此是 4 个动作（tunnels、layouts、saved、settings），不是参照的 2 个（tunnels、settings）。
- **Canvas 渲染器。** Tauri 的 canvas 回退保留小数单元格宽度；GPUI 目前只按默认 WebGL 的设备像素向下取整。用户在 Tauri 里选 canvas 时列数会与 GPUI 不同；首屏使用默认 WebGL，不受影响。
- **表头字体。** 标签序号和标题使用 GPUI 的界面字体，参照使用 system-ui；字重、内边距和纵向位置已对齐，字形仍有差别。
- **浅色调色板。** 此前的“固定 default”记录不再适用；本批两端都使用 light 预设。
- **布局 JSON。** 参照改用 Canvas 后，terminal 正文不再是 DOM 文本，`workspace.first-screen.layout.json` 的 `textRegions` 只剩表头 3 条；首屏 terminal 的比较以图像为准。

质量门禁：`node scripts/perf/gpui-release-frame-budget.mjs` 通过，frame p95 120.6 µs（预算 16.667 ms），throughput 198.62 MiB/s（30 MiB / 500 ms）。
