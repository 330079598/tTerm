# P2-06 续批第 2 批：概览标尺命令与搜索标记

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-06-overview`，独立 worktree
- 起点：`ca04c95d`
- 代码提交：`43446663`（模型只读命令标记）、`108bb7d9`（视图绘制与搜索标记接口）
- 平台：macOS 27.2（26B123），arm64；rustc 1.99.1
- 状态：**自动验收通过，只提交本地分支，未推送、未合并。** 本批不宣告 P2-06 或 G2 整卡通过；原生桌面手测、真实 shell 集成和原版截图对比仍单独记录。

## 结论

本批把真实命令标记接到生产 `TerminalView` 的概览标尺，并留下 P2-07 才填数据的只读搜索标记输入。

1. 复用已有 `CommandMarks` 状态机，不复制提示符、C/D 标记和运行命令的判断。模型只增加一个有界 OSC 133/633 扫描器，在 Alacritty 读完该控制字符串的字节位置调用同一状态机。
2. 快照暴露的只读命令位置使用“保留历史坐标”：0 是最旧的历史行，最后可见行是 `history_size + screen_lines - 1`。正常输出滚动同时改变历史长度和 Alacritty 的行号，两者相加保持不变；已知的历史裁剪调用 `trim_scrollback`，未知淘汰则使标记失效。
3. 视图在现有 10px 滚动条轨道内绘制。命令标记在左侧区，搜索标记在中区；几何使用 `ScrollbarGeometry` 的同一轨道和历史行换算。高密度标记按逻辑像素行合并，源列表最多处理 4096 条，每帧绘制数不超过轨道像素行的两倍。
4. 搜索接口是 `TerminalView::set_search_marks(Vec<OverviewSearchMark>, cx)`，默认空集合。现有模型只有逐次 `find_next`，没有全回滚区扫描和稳定绝对行，因此生产路径不产生搜索结果，也没有提前绘制搜索入口。P2-07 必须提供可版本化的全量匹配行。

## 生产改动

| 位置 | 改动 | 原因 |
| --- | --- | --- |
| `crates/tterm-terminal/src/command_marks.rs` | 新增 `CommandMarkPosition`、`overview_marks()` 和单调 `revision()`；抽出已解析标记入口，`handle_mark()` 仍走同一解析与状态机 | 给视图一份只读绝对行快照，同时让模型按 revision 缓存列表 |
| `crates/tterm-terminal/src/command_marks/overview.rs` | 有界扫描器、`CommandMarkViewport` 的 Alacritty 适配、标记缓存 | 标记必须在精确字节位置读取光标；扫描器复用 `ControlStringFramer` 和 `parse_command_mark`，不新增状态机 |
| `crates/tterm-terminal/src/terminal.rs` | 解析事件与 OSC 52 事件按结束偏移合并；清历史、已知裁剪、容量淘汰和 resize 分别失效或平移；快照携带 `Arc<[CommandMarkPosition]>` | 一个模型只有一份命令状态；标记与模型生命周期同步 |
| `crates/tterm-terminal/src/snapshot.rs` | `Snapshot::command_marks()` | 绘制只读快照，不把标记塞进行缓存 |
| `crates/tterm-terminal-view/src/view/scrollbar.rs` | `ScrollbarGeometry::line_fraction()` | 概览行与滑块共用同一轨道换算 |
| `crates/tterm-terminal-view/src/view/overview.rs` | 固定颜色、左侧/中区几何、像素行合并、4096 条源上限 | 对齐原版概览尺并限制绘制次数 |
| `crates/tterm-terminal-view/src/view/frame.rs`、`prepare.rs` | 在轨道背景之后、滑块之前绘制标记；统计 `ViewStats::overview_marks` | 不改变滑块层级，也不改滚轮、拖拽或历史容量 |
| `crates/tterm-terminal-view/src/view/mod.rs` | `OverviewSearchMark` 输入、`set_search_marks`/`search_marks` 与默认空集合 | 明确 P2-07 的只读输入，不伪造搜索结果 |

### 生命周期

| 事件 | 结果 |
| --- | --- |
| 普通输出增加历史 | 标记坐标不变，概览尺位置随总行数缓慢移动；已验证追加 200 行后仍在原内容行 |
| `set_scrollback` 已知缩容 | 按实际减少的历史行数调用 `trim_scrollback`；越界提示符整条丢弃，保留的提示符、C 和 D 行同步平移 |
| 历史满容量后的未知淘汰 | 标记全部失效。Alacritty 0.26 不暴露精确淘汰行数；本批选择失效而不是让旧行号指向另一条命令 |
| `ED 3` / `RIS` 清主屏历史 | 标记全部失效；`ED 2` 只清视口时保留 |
| 备用屏 | 备用屏上的 A/C/D 和清除不进入主屏标记；返回主屏后原标记仍在；备用屏不绘制概览尺 |
| 窗口 resize / 重排 | 标记全部失效。现有 `CommandMarks` 没有重排映射，不能安全平移 |
| 分块 OSC 报告 | 跨写入的 133/633 在终止符处只应用一次 |

## 与原版概览区域的对照

对照来源是锁定版本的前端源码，而不是运行中的 Tauri 窗口：

- `@xterm/xterm 6.0.0`：`src/browser/decorations/OverviewRulerRenderer.ts`、`ColorZoneStore.ts`。
- `@xterm/addon-search 0.16.0`：`src/DecorationManager.ts`。
- 现有前端常量：`commandMarks.ts` 的 `#22c55e` / `#ef4444`、`position: "left"`；`searchTypes.ts` 的 `#facc15` / `#fb923c`；`openTerminal.ts` 的 `overviewRuler: { width: 10 }`。

| 项目 | 原版 | 本批 | 差异 |
| --- | --- | --- | --- |
| 轨道宽度 | 10 CSS px | 10 逻辑 px | 无 |
| 边框 | 1 物理像素；主题把 `overviewRulerBorder` 设为透明 | 不绘制透明边框 | 无可视差异 |
| 分区 | 1px 边框后左、中、右各约 3px | 命令左侧 x=1、宽 3；搜索中区 x=4、宽 3 | 无 |
| 单行标记高度 | `round(clamp(pixelsPerLine, 6, 12))` | `round(clamp(track / total, 6, 12))`，并限制在轨道内 | 无 |
| 纵向位置 | `round((height - 1) * line / total - markHeight / 2)` | `line_fraction() - markHeight / 2` 后取整、钳制 | 无 |
| 命令颜色 | 成功 `#22c55e`，失败 `#ef4444` | 同 | 无 |
| 搜索颜色 | 普通 `#facc15`，活动 `#fb923c` | 同 | 无 |
| 合并 | 同色、同位置的区域在按行计算的 padding 内合并 | 按逻辑像素行填充，后画的标记覆盖先画的；命令/搜索分区独立 | 局部差异：不同颜色相邻时本批只保留一个像素行颜色；没有按原版同色 padding 合并 |
| 滑块层级 | 滚动条在概览画布之上 | 先画标记，再画滑块 | 一致 |
| 备用屏 | 概览画布隐藏 | 不绘制标记 | 一致 |
| 提示符旁的 4px 圆点 | `commandMarks.ts` 的 decoration 在终端内容左内边距绘制 | 未实现 | 本批只做概览标尺，内容侧圆点留给后续批次 |
| 搜索结果 | addon-search 生成匹配装饰 | 输入接口默认空集合，生产无搜索 | P2-07 依赖，未伪造 |
| 设备像素 | 画布按 DPR 取整 | GPUI 逻辑像素绘制 | 预期存在亚像素/DPR 局部差异 |

没有修改 `parity/screens.json`、颜色门槛或任何整页阈值。因为本批没有取得原版同状态的原生截图，表中“无差异”仅指源码常量与布局公式，不代替视觉验收。原生截图与桌面颜色对比仍待补。

## 自动回归

### 模型

```sh
cargo test -p tterm-terminal --test overview_marks
```

10 项：

| 测试 | 断言 |
| --- | --- |
| `three_commands_expose_prompt_output_and_end_lines` | 三条命令的 A/C/D 位置为 `0/1/2`、`2/3/4`、`4/5/6`（屏高 5，已滚入 2 行历史），状态都为 0 |
| `output_marks_stay_on_their_rows_while_the_viewport_scrolls` | `Scroll::Top` / `Bottom` 不改变只读标记 |
| `trimming_known_history_shifts_and_drops_marks` | `set_scrollback` 后按实际裁剪量平移，越界标记丢弃 |
| `ed3_and_reset_clear_every_mark` | `CSI 3 J` 和 RIS 都清空标记 |
| `clearing_only_the_viewport_keeps_marks` | `CSI 2 J` 不清标记 |
| `alternate_screen_marks_do_not_pollute_the_main_screen` | 备用屏 A/C/D 与 `ED 3` 不改变主屏标记；回主屏仍在 |
| `an_eviction_discards_marks_that_cannot_be_shifted_exactly` | 满容量未知淘汰后标记为空 |
| `a_window_resize_discards_marks_because_reflow_has_no_line_map` | resize 后标记为空 |
| `an_empty_model_has_no_marks` | 空模型返回空集合 |
| `a_mark_split_across_writes_is_applied_at_its_terminator` | 跨写入的 OSC 133 只产生一个标记，位置和状态正确 |

### 视图几何

`crates/tterm-terminal-view/src/view/overview.rs` 与 `scrollbar.rs` 的单元测试：

- 空标记、备用屏不绘制；
- 命令左区、搜索中区、成功/失败/普通/活动颜色与纵向顺序；
- 5000 条命令 + 5000 条搜索源合并后，矩形数不超过轨道像素行的两倍；
- 同一行 50 条标记只剩一个矩形；
- `line_fraction` 覆盖 0、越界负行、越界正行和最后一行。

### 生产视图

```sh
cargo test -p tterm-terminal-view --lib view::overview_tests
```

| 测试 | 断言 |
| --- | --- |
| `marks_stay_on_their_lines_through_scrollbar_drags_and_appended_output` | 用真实 `TerminalView`、滚轮/指针监听和合成 OSC 输出；顶部拖拽让最旧提示符进入第 0 行，底部拖回后标记集合不变；追加 200 行后标记仍在原内容行，概览纵坐标随总行数上移 |
| `dense_marks_and_the_search_input_are_bounded_and_leave_scrolling_alone` | 300 条命令、随后 2000 条搜索输入仍在轨道像素行上限内；搜索默认空；出现标记后滚轮仍改变显示偏移 |
| `the_alternate_screen_does_not_paint_main_screen_marks` | 备用屏统计为 0，返回主屏后恢复 |

这些测试不读取截图专用 `ScreenView`，断言读取的是生产视图准备好的 `last_frame`、真实指针事件和绘制统计。

## 全套检查

实际运行：

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
| `cargo test --workspace` | 通过；`src-tauri` 73 + 1 ignored，`tterm-core` 1869 + 64 ignored，`tterm-gpui` 277 + 3 ignored，`tterm-terminal` lib 346、`history_clear` 9、`overview_marks` 10、`replay` 2、`scrollback_memory` 1，`tterm-terminal-view` 167，`tterm-parity` 85，`tterm-platform` 14，`tterm-ui` 85；无失败 |
| release 帧与吞吐门 | 通过；30 后台会话前台 p95 **146 µs**（预算 16,667 µs），30 MiB 在 **150,669 µs** 内全部应用并确认（约 **199.11 MiB/s**，最低 60 MiB/s） |

`cargo test --workspace` 使用默认并行参数，未追加过滤、skip 或 ignored 参数；仓库原有的 68 个 ignored 保持原样。release 门在独立 worktree 的 release 目标上运行，测量时没有并行执行其他任务。

## 桌面手测与未取得证据

以下只能由原生桌面手测取得，本批没有替代证据：

- macOS / Windows / Linux 原生窗口中的概览尺颜色、DPR 缩放、亚像素位置和滚动条叠放；
- 真实 bash / zsh / fish / PowerShell 的 OSC 133/633 节奏，包括同一写入内提示符与命令输出、分块和 tmux/ssh 转发；
- 真实窗口缩放重排时标记消失的可见效果；
- 原版 Tauri 窗口同状态概览区域的原始截图或视频。

## 遗留与依赖

- **未知淘汰**：满容量后 Alacritty 不提供精确淘汰行数，本批选择丢弃全部标记；原版 xterm.js 的 marker 会随 `onTrim` 平移。需要精确平移时，应在后续批次引入行身份或上游滚动计数，而不是猜测行数。
- **重排**：没有重排映射，resize 后标记失效；原版会移动或销毁 marker。
- **搜索**：`tterm-terminal::search::Search` 目前只有一次 `find_next`，没有全量扫描、取消或结果 generation。P2-07 必须提供稳定的绝对行集合，并在淘汰、清除、备用屏和 resize 后重新扫描；本批只留下空的 `set_search_marks` 输入。
- **内容侧提示符圆点**：本批只画滚动条轨道内的概览标记，未绘制命令提示符左侧的 4px 圆点。
- **命令跳转与选中输出**：`CommandMarks` 已有状态，但本批只暴露位置，不接 P2-13 的跳转、输出选择和侧栏入口。
