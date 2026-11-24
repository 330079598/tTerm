# P2-07 第一批：保留历史全量搜索模型

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-07-search-model`，独立 worktree
- 起点：`2ba5e4c5`
- 代码提交：`4dbb62f5`
- 平台：macOS 27.2（26B123），arm64，Apple M4，36 GB；rustc 1.99.1；cargo 1.102.0；pnpm 11.25.0；Node 24.20.0
- 状态：**自动验收通过；只提交本地分支，未推送、未合并。** 本批只做模型、失效规则和性能基线，没有搜索栏、快捷键、高亮、概览搜索标记或上一个/下一个的界面。P2-07 与 G2 都不因本批通过。

## 结论

本批给生产终端模型加了一条完整的、可在 worker 上运行的搜索路径：

1. 模型按 P2-06 概览标记的保留历史坐标（0 是最旧保留行，最后可见行是 `history_size + screen_lines - 1`）复制文本；搜索结果是同一坐标下的全部匹配范围。
2. 查询支持正则、区分大小写和全词三个独立选项。正则引擎使用仓库已有的 `regress`（ECMAScript 语义，与前端 `@xterm/addon-search` 和关键词高亮一致）；字面量模式只转义查询，不改变正则模式的错误语义。
3. 扫描在纯数据上进行。模型每次只复制 `SEARCH_CHUNK_ROWS`（512）行并立即释放锁；取消标记在行和匹配之间生效，滚动世代变化会让部分结果变成 `Stale` 并被丢弃。
4. 历史淘汰、`ED 3`/`RIS`、备用屏切换和 resize 都推进 `scrollback_generation`。resize 原先不推进世代，本批补上；重排会移动行，即使历史长度不变。

本批没有把结果接到 `TerminalView::set_search_marks`，也没有伪造概览标记。接口稳定后再在下一批接入高亮、概览标记、上一个/下一个和结果计数。

## 生产改动

| 位置 | 改动 | 原因 |
| --- | --- | --- |
| `crates/tterm-terminal/src/search.rs` | `SearchOptions`、`SearchQuery`、`SearchPosition`、`SearchMatch`、`SearchResult`、`SearchOutcome`、`SearchCancellation`、`RetainedRows`、`SearchRow`、`SEARCH_CHUNK_ROWS`、`scan_chunks` | P2-07 的公开模型；保留 P0-09 的 `Search` 与 `find_next` 不变 |
| `.../src/search/query.rs` | 编译字面量或正则查询、大小写、全词、UTF-16 范围到单元格的映射 | 复用 `regress` 和 `keyword_highlight` 的字面量转义；全词使用前端固定的非单词字符集 |
| `.../src/search/rows.rs` | 在锁内把 Alacritty 网格行复制为 UTF-16 单位和单元格位置；跳过宽字符占位格，拼接折行，裁掉非折行行尾空白 | 扫描只读纯数据；坐标和折行规则集中在一处 |
| `.../src/search/scan.rs` | 逻辑行拼接、逐匹配取消、结果收集 | 跨块的折行不丢失；取消和 stale 不需要操作模型 |
| `.../src/terminal/search_ops.rs` | `retained_rows`、`search_retained`；`find_next` 从 `terminal.rs` 移入 | 每次只持锁复制一块；搜索入口与模型主体分开 |
| `.../src/terminal/selection_text.rs` | `selection_text` 与 `active_selection_range` 从 `terminal.rs` 移入 | 拆分超长文件；行为不变 |
| `.../src/terminal.rs` | resize 推进 `scrollback_generation`；拆出两个子模块 | 重排后旧行号必须失效；文件保持小于 800 行 |
| `.../src/keyword_highlight.rs` | 私有 `escape` 改为 `pub(crate) escape_pattern` | 搜索的字面量模式复用同一份 JS 正则转义 |
| `.../tests/search_model.rs` | 8 个生产模型生命周期测试 | 坐标、输出增长、淘汰、清除、备用屏和 resize |
| `.../tests/search_benchmark.rs` | `#[ignore]` 的 release 测量 | 10 万行的耗时、分配、匹配数和每块持锁时间的证据 |

`crates/tterm-terminal/src/terminal.rs` 从 787 行降到 735 行；`search_ops.rs` 62 行，`selection_text.rs` 49 行。没有修改概览绘制、滚动条、视口、快捷键、搜索结果界面或任何性能门槛。

## 接口与坐标

### 查询

```rust
pub struct SearchOptions {
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

pub struct SearchQuery { /* compiled once */ }

impl SearchQuery {
    pub fn new(query: &str, options: SearchOptions) -> Result<Self, SearchError>;
    pub fn options(&self) -> SearchOptions;
    pub fn is_empty(&self) -> bool;
}
```

- 空查询是合法输入，返回 `Complete` 且 0 个匹配；模型不会为空正则执行 10 万行扫描。
- `regex: true` 时非法模式返回 `SearchError`，`Display` 为 `invalid search pattern: ...`，可交给界面显示；不会退化成字面量。
- `regex: false` 时用仓库已有的 JS 正则转义函数处理查询，`.`、`[` 等按普通字符匹配。
- 正则匹配到空串（如 `^`、`a*`）是合法的，但不会产生 0 宽度的搜索结果；这与前端搜索忽略空匹配的行为一致。测试覆盖没有死循环。

### 结果

```rust
pub struct SearchPosition { pub line: i64, pub column: usize }
pub struct SearchMatch { pub start: SearchPosition, pub end: SearchPosition }

impl SearchResult {
    pub fn generation(&self) -> u64;
    pub fn total_lines(&self) -> usize;
    pub fn matches(&self) -> &[SearchMatch];
    pub fn match_count(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn index_from(&self, origin: SearchPosition, direction: SearchDirection) -> Option<usize>;
}
```

- `line` 是保留历史坐标：0 是最旧的保留行。测试直接和 `Snapshot::command_marks()` 的 `prompt_line` / `output_line` 比较，并验证最后可见行是 `history_size + screen_row`。
- `end` 是最后一个单元格，包含该单元格；宽字符按两列计算。折行匹配可以跨物理行，普通换行不能。
- `matches` 按阅读顺序排列。`index_from` 用二分查找给出下一个（起点不早于 origin）或上一个（起点不晚于 origin）结果的索引；调用方在活动结果上自行加一/减一。
- `generation` 是扫描开始时的 `scrollback_generation`。绘制前应与当前快照的世代比较；不同则丢弃结果。

### 复制与扫描

```rust
pub const SEARCH_CHUNK_ROWS: usize = 512;

impl Terminal {
    pub fn retained_rows(&self, start: usize, max_rows: usize) -> RetainedRows;
    pub fn search_retained(
        &self,
        query: &SearchQuery,
        cancellation: &SearchCancellation,
    ) -> SearchOutcome;
}
```

`retained_rows` 在锁内最多复制 512 行；`rows[0]` 的保留行号是 `start`，`total_lines` 是复制时的总保留行数，`generation` 是同一时刻的世代。`search_retained` 只通过这个入口取数据，复制完就释放锁，再在调用线程上扫描。

`SearchOutcome` 三种结果：

| 值 | 含义 |
| --- | --- |
| `Complete(SearchResult)` | 已覆盖扫描开始时快照的每一条保留行，且最后复查过世代 |
| `Cancelled` | 调用方在行或匹配之间要求停止 |
| `Stale` | 扫描中世代变化，部分匹配已丢弃，调用方应重新扫描 |

### 文本规则

- 逻辑行以 `WRAPLINE` 拼接；硬换行分开扫描，因此 `abcde` + 换行 + `fghij` 不会组成 `defg`。
- 宽字符的 `WIDE_CHAR_SPACER` 和行尾的 `LEADING_WIDE_CHAR_SPACER` 不产生文本；宽字符覆盖两列，组合字符跟随其基字符的单元格。
- 非折行行的右侧空白被裁掉；折行行保留整行宽度，因为它还要和下一行拼接。
- UTF-16 单位索引与前端 JavaScript 一致；`regress` 的 `find_from_ucs2` 用于保留 ECMAScript 的索引和 lookaround 语义。

## 失效规则

| 事件 | `scrollback_generation` | 旧结果 |
| --- | --- | --- |
| 普通输出增加历史、未淘汰 | 不变 | 保留坐标不变；新行不在旧结果中，需要重新扫描才能看到 |
| 历史满容量后淘汰最旧行 | 增加 | 丢弃；重新扫描后行号整体前移 |
| `ED 3` / `RIS` 清历史 | 增加 | 丢弃 |
| 进入或离开备用屏 | 增加 | 丢弃；备用屏有自己的 0 起始坐标 |
| resize / 重排 | 增加（本批新增） | 丢弃；重新扫描按重排后的行计算 |
| `set_scrollback` 缩容并实际裁掉历史 | 增加（沿用既有逻辑） | 丢弃 |
| 滚动 / 显示偏移 | 不变 | 结果不变；滚动不改变保留坐标 |
| 扫描中取消 | 不变 | 返回 `Cancelled`，不返回部分结果 |

扫描开始时固定总行数，因此持续输出不会让一次扫描无限延长。新输出的新行需要下一次扫描；输出覆盖可见单元格（例如进度条重写同一行）不会被 `scrollback_generation` 检测到，下一批的界面需要在输出到达时做去抖重扫。这个边界在“偏差与遗留”中再次记录。

## 性能基线

### 夹具

- 100,000 行输出，每行 100 个 ASCII 列，屏幕 24 行，回滚容量 100,000。
- 写完后保留行 100,001（99,977 历史行 + 24 屏幕行），搜索覆盖全部保留行。
- 每行有一个 `needle`；每 5 行改成 `needles`；每 10 行再加一个大写 `NEEDLE`。因此四种查询的匹配数不同，能区分大小写、全词和子串行为。
- 查询编译不计入耗时和分配；计时和分配计数只包住 `search_retained`。

### 硬件与命令

- Apple M4（10 CPU），36 GB，macOS 27.2（26B123），arm64，rustc 1.99.1。
- 命令：

```sh
cargo test -p tterm-terminal --release --test search_benchmark \
    -- --ignored --exact --nocapture --test-threads=1
```

- 机器不是完全空闲：测量时 `uptime` 的 1 分钟负载约 11。下表是 7 次独立运行的**中位数**，括号内是最小–最大；精确复现应在空闲机器上重跑。
- 分配数是 allocator 调用次数；字节数是请求的累计字节（`realloc` 按新大小计），不是峰值常驻。峰值受一块 512 行和匹配向量限制，约几 MiB。

| 查询 | 选项 | 匹配数 | 中位耗时 | 范围 | 分配次数 | 分配字节 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 普通文本 `needle` | 默认（大小写不敏感） | 110,000 | 69,188 µs | 64,908–110,862 µs | 300,217 | 78,789,808 |
| 正则 `alpha \w+ beta` | `regex` | 100,000 | 67,503 µs | 61,207–102,093 µs | 400,217 | 94,789,808 |
| 大小写 `NEEDLE` | `case_sensitive` | 10,000 | 68,305 µs | 62,662–74,830 µs | 300,214 | 71,449,776 |
| 全词 `needle` | `whole_word` | 90,000 | 69,308 µs | 58,617–126,615 µs | 300,217 | 78,789,808 |

### 持锁时间

同一次夹具另外测量了 `retained_rows`：196 个复制块，总复制时间中位数 51.45 ms，最慢单块 624 µs（7 次范围 492–1,314 µs）。这是锁内复制加行分配的代理值，扫描和正则匹配都在锁外。100,001 行没有被一次性复制，也没有在扫描期间持有模型锁。

### 与门槛的关系

没有新增或修改性能门槛。本批的搜索数字是 P2-07 的基线证据，不是 CI 门槛；协调者在空闲机器上复跑后再决定是否合并。

## 自动回归

### 模型单元测试

```sh
cargo test -p tterm-terminal --lib search
```

新增 21 项，过滤词 `search` 共匹配 27 项通过。覆盖：

- 空查询、无结果、非法正则不 panic 且返回可显示错误；
- 字面量模式不把正则元字符当模式，正则模式保留 lookbehind；
- 大小写选项和 Unicode `i` 折叠；
- 全词的前端非单词字符集、下划线/CJK 边界行为；
- Unicode 宽字符、组合字符、折行跨行匹配，以及行尾放不下的宽字符折到下一行；
- 硬换行不拼接；
- 结果排序与 `index_from` 上一个/下一个；
- 0 宽正则匹配被跳过且不循环；
- 块边界、世代变化的 `Stale`、取消、空查询不复制缓冲区；
- 长缓冲区分块扫描不漏匹配；
- 搜索类型可跨线程移动。

### 生产模型生命周期

```sh
cargo test -p tterm-terminal --test search_model
```

8 项通过：

| 测试 | 断言 |
| --- | --- |
| `search_lines_use_the_same_coordinate_as_overview_command_marks` | 提示符/输出匹配行与 P2-06 命令标记相同；最后可见行等于 `history_size + screen_row` |
| `output_growth_keeps_existing_lines_and_adds_new_ones` | 增加历史不推进世代、不改旧行号；新行追加在后 |
| `eviction_advances_the_generation_and_drops_old_matches` | 满容量淘汰推进世代，被淘汰的匹配不再出现 |
| `clearing_the_history_advances_the_generation` | `ED 2`/`ED 3` 后旧结果失效 |
| `alternate_screen_switches_invalidate_and_rescan_the_active_grid` | 进出备用屏都推进世代；备用屏扫描自己的网格，回主屏后主屏结果恢复 |
| `resize_invalidates_and_rescans_reflowed_rows` | resize 推进世代；长行重排后标记行号后移，重新扫描正确 |
| `scrolling_does_not_move_a_match` | `Scroll::Top`/`Bottom` 不改变匹配 |
| `a_cancelled_search_returns_cancelled_without_a_result` | 预取消的查询返回 `Cancelled` |

### 全套检查

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 全部测试二进制通过；`tterm-terminal` lib 369 项、`tterm-terminal-view` 172 项、`tterm-core` 1869 项；新的搜索 benchmark 为 `#[ignore]`，默认套件只计数不执行 |
| `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` | 通过；vitest 187 个文件、1367 项，脚本测试全部通过；1510 个文件在大小限制内 |
| `node scripts/perf/gpui-release-frame-budget.mjs` | 通过：foreground p95 421.667 µs，30-background p50 119.292 µs / p95 421.667 µs；background throughput 180.11 MiB/s，500 ms 截止内完成；两项独立测量都通过 |

release 门槛的 commit 行在运行时尚显示 `2ba5e4c5`，因为搜索改动当时还未提交；门槛测量的是同一个工作树的 release 代码。门槛的负载、步进、采样窗口、16.667 ms p95 预算、500 ms 截止和 30 MiB 总量都没有修改。

## 偏差与遗留

1. **本批没有界面。** 没有搜索栏、快捷键、结果高亮、当前结果颜色、概览搜索标记、上一个/下一个或结果计数显示。`TerminalView::set_search_marks` 仍默认空集合。这些属于下一批。
2. **空查询是完整的空结果。** 不把空串当非法正则，也不为它复制 10 万行。界面应把空输入显示为 0 个结果。
3. **全词语义来自前端而不是 `\b`。** 使用 `@xterm/addon-search` 0.16 的固定非单词字符集：空格和 ASCII 标点算边界，下划线和 CJK 字母不算。因此 `_needle` 和 `中文` 中的 `中` 不是全词匹配，而 `needle-`、`needle.` 是。这一点与 Tauri 版一致，但与 Rust 正则的 `\b` 不同。
4. **`regress` 的已知大小写折叠差异。** 与 P1-11b 登记的一致：`i` 标志下 `/s/i` 会匹配 U+017F（ſ），JS 不匹配。搜索沿用同一引擎，没有另写规避实现。
5. **输出增长与可见行覆盖。** 普通追加输出推进历史长度但不推进 `scrollback_generation`，已有匹配的保留行号仍然正确；新行需要重新扫描才会出现。扫描开始时固定总行数，避免持续输出让扫描无法结束。输出重写可见单元格本身不会被世代捕获；下一批的搜索界面需要在输出到达时去抖重扫，或在绘制前用快照世代/内容校验丢弃过期结果。本批没有把这一策略写进界面。
6. **resize 的行为变化。** `resize` 现在无条件推进 `scrollback_generation`，即使尺寸相同也会让视图的行缓存重建一次。这样做避免了“重排后行号仍被当作有效”的错误；`terminal-view` 的 172 项测试仍全部通过。
7. **备用屏扫描的是备用网格。** 进入备用屏后，搜索结果是备用屏的可见文本（保留坐标从 0 开始），不是隐藏的主屏历史；回到主屏后世代再变一次，主屏结果重新扫描。下一批如果决定备用屏完全不显示搜索结果，应只改界面，不改模型坐标。
8. **性能数字有负载噪声。** 测量时机器负载约 11，7 次中位数为 67–69 ms，范围到 126 ms；最慢复制块 624 µs。没有调低/调高任何门槛，也没有把本批数字写成 CI 门槛。协调者应在空闲机器上复跑。
9. **没有读取真实连接、密钥或正式数据。** 夹具是进程内的合成 10 万行输出；没有启动 SSH、PTY 或桌面窗口。
10. **不是 P2-07 或 G2 通过。** 本批只完成模型、失效规则、测试和性能基线；搜索的界面、视觉对比和真实桌面验收仍未完成。
