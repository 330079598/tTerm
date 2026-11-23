# P2-06 续批第 1 批：滚动与回滚内存

- 日期：2026-11-23
- 分支 / worktree：`migrate/P2-06-scroll-memory`，独立 worktree
- 起点：`03ab675d`
- 平台：macOS 27.2（26B123），arm64；rustc 1.99.1
- 状态：**本批代码与自动回归完成，未合并、未推送、未修改总状态表。** 这不是 P2-06 或 G2 的整卡通过。

## 结论

本批只闭合两件事：

1. 正式视图的滚轮和滚动条拖拽能在写满有限容量的真实终端模型上浏览历史，顶部、底部、追加与淘汰的锚点都有视图级回归。
2. 走现有的 `ED 3`（`CSI 3 J`，shell `clear` 会发送）清历史路径，历史行数归零、显示偏移回到 0、世代前进、后续输出正常；同时补上了“行数归零但 Alacritty 仍保留小容量行存储”的缺口。

概览标记和搜索匹配行没有接入，也没有构造占位数据。它们仍属于第 2 批 / P2-07，登记见文末。

## 起点检查与缺口

### 上游行存储

固定版本 `alacritty_terminal 0.26.0` 的 `Grid::clear_history` 只执行 `raw.shrink_lines(history)` 并重置 `display_offset`。`Storage::shrink_lines` 降低逻辑 `len`，只有 `inner.len() > len + 1000` 时才 `truncate` 真正释放。因此：

- 大容量清除会自然释放大部分行；
- 小容量（不足 1000 行缓存阈值）清除后，历史长度为 0，但行存储仍留在分配器里。

用带计数全局分配器的临时探针在修改生产代码前测得：

| 容量 | 写入 | 清除后历史 | 存活字节变化 | 说明 |
| --- | --- | --- | --- | --- |
| 500 | 2,000 行 | 500 → 0 | 4,148,612 → 4,148,612（释放 0） | 行数归零，内存未释放 |
| 5,000 | 20,000 行 | 5,000 → 0 | 12,057,988 → 2,457,988（释放 9,600,000） | 超过上游 1000 行缓存后自然释放 |

这证明“历史行数为 0”不能当作“内存已释放”；本机进程 RSS 不作为契约。

### 视图缓存

行缓存以内容哈希（字符、属性、颜色、样式世代）为键，容量有界；世代变化只丢弃上一帧复用。清历史后旧行不会重新上屏，但缓存不强制清空。回归验证的是“不发布旧内容”，不是“缓存必须归零”。

## 生产改动

| 位置 | 改动 | 原因 |
| --- | --- | --- |
| `crates/tterm-terminal/src/clear_scanner.rs` | 新增最小 VTE `Perform` 扫描器，跨写入识别 `ED 3` | Alacritty 不向模型暴露清历史事件；shell 的 `clear` 可能和 prompt 在同一批写入 |
| `crates/tterm-terminal/src/terminal.rs` | 收到 `ED 3` 或观察到历史长度下降时，用 `Grid::truncate` 释放已清除行；清历史推进 `scrollback_generation` | 关闭小容量“行数归零但内存保留”的缺口，并让绝对行缓存/后续标记有失效信号 |
| `crates/tterm-terminal/src/snapshot.rs` | 世代文档补充“清历史也推进” | 与实现一致 |
| `crates/tterm-terminal-view/src/view/pointer.rs` | 按下滚动条轨道并改变偏移后请求重绘 | 原来只有拖动中的移动会重绘，单击轨道可能等到下一次无关事件才更新 |

没有修改 `alacritty_terminal` 源码、全局缓存、上游依赖或设置页。

## 自动回归

### 模型与内存

命令：

```sh
cargo test -p tterm-terminal --lib
cargo test -p tterm-terminal --test scrollback_memory -- --nocapture --test-threads=1
```

覆盖：

| 测试 | 断言 |
| --- | --- |
| `tests::clearing_history_then_output_starts_a_consistent_generation` | 清除后历史 0、偏移 0、世代前进；后续输出可见 |
| `tests::clear_and_fresh_output_in_one_write_replaces_the_history` | `ED 3` 与 12 行新输出同一批到达时仍是新历史，不残留旧行 |
| `tests::clearing_the_viewport_keeps_the_main_history` | `ED 2` 只清当前屏，主屏历史保留 |
| `tests::clearing_the_alternate_screen_keeps_the_main_history` | 备用屏 `ED 2`/`ED 3` 不清主屏历史 |
| `clear_scanner::tests::*` | `ED 3`、`03J`、跨写入、OSC 字符串、私有参数 |
| `scrollback_memory::clearing_the_history_returns_the_row_storage_to_the_allocator` | 容量 500 清除并跟随 30 行 prompt 输出后，存活字节至少下降 400,000；后续输出正常 |

本次内存证据行：

```text
history-clear: capacity=500 rows=500 live_before=4318226 live_after=2242482 released=2075744
```

这个断言使用请求分配字节，不读取 RSS；不同平台的具体数值可以不同，但“清除前远大于清除后”的关系必须成立。

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

### 全套检查

```text
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
node scripts/perf/gpui-release-frame-budget.mjs
```

| 检查 | 结果 |
| --- | --- |
| 前端六项 | 通过；187 个测试文件、1367 个测试；`file-size: 1503 files within their limits` |
| `cargo fmt --all --check` | 通过 |
| 严格 Clippy | 通过 |
| `cargo test --workspace` | 通过；终端模型 349、视图 159、内存回归 1，其余 crate 无失败 |
| release 帧与吞吐门 | 通过；30 后台会话前台 p95 **285 µs**（预算 16,667 µs），30 MiB 在 **155,280 µs** 内全部应用并确认（约 **193.20 MiB/s**，最低 60 MiB/s） |

## 桌面手测边界

以下只能由原生桌面手测取得，本批没有替代证据：

- macOS / Windows / Linux 原生窗口里的真实触控板惯性、滚轮刻度、滚动条拖拽手感与视觉；
- 真实 SSH 会话里 `clear`、`Ctrl+L`、vim/tmux/htop 的屏幕切换与回滚；
- 原生 DPI、字体、窗口缩放变化下的滚动条几何与闪烁；
- 真实窗口中的帧预算和内存峰值。

自动事件是 GPUI test app 里的生产监听器，不是桌面验收。本批也没有原始截图或视频。

## 第 2 批登记：概览标记

已有生产者：

- `crates/tterm-terminal/src/command_mark.rs`、`command_marks.rs`：OSC 133/633 解析、prompt/output/done 条目、`trim_scrollback`、`dispose_markers`、`last_output_range`。
- `crates/tterm-core/src/command_tracking.rs`：把标记接到命令计时、通知和命令历史。
- `crates/tterm-core/src/remote_shell_integration.rs`、`shell_integration.rs`、`windows_shell.rs`：本地/远程 shell 发送 OSC 133/633。
- `crates/tterm-gpui/src/terminal_demo/session/reports.rs`：只消费 OSC 133 做注意力报告，不是标记存储。
- `crates/tterm-terminal/src/search.rs`：已有单个 `find_next` 匹配，没有全量匹配位置。
- 前端参照：`src/components/TerminalTab/commandMarks.ts`、`src/lib/terminalSearch*`、`xterm` 的 search addon。

需要第 2 批 / P2-07 接的连接点：

1. `Snapshot` 目前只有可见行、偏移、历史和世代，没有标记/匹配位置；需要一条按世代失效的侧通道，或扩展快照。
2. `view/scrollbar.rs` 目前只算轨道和滑块；`view/frame.rs` 只画这两块。概览标记需要把真实位置映射到滑块/轨道并处理高密度合并。
3. `CommandMarkViewport` 还没有由原生视图实现；跳转、复制命令输出和 prompt 状态仍只在 core 测试里。
4. 淘汰时没有调用 `CommandMarks::trim_scrollback` 的原生连接点。Alacritty 不公开准确淘汰行数，P0-12 也刻意没有暴露近似值；第 2 批要用世代 + 绝对行重定位，或接受确定性失效，不能猜行数。
5. 搜索全量匹配需要 P2-07 的后台扫描和取消；本批不实现搜索框，也不把 `find_next` 的单次结果伪装成全部匹配。
6. 清除历史后标记应全部失效；`dispose_markers` 已有，等第 2 批接线。

## 工作流

本批在 `migrate/P2-06-scroll-memory` 的独立 worktree 中执行，只使用合成输出和隔离测试窗口。没有合并、没有推送、没有修改 `docs/gpui-migration/status.md`；由协调者复核后决定是否本地合并。
