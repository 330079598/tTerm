# P2-06 续批第 1 批返工：回滚清除与内存

- 日期：2026-11-23
- 分支 / worktree：`migrate/P2-06-scroll-memory`，独立 worktree
- 起点：`03ab675d`
- 代码提交：`8fe8ba44`
- 测量修正提交：`9da7722c`
- 平台：macOS 27.2（26B123），arm64；rustc 1.99.1
- 状态：**本批通过自动验收，已集成本地 `GPUI`（`48a0840d`），未推送。** 协调者验证集成树与候选 `ce732733` 内容相同。独立连续复跑 15 次内存测试，容量 500 行的夹具清除后存活分配减少 1,920,000 字节（不是 RSS）；前端六项、fmt、严格 Clippy、workspace 测试全部通过。release 下 30 个后台会话的前台帧 p95 为 0.119 ms，独立 30 MiB 吞吐耗时 167.688 ms。概览标记和原生桌面手测未验，这不是 P2-06 或 G2 的整卡通过。

## 结论

本批闭合两件事：

1. 正式视图的滚轮和滚动条拖拽能在写满有限容量的真实终端模型上浏览历史，顶部、底部、追加与淘汰的锚点都有视图级回归。
2. `ED 3`（`CSI 3 J`，shell `clear` 会发送）和 `RIS`（`ESC c`）清历史时，历史行数归零、显示偏移回到 0、世代前进、后续输出正常；行存储的释放与 Alacritty 真正执行清除的时刻一致。

返工只改清历史路径：删除了第一轮的第二套原始字节扫描器，改为让同一份 VTE `Processor` 通过一个只做转发的 `Handler` 调用 Alacritty。没有修改上游 crate、上游缓存、回滚容量上限或设置页；没有逐字节重绘，也没有接入概览标记和搜索匹配行。概览标记、搜索和命令跳转仍属于第 2 批 / P2-07。

## 返工前复现

用临时集成测试和带计数全局分配器在返工前的 `7bcc8063` 上复现了三个真实遗漏：

| 场景 | 返工前结果 | 原因 |
| --- | --- | --- |
| 历史 500 行后，同一写入 `CSI 3 J` + `CSI ?1049 h` | 历史 0，存活字节释放 **0** | `was_alternate != is_alternate` 提前返回，主屏被换出后再也没有 `truncate` 的机会 |
| 容量 100、历史 4 行，同一写入 `ESC c` + 5 行新输出 | 历史 4，世代 **0 → 0** | 第一轮只识别 `ED 3`；`RIS` 后的 `history_before > history_size` 不成立，新输出把行数填回同值 |
| 同步更新内 `CSI ?2026 h` + `CSI 3 J`，尚未收到 `CSI ?2026 l` | 世代 **0 → 1**，行存储提前 `truncate` | 第二套 `Parser` 不共享 `Processor` 的同步更新缓冲，看到的 clear 不是 Alacritty 真正执行的时刻 |

另一项边界是第一轮在整批 `write` 末尾再做一次 `truncate`。这会让“清后同批 prompt”在单次写入时释放约 2 MiB，而在逐字节或分段写入时被后续输出重新长出上游的 1000 行缓存，最终存活字节不同。PTY 分批不是语义，这种依赖写入边界的结果不能作为回归。

返工后的复现命令：

```sh
cargo test -p tterm-terminal --test history_clear \
  a_clear_before_the_alternate_screen_switch_is_kept_on_the_main_screen
cargo test -p tterm-terminal --test history_clear \
  a_reset_that_refills_the_same_history_still_advances_the_generation
cargo test -p tterm-terminal --test history_clear \
  a_synchronized_saved_line_clear_runs_at_the_end_of_the_update
cargo test -p tterm-terminal --test scrollback_memory -- --nocapture
```

返工前的三次探针使用临时测试和同一分配器完成，没有保留在产品树中；上表记录的是它们的实际输出。

## 生产改动

| 位置 | 改动 | 原因 |
| --- | --- | --- |
| `crates/tterm-terminal/src/history_release.rs` | 新增 `Handler` 包装层：原样转发 Alacritty 的 71 个 handler 方法，只拦截 `clear_screen(ClearMode::Saved)` 和 `reset_state`，在 Alacritty 调用返回后立即 `truncate` 当前活动 grid | 释放点与 Alacritty 执行点完全相同；无需第二套解析器、`mem::take` 或额外控制字符串缓存 |
| `crates/tterm-terminal/src/terminal.rs` | 写入时把同一个 `Processor` 指向包装层；用包装层报告的 `cleared_main` 决定世代推进；删除整批 write 末尾的第二次 `truncate` | 清除和释放绑定在命令执行上，不再绑定在 write 边界上 |
| `crates/tterm-terminal/src/clear_scanner.rs` | 删除 | 原始字节扫描器会被控制字符串里的字面 `3J` 误判，也无法共享同步更新、备用屏和 reset 的真实时序 |
| `crates/tterm-terminal/tests/history_clear.rs` | 新增 9 个公开 API 回归 | 单次写入、逐字节写入、跨界 CSI、单独 `ED 3`、清后输出、控制字符串、备用屏、reset、同步更新 |
| `crates/tterm-terminal/tests/scrollback_memory.rs` | 保留原释放测试并把测量点移到清除之后；新增 6 个分配器回归 | 真实存活内存；释放点、延迟清除、备用屏不误清主屏、同一字节流单次/分段结果相等 |
| `crates/tterm-terminal/src/history_release.rs` 的单元测试 | 对同一段广覆盖字节流比较“直接交给 Alacritty”和“经过包装层”的可见单元格、模式、偏移和总行数 | 证明转发没有改变终端行为 |

包装层的 71 个方法逐一对应固定版本 `vte 0.15.0` 的 `Handler` trait：2 个拦截方法加 69 个转发方法。上游依赖更新时需要重新核对 trait；没有新增 `pub use`，没有修改 `alacritty_terminal`。

### 清除语义

| 输入 | 行为 |
| --- | --- |
| 主屏单独 `CSI 3 J` | 只清 saved lines；当前可见行、光标和屏幕内容不变；历史归零、偏移归零、世代前进 |
| 主屏 `CSI 3 J` 后紧跟输出 | 清除点在输出之前真实执行；新输出从空历史开始；同一写入和逐字节写入结果相同 |
| 主屏 `CSI 3 J` 后紧跟 `CSI ?1049 h` | 在换屏前对主屏执行 `truncate`；随后备用屏不持有、也不误清主屏历史；返回主屏时原可见行仍在、历史仍为 0 |
| 备用屏 `CSI 3 J` | Alacritty 只清备用 grid；包装层只 `truncate` 备用 grid；隐藏的主屏历史和行存储不动 |
| `RIS`（`ESC c`） | 重置后主屏为活动 grid；包装层在 reset 返回后 `truncate` 主屏；即使新输出把历史填回同样行数，世代也前进 |
| 同步更新内的 `ED 3` / `RIS` | 包装层在同一个 `Processor` 内，字节先被同步缓冲；直到 `CSI ?2026 l` 或超时路径真正执行命令时才清历史并释放，提前写入不会释放或推进世代 |
| OSC/DCS/其他控制字符串中的字面 `3J` | 不经过 `clear_screen`，不清历史、不推进世代 |

### 内存结论的边界

清除发生时，`truncate` 会释放已不在逻辑历史里的行；这是本批的释放保证。若同一写入在清除之后还有新输出，Alacritty 会按自身的 1000 行 `MAX_CACHE_SIZE` 重新增长行存储，因此“清后同批 prompt 的最终存活字节”**不保证**低于清除前。第一轮报告里 `clear + 30 行 prompt` 后释放 2,075,744 字节的数字依赖 write 末尾的第二次 `truncate`，会产生 PTY 分批差异，返工后不再复现，也不再作为断言。

返工后的内存证据分为两类：

1. 不重填的清除、reset、清后紧接换备用屏、同步更新结束时，实测释放约 1.92 MiB；
2. 同一字节流（清历史 + 30 行 prompt）无论一次写入、逐字节写入还是 7 字节分段写入，最终存活字节完全相同（本次均为 `4,149,770`），历史、偏移和可见内容也相同。这个断言不依赖具体的 1.92 MiB，也不读取进程 RSS。

## 自动回归

### 模型语义与分批

命令：

```sh
cargo test -p tterm-terminal --test history_clear
cargo test -p tterm-terminal --lib
```

`history_clear` 的 9 个测试：

| 测试 | 断言 |
| --- | --- |
| `a_standalone_saved_line_clear_keeps_the_visible_rows` | 单独 `ED 3` 保留可见行；历史 0、偏移 0、世代前进 |
| `clear_and_fresh_output_match_whether_the_stream_is_one_write_or_split` | 清后 12 行输出；单次写入与逐字节写入的完整帧相等，旧行不残留 |
| `a_clear_split_inside_its_escape_sequence_runs_when_completed` | `CSI`、`3`、`J` 分三次写入；前两次不清，第三次清一次；单次/分段相等 |
| `a_clear_before_the_alternate_screen_switch_is_kept_on_the_main_screen` | `ED 3` + `?1049h` 后回主屏，主屏行保留、历史 0、世代前进 |
| `a_clear_on_the_alternate_screen_does_not_touch_the_main_history` | 备用屏 `ED 2`/`ED 3` 后回主屏，主屏行和历史都不变 |
| `text_that_looks_like_a_clear_inside_control_strings_is_ignored` | OSC 标题、DCS、OSC 8 和私有参数中的 `3J` 不清历史、不推进世代 |
| `a_synchronized_saved_line_clear_runs_at_the_end_of_the_update` | `BSU` 后的 `ED 3` 在 `ESU` 前不清历史、不推进世代；`ESU` 后清一次 |
| `a_synchronized_reset_runs_at_the_end_of_the_update` | 同步更新内的 `RIS` 同样延迟到 `ESU`，结束后历史 0、世代前进、屏幕清空 |
| `a_reset_that_refills_the_same_history_still_advances_the_generation` | 容量 100、两行屏幕；reset 后 5 行把历史填回同样的 4 行，世代仍前进；单次/逐字节相等 |

`crates/tterm-terminal/src/tests.rs` 中原有的清历史、清视口、备用屏和世代回归继续通过。`history_release::tests::forwarding_preserves_the_grid_for_a_broad_stream` 另外比较包装层前后的可见单元格、模式、偏移和总行数。

### 内存

命令：

```sh
cargo test -p tterm-terminal --test scrollback_memory -- --nocapture
```

1 个测试入口（`history_clear_scenarios_hold_under_one_isolated_measurement`），按顺序调用 7 个普通函数场景：

| 场景 | 断言 |
| --- | --- |
| `clearing_the_history_returns_the_row_storage_to_the_allocator` | 容量 500、2,000 行后单独清除；释放 > 400,000；后续输出正常 |
| `clearing_before_the_alternate_screen_switch_releases_the_main_history` | `CSI 3 J` + `CSI ?1049 h`；主屏行在换屏前释放 > 1,000,000 |
| `a_synchronized_clear_does_not_release_before_it_executes` | `BSU` 后的 `ED 3` 不提前释放；`ESU` 后释放 > 400,000 |
| `clearing_the_alternate_screen_does_not_release_the_main_history` | 备用屏清除释放 < 100,000；返回主屏后历史仍为 500 |
| `a_reset_returns_the_row_storage_to_the_allocator` | `RIS` 释放 > 400,000，历史 0，世代前进 |
| `a_synchronized_reset_releases_only_after_the_update_ends` | 同步更新内的 `RIS` 在 `ESU` 前不释放、不推进世代；结束后释放 > 400,000 |
| `the_same_stream_leaves_the_same_live_memory_when_split` | 同一“清除 + 30 行 prompt”字节流，单次写入、逐字节、7 字节分段的最终存活字节、历史、可见内容完全相等 |

本次证据行（默认并行参数、`--nocapture`，同一稳定阶段：写入返回且断言用 snapshot 已释放）：

```text
history-clear: capacity=500 rows=500 live_before=4149418 live_after=2229418 released=1920000
history-clear-alt: capacity=500 live_before=4149418 live_after=2229418 released=1920000
history-clear-sync: live_before=4149418 live_waiting=4149418 live_after=2229418 released=1920000
history-reset: live_before=4149418 live_after=2229610 released=1919808
history-reset-sync: live_before=4149418 live_after=2229610 released=1919808
history-clear-on-alt: live_before=4149418 live_after=4149418 released=0
history-clear-batch: one write live_before=4149770 live_after=4149770 released=0
history-clear-batch: single bytes live_before=4149770 live_after=4149770 released=0
history-clear-batch: seven-byte chunks live_before=4149770 live_after=4149770 released=0
```

这些是请求分配字节，不是 RSS；绝对值取决于单元格布局和平台。断言使用的是关系（释放阈值、同步前的非释放、备用屏的“不释放”）和同一平台同一稳定阶段的完全相等，不把 1,920,000 或 4,149,770 写成跨平台常量。整个二进制只有这一个 `#[test]`，测量期间没有 libtest 工作者；`checked_sub` 在差值为负时返回负值而不是溢出。

#### 测量口径修正（`9da7722c`）

协调者在 `8fe8ba44` 上用 `cargo test --workspace --test scrollback_memory -- --nocapture` 复跑时，`the_same_stream_leaves_the_same_live_memory_when_split` 的严格差值断言报出 `left=254 right=0`。这不是清除路径的回归：五个释放场景的释放量仍是 1,920,000 / 1,919,808 字节。失败的是测量口径。原有 7 个 `#[test]` 由 libtest 并行运行，进程内 `Mutex` 只把测试函数体串行化，锁不住 libtest 的线程启动、输出和收尾；这些分配会落入另一个场景的 `before` 与 `after` 之间。而同一字节流场景比较的是清除后的净释放：`truncate` 后的第一次滚动会重新预留 1,000 行缓存（`MAX_CACHE_SIZE`），净差值本来就接近 0，所以 254 字节的旁路分配就足以让两个差值不再相等。

修正只改变测量隔离，不改生产代码、采样点和任何断言：

- 7 个场景改为普通 helper，由唯一的 `#[test] history_clear_scenarios_hold_under_one_isolated_measurement` 按顺序调用；运行期间 libtest 不再启动并列工作者，计数器只包含本线程的分配。
- 保留每个场景原来的严格断言（释放阈值、同步前不释放、备用屏不释放、同一字节流完全相等），没有放宽阈值或改成不等式。
- 不使用 `--test-threads=1` 绕过；修正后的命令保持默认并行参数。
- 模块文档写明该二进制只能有一个 `#[test]`，再加一个就会重新引入并发噪声。

验证：用默认并行参数和 `--nocapture` 连续运行 20 次，全部通过；9 条证据行（含绝对 `live_before` / `live_after`）20 次完全相同。随后 `cargo test --workspace` 在正常参数下通过，`scrollback_memory` 从 7 个测试变为 1 个测试，其余套件计数不变。

### 正式视图

命令：

```sh
cargo test -p tterm-terminal-view --lib view::scrollback_tests
```

新增 `view/scrollback_tests.rs` 使用生产 `TerminalView`、正式滚轮监听和正式指针监听；输出全部是合成的 `line-NN`，模型容量分别为 6 和 30。

| 测试 | 断言 |
| --- | --- |
| `the_wheel_browses_a_full_finite_history_to_both_boundaries` | 写满后向上一屏、顶部钳制、向下一屏、底部钳制；可见首行逐行对应 |
| `output_while_scrolled_keeps_the_anchor_until_it_is_evicted` | 追加时锚点保持到偏移上限；锚点淘汰后视口逐行前移 |
| `dragging_the_scrollbar_reaches_the_history_bounds` | 轨道顶部、底部、中部；按下轨道请求重绘；可见首行同步变化 |
| `clearing_the_history_resets_offset_and_notifies_the_frame` | `ED 2`+`ED 3` 后历史 0、偏移 0、世代前进、屏清空、后续输出无旧行 |
| `clearing_the_viewport_or_alternate_screen_is_not_clearing_main_history` | `ED 2` 与备用屏清除都不当主屏清历史 |

## 全套检查

实际运行：

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
node scripts/perf/gpui-release-frame-budget.mjs
```

| 检查 | 结果 |
| --- | --- |
| 前端六项 | 通过；187 个测试文件、1367 个测试；`file-size: 1503 files within their limits` |
| `cargo fmt --all --check` | 通过 |
| 严格 Clippy | 通过，`-D warnings` |
| `cargo test --workspace` | 通过；`src-tauri` 73 + 1 ignored、`tterm-core` 1869 + 64 ignored、`tterm-gpui` 277 + 3 ignored、`tterm-terminal` lib 346、`history_clear` 9、`replay` 2、`scrollback_memory` 1（7 个场景）、`tterm-terminal-view` 159、`tterm-parity` 85、`tterm-platform` 14、`tterm-ui` 85；无失败 |
| release 帧与吞吐门 | 通过；30 后台会话前台 p95 **182 µs**（预算 16,667 µs），30 MiB 在 **162,696 µs** 内全部应用并确认（约 **184.39 MiB/s**，最低 60 MiB/s） |

`cargo test --workspace` 第一次并行运行时，`tterm-gpui` 的 `split_panes_keep_live_sessions_and_route_input_only_to_the_focused_view` 曾报一次 PTY 时序失败；该测试单独运行通过，随后两次全量运行（其中一次写入日志并核对汇总）也通过。没有把它当成代码通过证据，也没有修改该测试。

## 桌面手测边界

以下只能由原生桌面手测取得，本批没有替代证据：

- macOS / Windows / Linux 原生窗口里的真实触控板惯性、滚轮刻度、滚动条拖拽手感与视觉；
- 真实 SSH 会话里 `clear`、`Ctrl+L`、`reset`、vim/tmux/htop 的屏幕切换与回滚；
- 原生 DPI、字体、窗口缩放变化下的滚动条几何与闪烁；
- 真实窗口中的帧预算和内存峰值。

自动事件是 GPUI test app 里的生产监听器，不是桌面验收。本批也没有原始截图或视频。

## 遗留问题

- 概览标记、搜索全量匹配、命令跳转、淘汰时的 `CommandMarks::trim_scrollback` 连接点仍在第 2 批 / P2-07；本批没有构造占位数据，也没有修改这些模块。
- 清后同批输出会重新长出上游的有界行缓存；这是 Alacritty 的存储策略，不是本批引入的泄漏。需要进一步压缩“清后小历史”的常驻内存时，应单独设计容量/缓存策略，不能靠 `write` 末尾无条件 `truncate`。
- `history_release.rs` 的转发列表固定对应 `vte 0.15.0`。升级 Alacritty/VTE 后必须重新核对 Handler trait，否则新增的默认方法会在包装层静默变成 no-op。

## 工作流

本批在 `migrate/P2-06-scroll-memory` 的独立 worktree 中执行，只使用合成输出和隔离测试窗口。没有合并、没有推送、没有修改 `docs/gpui-migration/status.md`；由协调者复核后决定是否本地合并。
