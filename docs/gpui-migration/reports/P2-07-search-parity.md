# P2-07 第三批：终端搜索栏原版对照与拖动验收

本报告按[执行卡](P2-07-search-parity-plan.md)记录两个确定状态：默认位置，以及从把手拖到右下边界后。**在 800×600 的这两帧，栏框、查询焦点、命中位置、计数和命中填充色一致；整屏并非像素相同。** 没有运行原生输入法，也没有接入真实 shell 或 SSH。这不是 P2-07 整卡或 G2 签收。

## 参照与状态归一化

- **WebKit 原版**：[截图、DOM 布局与运行态](evidence/P2-07-search-parity/webkit/)。Playwright WebKit 26 挂载生产 `TerminalSearchBar`、`useTerminalSearch`、生产 CSS 和 xterm.js SearchAddon，不是为截图重画的界面。
- **GPUI 候选**：[截图与运行态](evidence/P2-07-search-parity/gpui/)。无桌面渲染器挂载生产 `SearchBar` 和 `TerminalView`；从截图帧读取栏框、表面、输入焦点和活动命中。下表坐标为逻辑像素，PNG 均为 1600×1200 物理像素。
- 两端使用相同的 80 行合成数据，英文、深色、100%、DPR 2。大小写、全字和正则均关闭，查询 `needle`。第 28 项是第 27 行（从零计）、第 23 至 28 列；WebKit 选区末列 29 为**不含端点**，与 GPUI 记录的末列 28 相同。
- **归一化只用于取证。** WebKit 先从底部向上滚动 30 行，输入 `needle` 后点 27 次“下一个”；每次活动结果变化，夹具把视口恢复到 `baseY - 30` 并重新聚焦输入框。GPUI 通过 Enter 导航到相同的命中，再把可见首行设为 14。计数和焦点从实际控件读取，而非静态标签。截图证明最终状态可比，不能证明未经夹具干预时的首个命中、导航滚动或焦点恢复策略相同。

## 两帧的实测结果

| 项目 | WebKit | GPUI | 结论 |
| --- | --- | --- | --- |
| 初始栏框 `(x,y,w,h)` | `(264,12,522,44)` | `(264,12,522,44)` | 四项差值均为 0 |
| 拖后栏框 `(x,y,w,h)` | `(270,548,522,44)` | `(270,548,522,44)` | 拖动已结束，右、下各留 8 px |
| 可见首行、活动项和计数 | `14`，`27`（零基），`28 / 80` | 同左 | 两帧均保持输入焦点、活动项可见 |
| 非活动／活动命中填充 | `#facc15`／`#fb923c` | `#facc15`／`#fb923c` | PNG 取样一致 |
| 终端列×行；`baseY` | `93×37`；`44` | `98×32`；`49` | GPUI 多 5 列、少 5 行，行高不同 |

取样位置为默认 PNG 的物理像素：WebKit 非活动 `(289,122)`、活动 `(293,416)`；GPUI 非活动 `(370,120)`、活动 `(370,499)`。GPUI 在字形之前绘制不透明填充；命中文字用 `#1b1d23`，与两种底色的对比度分别为 11.00:1、7.44:1。命中以外的 ANSI 字色不变。`tterm-terminal-view` 的搜索相关定向回归已跑 14 项，全通过。

**仍有差异或未证实：** 网格不同，同一命中不在截图的同一像素位置。WebKit 记录了 `backdrop-filter: blur(14px)`；没有等效的 GPUI 模糊半径实测。WebKit 有各子控件的 DOM 尺寸，GPUI 截图 JSON 只记录了终端表面与栏外框；相同外框不等于把手、图标和字形逐像素一致。没有为这些差异放宽基线。

## 截图独立复跑

在**含本批两个 capture 入口的仓库根目录**运行；需要项目锁定的 pnpm 依赖、Playwright WebKit 和可用的本地 headless 渲染器。输出到新建临时目录，不覆盖存档。

```sh
out=$(mktemp -d)
node scripts/parity/capture.mjs --platform macos --theme dark --ui-style classic --ui-scale 100 \
  --screen terminal.search.default --screen terminal.search.dragged --out "$out/webkit"
cargo run --locked -p tterm-gpui --example search_capture --features tterm-gpui/automation -- \
  --output "$out/gpui"
```

分别查看 `terminal.search.{default,dragged}.state.json`、同名 PNG，WebKit 另有 `.layout.json`。当字体、系统和渲染器一致时，可用 `diff -rq docs/gpui-migration/reports/evidence/P2-07-search-parity/gpui "$out/gpui"` 比较 GPUI 文件；本批两次无桌面生成的四个文件完全一致。不要用两套不同渲染器的整图哈希判定局部匹配。

## 原生 macOS 拼音：待人工

`InputState` 的 marked-text 更新、取消、提交可以用 GPUI 文本输入协议驱动；分发已经提交的 CJK 字符则连组合过程都不会经过。两类测试都不会打开 macOS 候选窗，不能代替下列复验。请使用**专用测试桌面与合成内容**，不对正在使用的桌面切换输入源或发送全局按键。

1. 打开两个可区分的测试窗格；右窗格有两处「中文」并获得焦点。按 `Cmd+F` 打开右窗格搜索栏，在简体拼音下键入 `zhong`，不选候选。确认预编辑可见、候选窗锚在搜索输入框，两窗格都没有收到这些字母。
2. 组合中按一次 `Esc`。候选和预编辑应取消，搜索栏仍然打开并持有焦点，原有已提交查询不丢失，两窗格都不应收到 Esc。
3. 重新组字并确认「中文」。确认候选消失，查询和计数更新；再按 `Esc`，应关闭搜索栏。输入一个 ASCII 字符且不按回车，仅原右窗格应收到它，左窗格不变。记录构建、macOS 版本、输入源和三步实际观察；未运行不得标为通过。

## 协调者最终验收

- **几何与输入隔离通过自动验收。** 600px 窗口初始左右和顶部留 10px，从把手拖动后按 8px 夹紧；换行分屏以实际外框测右下边距，两边均为 8px。两个窗格同时启用 1003/1006 上报：修复前，越到邻窗格的滚轮及分屏从 50:50 改为 65:35 后的移动都向邻窗格写入了鼠标报告；修复后两条回归通过，额外右键不抢焦点，释放后正常上报恢复。完整生产窗口中，连续组字后第一次 Esc 保留原查询和输入焦点，第二次 Esc 关闭栏并返回活动终端。
- **全量通过。** 协调者在最终候选依次运行前端六项、`cargo fmt --all --check`、严格 workspace Clippy、`cargo test --workspace`，重新生成两态 GPUI 截图与状态文件并逐字节核对。最后单跑 `node scripts/perf/gpui-release-frame-budget.mjs`：30 个后台会话时前台帧 p95 为 499.958 µs（预算 16.667 ms）；独立 30 MiB 输入全部应用并确认，用时 177.443 ms（169.07 MiB/s，截止 500 ms）。没有调整负载或阈值；这不是桌面滚动或 Ghostty 性能对比。
- **本地合并。** 实现与证据 `9ffb5d6e` 合入 `GPUI`（`084100b1`），已核对两者源码一致，未推送。原生候选窗口、真实 shell 和原生桌面滚动仍待验；不签收 P2-07 整卡或 G2。
