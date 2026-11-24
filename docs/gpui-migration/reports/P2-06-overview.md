# P2-06 续批第 2 批返工：概览标尺的标记同步与绘制

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-06-overview`，独立 worktree
- 起点：`1cbf6682`（第一版本批报告）
- 代码提交：`5719b0f7`（命令标记跟随解析器执行的历史变化）、`22751c13`（DECCOLM 同步重置镜像滚动区域）、`6b52ef77`（只按保留历史绘制概览标记）
- 平台：macOS 27.2（26B123），arm64；rustc 1.99.1
- 状态：**自动检查通过；只提交本地分支，未推送、未合并。** 本批仍不宣告 P2-06 或 G2 整卡通过。原生桌面手测、真实 shell 集成、原版同状态截图均未完成；P2-07 的搜索 UI 没有实现。

第一版 `P2-06-overview-marks.md` 中“第二套 OSC 扫描器 + 写后按历史差估算裁剪 + 容量满即丢弃全部标记”的方案已被本文件取代。旧文只保留为返工前记录，不再作为验收依据。

## 结论

本批返工了四个根因，而不是给第一版补几条断言：

1. **不再有第二套命令标记扫描器。** 模型改用生产 `SessionReportScanner` 的同一实现和同一状态机，只增加 `for_command_marks()` 配置与带结束偏移的只读结果。扫描器只负责找到完整控制字符串的结束字节；标记要等同一个 `Processor` 消费到该结束字节之后才应用，位置来自 Alacritty 当时的真实 cursor/history，不是扫描器猜的行号。同步更新内尚未执行的控制字符串不生成假位置：该批标记作废，下一个提示符重新建立。
2. **历史移动在解析器执行点记录，不再按一次 write 前后的历史行数估算。** `HistoryRelease` 继续把每条命令原样转发给 Alacritty，同时镜像滚动区域，并在 `linefeed`、`newline`、`scroll_up`、`delete_lines`、`ED 2` 的 viewport 清屏、`ED 3` 和 `RIS` 真正执行时记录“丢弃全部”或“淘汰 N 行”。标记在同一个 parser 调用内平移，因此一次 write 与逐字节 write 得到同一列表；“清历史后同批新标记”也能保留清除之后的新命令。
3. **概览坐标只使用真实标记绝对行、`history + screen` 总行和现有 `ScrollbarGeometry`。** 保留历史为空时不画标记；备用屏不画；轨道背景之后、滑块之前绘制，10px 轨道内的左侧命令区和中间搜索区不会盖住滑块。
4. **测试改为生产路径证据。** 模型测试先证实淘汰、清除、备用屏、同步更新和分块边界，再用正式 `TerminalView`、真实输出和鼠标/滚轮事件验证布局，并新增 `Window::painted_quads()` 断言证明 layout 确实进入绘制场景。空搜索源不产生任何搜索颜色矩形。

搜索仍然只是只读输入接口 `set_search_marks`，生产默认空集合；本批没有 P2-07 的搜索栏、匹配扫描或结果导航。

## 根因与修复

### 1. 扫描边界与执行时序

| 返工前 | 现在 |
| --- | --- |
| `CommandMarkScanner` 自己持有第二个 `ControlStringFramer`，与生产 `SessionReportScanner` 分别维护跨 write 状态 | 命令标记直接使用 `SessionReportScanner`；`new()` 与 `for_command_marks()` 共用同一个 `ControlStringFramer` 状态机 |
| 在 `write` 前扫描后按事件 `end` 与 OSC 52 store 合并；没有证明两个扫描器的控制字符串边界一致 | 扫描器仍可先给出边界，但每个 mark 只在 `parser.advance(..event.end)` 返回后应用；新增测试把同一段包含 OSC/DCS/标题/OSC 8/ESC 非 ST 终止的字节流在每一个字节处分块，比较生产 scanner 与命令 scanner 的全部 133/633 结束偏移 |
| 同步更新中的 OSC 只被缓存，尚未执行，标记却可能读到旧 cursor | `Processor::sync_bytes_count() > 0` 时丢弃该批标记，不写入假坐标；同步更新内的清除/滚动仍在 ESU 真正执行时更新历史。下一个提示符重新建立标记 |
| 备用屏切换在 write 末尾提前返回，主屏在切换前发生的淘汰可能没有平移 | 每次主屏滚动都在执行点记录；先滚动再 `?1049h` 与单独滚动的标记结果一致 |

OSC 52 store 仍由独立的 `Osc52Scanner` 和命令扫描器分别扫描，但两者使用同一个有界 `ControlStringFramer` 状态机；OSC 52 不与命令标记共享状态，也不影响命令标记的位置。本批没有把 OSC 52 合并进 `SessionReportScanner`，避免把剪贴板策略塞进 session report 类型。

### 2. 历史淘汰与清除

| 解析器命令 | 现在的标记处理 |
| --- | --- |
| 普通输出、`LF`、`IND`、`NEL` 在区域底端滚动 | 若历史未满，行进入历史，已有绝对行不变；若已满，精确淘汰的行数立即 `trim_scrollback` |
| `CSI S`、`CSI L` 正向滚动 | 仅在滚动区域从第 0 行开始时影响历史，按实际位置数平移 |
| `ED 2` 清 viewport | Alacritty 先把非空行滚入历史；按它实际滚动的行数平移，不把“清 viewport”误当成“清历史” |
| `ED 3`、`RIS` | 在执行点丢弃所有标记；同一批后续的 A/C/D 重新建立新标记 |
| 备用屏 | 备用屏上的标记和清除都不进入主屏；主屏在切换前的滚动已经同步 |
| `set_scrollback` 缩小容量 | 主屏按实际减少的历史行数平移；备用屏隐藏主屏时无法知道精确行数，丢弃标记 |
| resize / reflow | 仍丢弃全部标记；没有重排行映射 |
| DECCOLM 设置/取消 | Alacritty 会重置滚动区域；镜像区域同步重置，避免之后的换行被误判为全屏历史滚动 |
| 同步更新 | 清除/滚动延迟到 ESU 执行；同步缓冲内尚未执行的命令标记作废 |

`CommandMarks::trim_scrollback` 继续复用核心状态机：提示符行被淘汰时整条 entry 丢弃，输出/结束行被淘汰时只把对应位置变成不可用。标记不再是“满容量就全部丢弃”。

### 3. 布局与绘制

- `layout_for_snapshot` 传入 `snapshot.history_size()` 和 `snapshot.grid().screen_lines`；`total_lines = history + screen`，与 xterm 的 `buffer.lines.length` 口径一致。
- `history_lines == 0`、备用屏、轨道高度小于 1px 都返回空布局；纯布局测试和正式视图的 painted-quads 测试都覆盖空历史。
- 行位置仍由 `ScrollbarGeometry::line_fraction` 计算；单行高度为 `round(clamp(track / total, 6, 12))`，并钳制在轨道内。
- 轨道 10px：命令区 `x=1`、宽 3；搜索区 `x=4`、宽 3；剩余 3px 属于原版的右区，本批未使用。
- 绘制顺序是轨道背景 → 概览矩形 → 滑块，因此滑块在最上层，10px gutter 不会让标记遮住 thumb。新增测试从 `painted_quads()` 中找到命令色矩形和滑块，断言矩形宽度、在轨道内、且在滑块之前绘制。
- 颜色来自现有前端的固定概览常量：命令成功 `#22c55e`、失败 `#ef4444`，搜索普通 `#facc15`、活动 `#fb923c`。这些颜色不跟主题调色板变化，和原版 `commandMarks.ts` / `searchTypes.ts` 一致。
- 高密度标记按逻辑像素行合并，每个轨道像素行每个区最多一个矩形；源列表最多处理 4096 条。不同颜色落在同一像素行时后画者覆盖先画者，这是有限像素高度的局部差异，已经写进下面的原版对照。

## 与原版概览区域的差异

对照来源仍是锁定版本的前端源码：`@xterm/xterm 6.0.0` 的 `OverviewRulerRenderer.ts`、`ColorZoneStore.ts`，`@xterm/addon-search 0.16.0` 的 `DecorationManager.ts`，以及本仓库 `commandMarks.ts`、`searchTypes.ts`、`openTerminal.ts`。这不是运行中原版窗口的截图对比。

| 项目 | 原版 | 本批 | 差异 |
| --- | --- | --- | --- |
| 轨道宽度 | 10 CSS px | 10 逻辑 px | 无 |
| 边框 | 1 物理像素；本仓库主题设为透明 | 不绘制透明边框 | 无可视差异 |
| 分区 | 1px 边框后左/中/右约 3px | 命令左区 3px、搜索中区 3px | 不画未使用的右区 |
| 单行高度 | `round(clamp(pixelsPerLine, 6, 12))`，物理像素 | `round(clamp(track / total, 6, 12))`，逻辑像素 | DPR 取整可能有亚像素差异 |
| 纵向位置 | `round((height - 1) * line / total - markHeight / 2)` | `line_fraction - markHeight / 2` 后取整、钳制 | 公式一致，DPR 取整不同 |
| 命令颜色 | 成功 `#22c55e`，失败 `#ef4444` | 同 | 无 |
| 搜索颜色 | 普通 `#facc15`，活动 `#fb923c` | 同 | 无 |
| 合并 | 同色、同位置在按行计算的 padding 内合并 | 按逻辑像素行填充，后画者覆盖先画者 | 不同颜色相邻时只保留一个像素行颜色 |
| 空历史 | 装饰行在屏幕内时仍可画在画布上 | 保留历史为 0 时不画 | 本批有意采用更保守的“只画历史”规则 |
| 滑块层级 | 滚动条在概览画布之上 | 先画标记，再画滑块 | 一致 |
| 备用屏 | 画布隐藏 | 不绘制标记 | 一致 |
| 提示符旁 4px 圆点 | `commandMarks.ts` 在内容左内边距绘制 | 未实现 | 只做概览轨道 |
| 搜索结果 | addon-search 生成匹配装饰 | 输入接口默认空集合，生产无搜索 | P2-07 依赖，未伪造 |
| 设备像素 | 画布按 DPR 取整 | GPUI 逻辑像素绘制 | 预期存在亚像素/DPR 局部差异 |

没有修改 `parity/screens.json`、颜色门槛或整页阈值。上表的“无差异”只表示源码常量与布局公式一致，不代替原生窗口截图或桌面颜色验收。

## 自动回归

### 模型与扫描器

```sh
cargo test -p tterm-terminal --test overview_marks
cargo test -p tterm-terminal --lib session_reports
cargo test -p tterm-terminal --lib history_release
```

结果：`overview_marks` 15 项通过，`session_reports` 7 项通过，`history_release` 1 项通过。

`overview_marks` 的关键断言：

| 测试 | 断言 |
| --- | --- |
| `three_commands_expose_prompt_output_and_end_lines` | 三条命令的 A/C/D 绝对行和状态正确 |
| `output_marks_stay_on_their_rows_while_the_viewport_scrolls` | 滚动 viewport 不改变标记 |
| `trimming_known_history_shifts_and_drops_marks` | `set_scrollback` 按实际裁剪量平移并丢弃越界提示符 |
| `ed3_and_reset_clear_every_mark` | `CSI 3 J`、RIS 在执行点清空 |
| `clearing_only_the_viewport_keeps_marks` | `CSI 2 J` 不清主历史标记 |
| `alternate_screen_marks_do_not_pollute_the_main_screen` | 备用屏 A/C/D 和 ED3 不碰主屏 |
| `an_eviction_shifts_surviving_marks_by_the_exact_rows` | 历史未满时不动；满容量后第一条淘汰、第二条精确上移一行 |
| `a_scroll_before_an_alternate_screen_switch_still_moves_main_marks` | 先滚动再换备用屏仍按主屏实际淘汰平移；备用屏清除不改主屏 |
| `a_window_resize_discards_marks_because_reflow_has_no_line_map` | resize 后不保留可能错位的行 |
| `a_mark_split_across_writes_is_applied_at_its_terminator` | OSC 133 跨 write 只在终止符处应用一次 |
| `control_string_payloads_do_not_create_command_marks` | OSC 标题、DCS、OSC 8 内的 `133;A` 文本不产生标记 |
| `a_clear_and_new_marks_in_one_write_keep_only_the_new_command` | 同一批 `ED 3` 后的新 A/C/D 保留，旧命令丢弃；逐字节写入结果相同 |
| `a_synchronized_update_does_not_give_a_buffered_mark_a_fake_position` | BSU 内的标记不生成假位置；下一提示符恢复 |
| `one_write_and_byte_writes_agree_on_clears_alternate_screens_and_evictions` | 长流一次写入与逐字节写入得到相同标记和相同历史行数 |

`session_reports` 的 `command_mark_boundaries_match_the_production_report_scanner` 在同一个对抗性字节流的每一个字节位置切分，比较生产 scanner 与命令 scanner 的全部 133/633 结束偏移；两者相同。

### 正式视图与绘制

```sh
cargo test -p tterm-terminal-view --lib view::overview
```

结果：12 项通过（7 项纯布局 + 5 项正式视图）。正式视图测试使用生产 `TerminalView`、滚轮/鼠标按下与抬起、滚动条拖拽和合成 OSC 输出：

| 测试 | 断言 |
| --- | --- |
| `marks_stay_on_their_lines_through_scrollbar_drags_and_appended_output` | 拖到顶部让最旧提示符进入第 0 行；拖回底部标记集合不变；追加 200 行后仍在原内容行，概览纵坐标随总行数变化 |
| `dense_marks_and_the_search_input_are_bounded_and_leave_scrolling_alone` | 300 条命令和 2000 条搜索输入仍在轨道像素行上限内；搜索默认空；滚轮仍能滚动 |
| `the_alternate_screen_does_not_paint_main_screen_marks` | 备用屏统计为 0，返回主屏恢复 |
| `overview_layout_is_painted_in_the_track_under_the_thumb` | `painted_quads()` 中出现命令色矩形；宽度 3px、位于 10px 轨道内、在滑块之前绘制；空搜索源没有搜索色矩形 |
| `empty_history_paints_no_overview_marks_even_when_a_command_is_tracked` | 模型有命令标记但历史为 0 时，绘制统计和实际 quads 都没有概览标记 |

纯布局测试另外覆盖：空标记、备用屏、空历史、命令左区/搜索中区、成功/失败/普通/活动颜色、绝对行从最旧历史到最新屏幕的映射、5000+5000 源合并后不超过轨道像素行两倍、同一行 50 条只留一个矩形。

这些是自动绘制证据，不是桌面原生窗口验收。GPUI 测试窗体的 `painted_quads()` 能证明 layout 进入场景，但不能证明 macOS/Windows/Linux 实际窗口的 DPR、亚像素和颜色感知。

## 全套检查

在 `22751c13` 的工作树上实际运行；之后的文档提交不改代码。

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
node scripts/perf/gpui-release-frame-budget.mjs
```

| 检查 | 结果 |
| --- | --- |
| 前端六项 | 通过；187 个测试文件、1367 个测试；`file-size: 1505 files within their limits` |
| `cargo fmt --all --check` | 通过 |
| 严格 Clippy | 通过，`-D warnings` |
| `cargo test --workspace` | 通过；`src-tauri` 73 + 1 ignored，`tterm-core` 1869 + 64 ignored，`tterm-gpui` 277 + 3 ignored，`tterm-terminal` lib 347、`history_clear` 9、`overview_marks` 15、`replay` 2、`scrollback_memory` 1，`tterm-terminal-view` 172，`tterm-parity` 85，`tterm-platform` 14，`tterm-ui` 85；无失败 |
| release 帧与吞吐门 | 通过；提交 `22751c13` 上 30 后台会话前台 p95 **119 µs**（预算 16,667 µs），30 MiB 在 **151,775 µs** 内全部应用并确认（约 **197.66 MiB/s**，最低 60 MiB/s） |

`cargo test --workspace` 使用默认并行参数，未追加过滤、skip 或 ignored 参数。release 门在独立 worktree 的 release 目标上运行，测量时没有并行执行其他任务。

## 桌面手测与未取得证据

以下仍只能由原生桌面手测取得，本批没有替代证据：

- macOS / Windows / Linux 原生窗口中的概览尺颜色、DPR 缩放、亚像素位置和滚动条叠放；
- 真实 bash / zsh / fish / PowerShell 的 OSC 133/633 节奏，包括同一写入内提示符与命令输出、分块、tmux/ssh 转发和同步更新；
- 真实窗口缩放重排时标记消失的可见效果；
- 原版 Tauri 窗口同状态概览区域的原始截图或视频；
- 真实触摸板/鼠标在 10px 轨道上的拖动和标记共存手感。

## 遗留与依赖

- **同步更新内的命令标记**：为了避免假位置，本批选择作废该批标记；Alacritty 的缓存在 ESU 一次执行，无法在不重放缓冲的前提下取得缓冲内多个标记的中间 cursor。原版 xterm.js 由自己的 parser 逐段执行，因此会保留这些标记。
- **重排**：仍没有重排行映射，resize 后标记失效；原版会移动或销毁 marker。
- **搜索**：`tterm-terminal::search::Search` 仍只有一次 `find_next`，没有全量匹配行、取消或 generation。P2-07 必须提供稳定绝对行，并在淘汰、清除、备用屏和 resize 后重新扫描；本批只保留空的 `set_search_marks` 输入。
- **内容侧提示符圆点**：只画概览轨道，未画提示符左侧 4px 圆点。
- **命令跳转与选中输出**：`CommandMarks` 已有状态，但本批没有接 P2-13 的跳转、输出选择和侧栏入口。
- **OSC 52 合并**：OSC 52 与命令标记仍各自扫描同一批 bytes，共用有界 framer 但不共享一次遍历；如果未来要减少一次扫描，应在不改变控制字符串状态机的前提下合并，而不是新增第三套 parser。
