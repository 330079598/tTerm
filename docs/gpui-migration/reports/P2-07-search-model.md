# P2-07 第一批：保留历史全量搜索模型

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-07-search-model`，独立 worktree
- 起点：`2ba5e4c5`
- 平台：macOS 27.2（26B123），arm64，Apple M4，36 GB；rustc 1.99.1；cargo 1.102.0；pnpm 11.25.0；Node 24.20.0
- 状态：**自动验收通过；只提交本地分支，未推送、未合并。** 本批只做模型、失效规则和性能基线，没有搜索栏、快捷键、高亮、概览搜索标记或上一个/下一个的界面。P2-07 与 G2 都不因本批通过。

## 结论

本批给生产终端模型加了一条完整的、可在 worker 上运行的搜索路径：保留历史文本使用 P2-06 概览标记的坐标；查询支持正则、区分大小写和全词；扫描只复制有界块并在锁外进行；取消和滚动世代校验都有了；10 万行夹具的耗时、分配、匹配数和每块持锁时间已经记录。搜索结果的界面、高亮、概览标记和结果导航留给下一批。

## 改动

### 提交

| 提交 | 摘要 |
| --- | --- |
| `4dbb62f5` | `:sparkles:` 保留历史全量搜索模型、失效规则、测试、release 测量 |
| `9bb403bb` | `:memo:` 本报告 |
| `ee996817` | `:memo:` 登记 `@xterm/addon-search` 的全词边界集合 |
| `c8bbd604` | `:white_check_mark:` 行尾放不下的宽字符折行测试与报告计数 |

### 主要文件

| 位置 | 职责 |
| --- | --- |
| `crates/tterm-terminal/src/search.rs` | `SearchOptions`、`SearchQuery`、`SearchPosition`、`SearchMatch`、`SearchResult`、`SearchOutcome`、`SearchCancellation`、`RetainedRows`、`SearchRow`、`SEARCH_CHUNK_ROWS`；保留 P0-09 的 `Search`/`find_next` |
| `.../src/search/query.rs` | 编译字面量或正则查询、大小写与全词、UTF-16 范围到单元格的映射 |
| `.../src/search/rows.rs` | 在锁内把 Alacritty 网格行复制为 UTF-16 单位和单元格位置；跳过宽字符占位格，保留折行，裁掉硬行行尾空白 |
| `.../src/search/scan.rs` | 逻辑行拼接、逐个匹配检查取消、结果收集 |
| `.../src/terminal/search_ops.rs` | `retained_rows`、`search_retained`；`find_next` 移入该模块 |
| `.../src/terminal/selection_text.rs` | `selection_text` 与 `active_selection_range` 从 `terminal.rs` 移出，行为不变 |
| `.../src/terminal.rs` | resize 推进 `scrollback_generation`；拆出两个子模块，787 行降到 735 行 |
| `.../src/keyword_highlight.rs` | 私有 `escape` 改为 `pub(crate) escape_pattern`，字面量搜索复用同一份 JS 正则转义 |
| `.../tests/search_model.rs` | 8 个生产模型生命周期测试 |
| `.../tests/search_benchmark.rs` | `#[ignore]` 的 release 测量，含分配计数器和持锁复制计时 |

没有修改概览绘制、滚动条、视口、快捷键、搜索结果界面或任何性能门槛。

### 接口与坐标

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

- `SearchPosition.line` 是保留历史坐标：0 是最旧的保留行，最后可见行是 `history_size + screen_lines - 1`。测试直接和 `Snapshot::command_marks()` 的 `prompt_line` / `output_line` 比较。
- `SearchMatch.end` 是最后一个单元格（包含）；宽字符覆盖两列，折行匹配可以跨物理行。
- `matches` 按阅读顺序排列。`index_from` 用二分查找给出下一个（起点不早于 origin）或上一个（起点不晚于 origin）结果的索引；调用方在活动结果上自行加一/减一。
- `generation` 是扫描开始时的 `scrollback_generation`。绘制前应与当前快照的世代比较，不同则丢弃。
- `retained_rows` 在锁内最多复制 512 行；`search_retained` 只通过这个入口取数据，复制完就释放锁，再在调用线程上扫描。`SearchOutcome` 为 `Complete`、`Cancelled` 或 `Stale`。

### 文本与查询规则

- 逻辑行以 `WRAPLINE` 拼接；硬换行分开扫描，因此 `abcde` + 换行 + `fghij` 不会组成 `defg`。
- 宽字符的 `WIDE_CHAR_SPACER` 和行尾的 `LEADING_WIDE_CHAR_SPACER` 不产生文本；宽字符覆盖两列，组合字符跟随其基字符的单元格。
- 非折行行的右侧空白被裁掉；折行行保留整行宽度，因为它还要和下一行拼接。
- 空查询合法，返回 `Complete` 和 0 个匹配，不复制缓冲区、不执行空正则。
- `regex: true` 时非法模式返回 `SearchError`，`Display` 为 `invalid search pattern: ...`，可交给界面显示；不会退化成字面量。`regex: false` 时用仓库已有的 JS 正则转义函数处理查询。
- 正则匹配到空串（如 `^`、`a*`）是合法的，但不产生 0 宽度结果；与前端搜索忽略空匹配的行为一致，测试确认不会死循环。
- 引擎是 `regress`，与前端 `@xterm/addon-search` 和关键词高亮使用同一套 ECMAScript 语义（`\w`、`\s`、`\d`、lookaround、`i`）。全词使用前端固定的非单词字符集，而不是 `\b`。

### 失效规则

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

扫描开始时固定总行数，因此持续输出不会让一次扫描无限延长。新输出的新行需要下一次扫描；输出覆盖可见单元格（例如进度条重写同一行）不会被 `scrollback_generation` 检测到，下一批的界面需要在输出到达时做去抖重扫。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 从生产终端快照生成稳定的保留历史文本视图，行坐标与 P2-06 概览标记一致；历史淘汰、清除、备用屏和 resize 后旧结果必须失效或可重新扫描。 | 通过 | `search_lines_use_the_same_coordinate_as_overview_command_marks` 直接比较 `Snapshot::command_marks()`；`eviction_advances_the_generation_and_drops_old_matches`、`clearing_the_history_advances_the_generation`、`alternate_screen_switches_invalidate_and_rescan_the_active_grid`、`resize_invalidates_and_rescans_reflowed_rows` |
| 支持正则、大小写和全词三个选项，复用仓库已有 regex 语义；非法正则返回可显示错误，不 panic、不把查询当普通文本偷偷改写。 | 通过 | `SearchOptions` 三个独立字段；`literal_mode_escapes_regex_syntax_and_regex_mode_does_not`、`case_sensitivity_is_an_explicit_option`、`whole_word_uses_the_frontends_non_word_characters`、`an_invalid_regular_expression_is_an_error_not_literal_text`；引擎为 `regress`，字面量转义复用 `keyword_highlight::escape_pattern` |
| 返回全部匹配的绝对行/范围、结果计数和当前结果索引所需的数据；相邻行/折行匹配要有明确测试。 | 通过 | `SearchResult::{matches, match_count, index_from}`；`results_are_ordered_and_index_the_next_and_previous_match`、`adjacent_hard_lines_do_not_form_a_wrapped_line`、`a_match_can_span_wrapped_rows`、`a_wide_character_that_wraps_keeps_its_place` |
| 扫描放到 worker 可用的纯数据接口；模型锁不能在 10 万行扫描期间长时间占用。先提供可取消/generation 校验的 API，UI 在下一批接入。 | 通过 | `search_retained` 可跨线程调用；`SEARCH_CHUNK_ROWS = 512`；`SearchCancellation`；世代变化返回 `Stale`；benchmark 中 196 块、最慢复制块中位数 624 µs；本批没有接入 UI |
| 使用隔离终端合成 100,000 行，测量普通文本、正则、大小写和全词查询的耗时与分配，报告硬件、行宽、匹配数和命令；不修改性能门槛。测试覆盖空查询、无结果、非法正则、Unicode/中文、折行、输出持续增加、历史淘汰、清除、备用屏和 resize。 | 通过 | 见下面“性能测量”；测试命令和覆盖表见 `## 检查`；没有修改 `scripts/perf/` 或任何阈值 |
| 本批不绘制搜索框，也不伪造 P2-06 概览搜索标记；只在结果接口稳定且 generation 正确后，下一批接入高亮、概览标记、上一个/下一个和结果计数。 | 通过 | `git diff` 不包含 `tterm-terminal-view`；`set_search_marks` 仍默认空集合；报告结论明确下一批范围 |
| 使用独立 worktree，由 Pi + DeepSeek V4.1 Flash 实现；不读取真实连接、密钥或正式数据，不 push/merge。 | 通过 | 工作在 `/Users/stone/Code/rust/tTerm-P2-07-search-model` 独立 worktree；夹具是进程内合成输出；没有启动 SSH、PTY 或桌面窗口；`git log` 只在本地分支，未推送、未合并 |

### 性能测量

#### 夹具

- 100,000 行输出，每行 100 个 ASCII 列，屏幕 24 行，回滚容量 100,000。
- 写完后保留行 100,001（99,977 历史行 + 24 屏幕行），搜索覆盖全部保留行。
- 每行有一个 `needle`；每第 5 行改成 `needles`；每第 10 行再加一个大写 `NEEDLE`。四种查询的匹配数因此不同，能区分大小写、全词和子串行为。
- 查询编译不计入耗时和分配；计时和分配计数只包住 `search_retained`。

#### 硬件与命令

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

#### 持锁时间

同一次夹具另外测量了 `retained_rows`：196 个复制块，总复制时间中位数 51.45 ms，最慢单块 624 µs（7 次范围 492–1,314 µs）。这是锁内复制加行分配的代理值，扫描和正则匹配都在锁外。100,001 行没有被一次性复制，也没有在扫描期间持有模型锁。

没有新增或修改性能门槛；这些数字是 P2-07 的基线证据，不是 CI 门槛。

## 检查

### 搜索专项

```sh
cargo test -p tterm-terminal --lib search
cargo test -p tterm-terminal --test search_model
```

- 新增 21 项 `search::tests`，过滤词 `search` 共匹配 27 项通过；`search_model` 8 项通过。
- 覆盖：空查询、无结果、非法正则、Unicode/大小写折叠、CJK 全词边界、宽字符、组合字符、折行跨行、行尾宽字符折行、硬换行不拼接、结果排序与 `index_from`、0 宽正则、块边界、世代变化、取消、长缓冲区分块、类型跨线程。完整列表见 `search/tests.rs` 和 `tests/search_model.rs`。

### 全套检查

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 全部测试二进制通过；`tterm-terminal` lib 369 项、`tterm-terminal-view` 172 项、`tterm-core` 1869 项；新的搜索 benchmark 为 `#[ignore]`，默认套件只计数不执行 |
| `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` | 通过；vitest 187 个文件、1367 项，脚本测试全部通过；1510 个文件在大小限制内 |
| `node scripts/perf/gpui-release-frame-budget.mjs` | 通过：foreground p95 421.667 µs；30-background p50 119.292 µs / p95 421.667 µs；background throughput 180.11 MiB/s，500 ms 截止内完成；两项独立测量都通过 |

release 门槛的 commit 行在运行时尚显示 `2ba5e4c5`，因为搜索改动当时还未提交；门槛测量的是同一个工作树的 release 代码。门槛的负载、步进、采样窗口、16.667 ms p95 预算、500 ms 截止和 30 MiB 总量都没有修改。门槛只在一次运行中通过，协调者仍应在空闲机器上复跑。

## 偏差

1. **空查询是完整的空结果。** 不把空串当非法正则，也不为它复制 10 万行。界面应把空输入显示为 0 个结果。
2. **全词语义来自前端而不是 `\b`。** 使用 `@xterm/addon-search` 0.16 的固定非单词字符集：空格和 ASCII 标点算边界，下划线和 CJK 字母不算。因此 `_needle` 和 `中文` 中的 `中` 不是全词匹配，而 `needle-`、`needle.` 是。这与 Tauri 版一致，但与 Rust 正则的 `\b` 不同。
3. **`regress` 的已知大小写折叠差异。** 与 P1-11b 登记的一致：`i` 标志下 `/s/i` 会匹配 U+017F（ſ），JS 不匹配。搜索沿用同一引擎，没有另写规避实现。
4. **输出增长与可见行覆盖。** 普通追加输出推进历史长度但不推进 `scrollback_generation`，已有匹配的保留行号仍然正确；新行需要重新扫描才会出现。扫描开始时固定总行数，避免持续输出让扫描无法结束。输出重写可见单元格本身不会被世代捕获；下一批的搜索界面需要在输出到达时去抖重扫，或在绘制前用快照世代/内容校验丢弃过期结果。
5. **resize 的行为变化。** `resize` 现在无条件推进 `scrollback_generation`，即使尺寸相同也会让视图的行缓存重建一次。这样避免“重排后行号仍被当作有效”的错误；`tterm-terminal-view` 的 172 项测试仍全部通过。
6. **备用屏扫描的是备用网格。** 进入备用屏后，搜索结果是备用屏的可见文本（保留坐标从 0 开始），不是隐藏的主屏历史；回到主屏后世代再变一次，主屏结果重新扫描。下一批如果决定备用屏完全不显示搜索结果，应只改界面，不改模型坐标。

## 遗留问题

1. **界面尚未接入。** 搜索栏、快捷键、结果高亮、当前结果颜色、概览搜索标记、上一个/下一个和结果计数的显示都留给下一批。`TerminalView::set_search_marks` 仍默认空集合。
2. **可见行覆盖的失效策略未定。** 世代只覆盖淘汰、清除、备用屏、resize 和 `set_scrollback` 缩容；输出重写同一可见行不会推进世代。下一批需要在“每个输出批次去抖重扫”和“绘制前校验内容/世代”之间做决定。
3. **性能数字有负载噪声。** 测量时机器负载约 11，7 次中位数为 67–69 ms，范围到 126 ms；最慢复制块 624 µs。没有调低/调高任何门槛，也没有把本批数字写成 CI 门槛。协调者应在空闲机器上复跑。
4. **真实桌面、真实 shell 和视觉对比未验证。** 本批只在进程内合成输出上验证模型；没有启动 SSH、PTY 或桌面窗口，也没有和 Tauri 版并排截图。
5. **P2-07 与 G2 未通过。** 本批只完成模型、失效规则、测试和性能基线；搜索的界面、视觉对比和真实桌面验收仍未完成。