# P2-07 第一批：保留历史全量搜索模型

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-07-search-model`，独立 worktree
- 起点：`2ba5e4c5`
- 平台：macOS 27.2（26B123），arm64，Apple M4，36 GB；rustc 1.99.1；cargo 1.102.0；pnpm 11.25.0；Node 24.20.0
- 状态：**自动验收通过；只提交本地分支，未推送、未合并。** 本批只做模型、失效规则和性能基线，没有搜索栏、快捷键、高亮、概览搜索标记或上一个/下一个的界面。P2-07 与 G2 都不因本批通过。修复批补上了独立于滚动缓存的内容版本，见[内容版本修复](#内容版本修复)。

## 结论

本批给生产终端模型加了一条完整的、可在 worker 上运行的搜索路径：保留历史文本使用 P2-06 概览标记的坐标；查询支持正则、区分大小写和全词；扫描只复制有界块并在锁外进行；取消和内容版本校验都有了；10 万行夹具的耗时、分配、匹配数和每块持锁时间已经记录。

修复批解决了初版把 `scrollback_generation` 当成内容版本的问题。搜索现在在锁内与行一起采样独立的 `content_version`：非空写入、改变网格的 resize 和实际裁掉历史的回滚配置会推进它，快照和 viewport scroll 不会。每一块、最后一块之后的复核和空查询的复核都必须属于同一个版本和同一个总行数；不一致就丢弃部分结果并返回 `Stale`。结果返回后，调用方用 `Terminal::content_version()` 就能低成本核验它是否仍新鲜，不需要复制网格。搜索结果的界面、高亮、概览标记和结果导航仍留给下一批。

## 改动

### 提交

| 提交 | 摘要 |
| --- | --- |
| `4dbb62f5` | `:sparkles:` 保留历史全量搜索模型、失效规则、测试、release 测量 |
| `9bb403bb` | `:memo:` 本报告 |
| `ee996817` | `:memo:` 登记 `@xterm/addon-search` 的全词边界集合 |
| `c8bbd604` | `:white_check_mark:` 行尾放不下的宽字符折行测试与报告计数 |
| `d30bd229` | `:memo:` 按迁移报告模板整理本文 |
| `522beb26` | `:zap:` 预取消的搜索在取第一块前返回 |
| `241e89bc` | `:bug:` 用独立内容版本校验搜索结果，不再把滚动缓存世代当内容版本 |

### 主要文件

| 位置 | 职责 |
| --- | --- |
| `crates/tterm-terminal/src/search.rs` | `SearchOptions`、`SearchQuery`、`SearchPosition`、`SearchMatch`、`SearchResult`（含 `generation` 与 `content_version`）、`SearchOutcome`、`SearchCancellation`、`RetainedRows`、`SearchRow`、`SEARCH_CHUNK_ROWS`；保留 P0-09 的 `Search`/`find_next` |
| `.../src/search/query.rs` | 编译字面量或正则查询、大小写与全词、UTF-16 范围到单元格的映射 |
| `.../src/search/rows.rs` | 在锁内把 Alacritty 网格行复制为 UTF-16 单位和单元格位置；跳过宽字符占位格，保留折行，裁掉硬行行尾空白 |
| `.../src/search/scan.rs` | 逻辑行拼接、逐个匹配检查取消、结果收集 |
| `.../src/terminal/search_ops.rs` | `retained_rows`（在锁内同时采样内容版本）、`search_retained`、`content_version` 读取；`find_next` 移入该模块 |
| `.../src/terminal/selection_text.rs` | `selection_text` 与 `active_selection_range` 从 `terminal.rs` 移出，行为不变 |
| `.../src/terminal.rs` | 内容版本字段；非空 `write`、改变网格的 `resize` 和实际裁历史的 `set_scrollback` 推进它；拆出两个子模块，787 行降到 747 行 |
| `.../src/keyword_highlight.rs` | 私有 `escape` 改为 `pub(crate) escape_pattern`，字面量搜索复用同一份 JS 正则转义 |
| `.../tests/search_model.rs` | 17 个生产模型生命周期测试 |
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
    pub fn content_version(&self) -> u64;
    pub fn total_lines(&self) -> usize;
    pub fn matches(&self) -> &[SearchMatch];
    pub fn match_count(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn index_from(&self, origin: SearchPosition, direction: SearchDirection) -> Option<usize>;
}

pub const SEARCH_CHUNK_ROWS: usize = 512;

impl Terminal {
    pub fn content_version(&self) -> u64;
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
- `generation` 是扫描开始时的 `scrollback_generation`，只跟随淘汰、清除、备用屏切换和 `set_scrollback` 裁历史这类缓存事件；它不是内容新鲜度。
- `content_version` 是扫描开始时在同一个锁内与行一起采样的内容版本。结果返回后应与 `Terminal::content_version()` 比较，不同则丢弃。版本只在同一会话内可比；两个会话可能得到相同的数字。
- `retained_rows` 在锁内最多复制 512 行；`search_retained` 只通过这个入口取数据，复制完就释放锁，再在调用线程上扫描。每一块和最后一块之后的复核都必须匹配起始的内容版本与总行数，任何一块也不能越过起始总行数；否则返回 `Stale`，不返回部分结果。`SearchOutcome` 为 `Complete`、`Cancelled` 或 `Stale`。

### 文本与查询规则

- 逻辑行以 `WRAPLINE` 拼接；硬换行分开扫描，因此 `abcde` + 换行 + `fghij` 不会组成 `defg`。
- 宽字符的 `WIDE_CHAR_SPACER` 和行尾的 `LEADING_WIDE_CHAR_SPACER` 不产生文本；宽字符覆盖两列，组合字符跟随其基字符的单元格。
- 非折行行的右侧空白被裁掉；折行行保留整行宽度，因为它还要和下一行拼接。
- 空查询合法，返回 `Complete` 和 0 个匹配，不复制缓冲区、不执行空正则。
- `regex: true` 时非法模式返回 `SearchError`，`Display` 为 `invalid search pattern: ...`，可交给界面显示；不会退化成字面量。`regex: false` 时用仓库已有的 JS 正则转义函数处理查询。
- 正则匹配到空串（如 `^`、`a*`）是合法的，但不产生 0 宽度结果；与前端搜索忽略空匹配的行为一致，测试确认不会死循环。
- 引擎是 `regress`，与前端 `@xterm/addon-search` 和关键词高亮使用同一套 ECMAScript 语义（`\w`、`\s`、`\d`、lookaround、`i`）。全词使用前端固定的非单词字符集，而不是 `\b`。

### 内容版本修复

初版把 `scrollback_generation` 当成内容版本，有三个问题：它看不到同一行的就地覆写（例如 CR 后重写）、ED2，也看不到同一批内进出备用屏；未淘汰的追加不推进它，后一个块可能越过扫描开始时记录的总行数，导致结果范围与实际扫描的行不一致。仅靠下一批界面去抖不能修复模型层的结果。

本修复的做法：

- `State` 持有独立的 `content_version`，`retained_rows` 在锁内把它和行、`total_lines`、`first_line` 一起复制，因此每一块看到的版本与文本属于同一时刻。
- 版本推进是保守的：任何非空 `write`、改变网格尺寸的 `resize`、实际裁掉历史的 `set_scrollback`。空写入、相同尺寸且仅改 cell size 的 `resize`、相同容量或不裁行的容量调整、快照和 viewport scroll 都不推进。
- 每一块必须与起始块有相同的 `scrollback_generation`、`content_version`、`total_lines` 和连续的 `first_line`；块尾不能越过起始总行数。最后一块之后再用 `fetch(total_lines, 0)` 采样一次，空查询也做同样的两次采样。任何不一致都丢弃已收集的匹配并返回 `Stale`，不会把两版拼成一个结果，也不会悄悄返回半套结果。
- 最终复核之后、构造 `Complete` 之前再做一次取消检查；取消优先于版本变化。
- 结果返回后调用方用 `Terminal::content_version()` 复核。这个读取只取一次锁、只读一个 `u64`，不复制行，也不消费 damage。
- 连续输出时连续重试可能一直得到 `Stale`；本批不承诺在无限写入期间最终成功。调用方可以保留旧结果、停止重试或等输出稳定后再扫描。
- 没有改变缓存世代：`resize` 不再推进 `scrollback_generation`，视图仍依赖 Alacritty 的 full damage 重建行缓存；内容失效由独立版本承担。搜索栏、去抖和绘制仍未接入。

### 失效规则

| 事件 | `scrollback_generation`（缓存） | `content_version`（内容） | 旧结果 |
| --- | --- | --- | --- |
| 普通输出增加历史、未淘汰 | 不变 | 增加 | 旧结果失效；已有匹配的保留坐标不变，新行需要重新扫描 |
| 同一行就地覆写（CR 重写、进度条） | 不变 | 增加 | 丢弃；旧匹配不能再当作当前文本 |
| `ED 2` / 擦除可见区 | 通常不变 | 增加 | 丢弃；可见单元格已改变 |
| `ED 3` / `RIS` 清历史 | 增加 | 增加 | 丢弃 |
| 历史满容量后淘汰最旧行 | 增加 | 增加 | 丢弃；重新扫描后行号整体前移 |
| 进入或离开备用屏（跨写入） | 增加 | 增加 | 丢弃；备用屏有自己的 0 起始坐标 |
| 同一批内进入并离开备用屏 | 不变 | 增加 | 丢弃；内容已经换过，但最终模式没变 |
| resize / 重排（网格尺寸改变） | 不变 | 增加 | 丢弃；重新扫描按重排后的行计算 |
| resize（仅 cell size 改变） | 不变 | 不变 | 结果仍有效；保留行没有移动 |
| `set_scrollback` 缩容并实际裁掉历史 | 增加（沿用既有逻辑） | 增加 | 丢弃 |
| `set_scrollback` 同值、扩容或不裁行 | 不变 | 不变 | 结果仍有效 |
| 滚动 / 显示偏移 | 不变 | 不变 | 结果不变；滚动不改变保留坐标 |
| 快照 | 不变 | 不变 | 结果不变；快照不修改网格 |
| 扫描中取消 | 不变 | 不变 | 返回 `Cancelled`，不返回部分结果 |

扫描开始时固定总行数。后续块若来自另一个内容版本或另一个总行数，或者块尾越过这个总行数，整次扫描返回 `Stale`，因此持续输出不会把新行混进旧结果，也不会让一次扫描报告一个与实际扫描范围不符的行数。代价是连续输出时重扫可能反复 `Stale`；本批不保证无限写入期间最终成功。普通追加不改变已有匹配的保留行号，但会推进内容版本，因为旧结果不包含新行。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 从生产终端快照生成稳定的保留历史文本视图，行坐标与 P2-06 概览标记一致；历史淘汰、清除、备用屏和 resize 后旧结果必须失效或可重新扫描。 | 通过 | `search_lines_use_the_same_coordinate_as_overview_command_marks` 直接比较 `Snapshot::command_marks()`；`eviction_advances_the_generation_and_drops_old_matches`、`clearing_the_history_advances_the_generation`、`alternate_screen_switches_invalidate_and_rescan_the_active_grid`、`resize_invalidates_and_rescans_reflowed_rows`；resize 的失效由 `content_version` 断言，且同时断言 `scrollback_generation` 不变 |
| 支持正则、大小写和全词三个选项，复用仓库已有 regex 语义；非法正则返回可显示错误，不 panic、不把查询当普通文本偷偷改写。 | 通过 | `SearchOptions` 三个独立字段；`literal_mode_escapes_regex_syntax_and_regex_mode_does_not`、`case_sensitivity_is_an_explicit_option`、`whole_word_uses_the_frontends_non_word_characters`、`an_invalid_regular_expression_is_an_error_not_literal_text`；引擎为 `regress`，字面量转义复用 `keyword_highlight::escape_pattern` |
| 返回全部匹配的绝对行/范围、结果计数和当前结果索引所需的数据；相邻行/折行匹配要有明确测试。 | 通过 | `SearchResult::{matches, match_count, index_from}`；`results_are_ordered_and_index_the_next_and_previous_match`、`adjacent_hard_lines_do_not_form_a_wrapped_line`、`a_match_can_span_wrapped_rows`、`a_wide_character_that_wraps_keeps_its_place` |
| 扫描放到 worker 可用的纯数据接口；模型锁不能在 10 万行扫描期间长时间占用。先提供可取消/generation 校验的 API，UI 在下一批接入。 | 通过 | `search_retained` 可跨线程调用；`SEARCH_CHUNK_ROWS = 512`；`SearchCancellation`；缓存世代和内容版本逐块及最终复核，不一致返回 `Stale`；benchmark 中 196 块、修复后最慢复制块中位数 236 µs；本批没有接入 UI |
| 使用隔离终端合成 100,000 行，测量普通文本、正则、大小写和全词查询的耗时与分配，报告硬件、行宽、匹配数和命令；不修改性能门槛。测试覆盖空查询、无结果、非法正则、Unicode/中文、折行、输出持续增加、历史淘汰、清除、备用屏和 resize。 | 通过 | 见下面“性能测量”；测试命令和覆盖表见 `## 检查`；没有修改 `scripts/perf/` 或任何阈值 |
| 本批不绘制搜索框，也不伪造 P2-06 概览搜索标记；只在结果接口稳定且 generation 正确后，下一批接入高亮、概览标记、上一个/下一个和结果计数。 | 通过 | `git diff` 不包含 `tterm-terminal-view`；`set_search_marks` 仍默认空集合；结果同时携带缓存世代与内容版本，报告结论明确下一批范围 |
| 使用独立 worktree，由 Pi + DeepSeek V4.1 Flash 实现；不读取真实连接、密钥或正式数据，不 push/merge。 | 通过 | 工作在 `/Users/stone/Code/rust/tTerm-P2-07-search-model` 独立 worktree；夹具是进程内合成输出；没有启动 SSH、PTY 或桌面窗口；`git log` 只在本地分支，未推送、未合并 |

### 修复批验收

| 要求 | 证据 |
| --- | --- |
| 未淘汰追加越过最后块时不得报告与实际范围不符的结果 | `an_untrimmed_append_seen_only_by_the_final_check_discards_the_scan`、`an_untrimmed_append_between_chunks_discards_the_scan`、`a_chunk_that_runs_past_the_sampled_total_discards_the_scan` |
| 同一行就地覆写必须使结果失效 | `an_in_place_overwrite_between_chunks_discards_the_partial_scan`；模型层 `an_in_place_overwrite_invalidates_a_completed_result` |
| ED2 和同批备用屏进出必须使结果失效 | `erasing_the_visible_screen_invalidates_a_completed_result`、`an_alternate_screen_round_trip_in_one_write_invalidates_a_completed_result` |
| 扫描完成后写入能被低成本复核发现 | `a_write_after_a_complete_scan_makes_the_result_stale`；`a_write_during_the_final_check_discards_the_scan` |
| 最终取消检查优先于版本变化 | `the_final_cancellation_check_wins_over_a_newer_content_version` |
| 多块扫描中途写入不混合两版 | `a_write_between_chunks_discards_the_partial_scan`；所有纯扫描测试用闭包按块序号交错，没有 `sleep` |
| 快照与 viewport scroll 不污染内容版本 | `snapshots_and_viewport_scroll_leave_the_content_version_alone` |
| 空写入、同网格 resize、无裁行容量调整不推进内容版本 | `an_empty_write_does_not_advance_the_content_version`、`a_grid_changing_resize_advances_the_content_version`、`only_a_scrollback_change_that_drops_history_advances_the_content_version` |
| 不改缓存世代定义 | `resize_is_not_a_scrollback_cache_event_but_advances_the_content_version`；`resize_invalidates_and_rescans_reflowed_rows` 断言 resize 不推进 `scrollback_generation` |

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
    -- --exact one_hundred_thousand_line_search_reports_time_and_allocations \
    --ignored --nocapture --test-threads=1
```

- 机器不是完全空闲。修复前的初版测量时 `uptime` 的 1 分钟负载约 11；修复后的测量窗口内 1 分钟负载在 2.65 到 16.61 之间，结束时 5 分钟负载为 8.67。下表是 7 次独立运行的**中位数**，括号内是最小–最大；精确复现应在空闲机器上重跑。
- 分配数是 allocator 调用次数；字节数是请求的累计字节（`realloc` 按新大小计），不是峰值常驻。峰值受一块 512 行和匹配向量限制，约几 MiB。

#### 修复前（初版，7 次中位数）

| 查询 | 选项 | 匹配数 | 中位耗时 | 范围 | 分配次数 | 分配字节 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 普通文本 `needle` | 默认（大小写不敏感） | 110,000 | 69,188 µs | 64,908–110,862 µs | 300,217 | 78,789,808 |
| 正则 `alpha \w+ beta` | `regex` | 100,000 | 67,503 µs | 61,207–102,093 µs | 400,217 | 94,789,808 |
| 大小写 `NEEDLE` | `case_sensitive` | 10,000 | 68,305 µs | 62,662–74,830 µs | 300,214 | 71,449,776 |
| 全词 `needle` | `whole_word` | 90,000 | 69,308 µs | 58,617–126,615 µs | 300,217 | 78,789,808 |

#### 修复后（本修复批，7 次中位数）

| 查询 | 选项 | 匹配数 | 中位耗时 | 范围 | 分配次数 | 分配字节 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 普通文本 `needle` | 默认（大小写不敏感） | 110,000 | 57,174 µs | 55,291–58,858 µs | 300,217 | 78,789,808 |
| 正则 `alpha \w+ beta` | `regex` | 100,000 | 58,665 µs | 57,356–60,557 µs | 400,217 | 94,789,808 |
| 大小写 `NEEDLE` | `case_sensitive` | 10,000 | 57,757 µs | 55,971–65,980 µs | 300,214 | 71,449,776 |
| 全词 `needle` | `whole_word` | 90,000 | 57,371 µs | 55,646–59,929 µs | 300,217 | 78,789,808 |

修复后的匹配数和分配次数与修复前一致；耗时差异主要来自测量窗口的负载不同，不能归因于内容版本。内容版本校验没有新增分配，每块只多比较两个 `u64`。

#### 持锁时间

同一夹具另外测量了 `retained_rows`：196 个复制块。修复前总复制时间中位数 51.45 ms，最慢单块 624 µs（7 次范围 492–1,314 µs）；修复后总复制时间中位数 31.22 ms（28,843–31,407 µs），最慢单块 236 µs（222–245 µs）。这是锁内复制加行分配的代理值，扫描和正则匹配都在锁外。100,001 行没有被一次性复制，也没有在扫描期间持有模型锁。复制结果现在多携带一个 `u64` 内容版本，开销可以忽略。

没有新增或修改性能门槛；这些数字是 P2-07 的基线证据，不是 CI 门槛。

## 检查

### 搜索专项

```sh
cargo test -p tterm-terminal --lib search
cargo test -p tterm-terminal --test search_model
```

- `search::tests` 共 29 项，过滤词 `search` 匹配 35 项通过；`search_model` 17 项通过。
- 覆盖：空查询、无结果、非法正则、Unicode/大小写折叠、CJK 全词边界、宽字符、组合字符、折行跨行、行尾宽字符折行、硬换行不拼接、结果排序与 `index_from`、0 宽正则、块边界、世代变化、内容版本中途变化、同一行覆写、ED2、同批备用屏进出、未淘汰追加越过最后块、最终复核写入、最终取消、预取消不复制、长缓冲区分块、类型跨线程。纯扫描测试由 `fetch` 闭包按块序号确定性交错，没有 `sleep`。完整列表见 `search/tests.rs` 和 `tests/search_model.rs`。

### 全套检查

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace` | 全部测试二进制通过；`tterm-terminal` lib 377 项、`search_model` 17 项、`tterm-terminal-view` 172 项、`tterm-core` 1869 项；新的搜索 benchmark 为 `#[ignore]`，默认套件只计数不执行 |
| `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` | 通过；vitest 187 个文件、1367 项，脚本测试全部通过；1510 个文件在大小限制内 |
| `node scripts/perf/gpui-release-frame-budget.mjs` | 通过：foreground baseline p50 109.667 µs / p95 115.791 µs；30-background p50 111.958 µs / p95 160.125 µs；background throughput 193.60 MiB/s，500 ms 截止内完成；两项独立测量都通过 |

release 门槛的报告 commit 是 `241e89bc`（本修复提交）。门槛的负载、步进、采样窗口、16.667 ms p95 预算、500 ms 截止和 30 MiB 总量都没有修改。门槛只在一次运行中通过，协调者仍应在空闲机器上复跑。搜索 benchmark 的 release 命令明确带测试名，避免 `--exact` 匹配零个测试；7 次输出都以 `test result: ok. 1 passed` 结束。

## 偏差

1. **空查询是完整的空结果。** 不把空串当非法正则，也不为它复制 10 万行。界面应把空输入显示为 0 个结果。
2. **全词语义来自前端而不是 `\b`。** 使用 `@xterm/addon-search` 0.16 的固定非单词字符集：空格和 ASCII 标点算边界，下划线和 CJK 字母不算。因此 `_needle` 和 `中文` 中的 `中` 不是全词匹配，而 `needle-`、`needle.` 是。这与 Tauri 版一致，但与 Rust 正则的 `\b` 不同。
3. **`regress` 的已知大小写折叠差异。** 与 P1-11b 登记的一致：`i` 标志下 `/s/i` 会匹配 U+017F（ſ），JS 不匹配。搜索沿用同一引擎，没有另写规避实现。
4. **输出增长与内容版本。** 普通追加不改变已有匹配的保留行号，但会推进 `content_version`，因为旧结果不包含新行；未淘汰追加不推进 `scrollback_generation`。扫描开始时固定总行数，后续块必须属于同一内容版本和同一总行数，块尾也不得越过该总行数，否则返回 `Stale`。内容版本覆盖同一行覆写、`ED 2` 和同一批内进出备用屏，因此界面不必靠去抖来保证正确性；去抖只影响重扫频率。连续写入时可能反复得到 `Stale`，本批不保证无限写入期间最终成功。
5. **resize 不再改变缓存世代。** 修复批撤销了初版让 `resize` 无条件推进 `scrollback_generation` 的做法，缓存世代保持原有契约，视图继续依靠 Alacritty 的 full damage 重建行缓存。改变网格尺寸的 `resize` 只推进 `content_version`；只改 cell size 不推进。`tterm-terminal-view` 的 172 项测试仍全部通过。
6. **备用屏扫描的是备用网格。** 进入备用屏后，搜索结果是备用屏的可见文本（保留坐标从 0 开始），不是隐藏的主屏历史；回到主屏后重新扫描。同一批内进入并离开备用屏时 `scrollback_generation` 可能不变，但 `content_version` 一定推进，旧结果会被判 `Stale`。下一批如果决定备用屏完全不显示搜索结果，应只改界面，不改模型坐标。
7. **内容版本只在同一会话内可比。** 版本是每个模型的本地计数器，不跨会话、跨重连或跨进程比较；两个模型可能报告相同的数字。调用方只能拿同一模型的 `content_version()` 与结果里的版本比较。

## 遗留问题

1. **界面尚未接入。** 搜索栏、快捷键、结果高亮、当前结果颜色、概览搜索标记、上一个/下一个和结果计数的显示都留给下一批。`TerminalView::set_search_marks` 仍默认空集合。
2. **可见行覆盖的失效已有内容版本，界面接入时仍要校验。** 模型在每次非空写入后推进 `content_version`。下一批应在绘制前比较结果的版本与模型当前版本，不同就丢弃；可以在输出到达时去抖重扫以减少扫描次数，但不能省略版本校验。连续写入时扫描可能一直返回 `Stale`。
3. **性能数字有负载噪声。** 修复前测量时 1 分钟负载约 11，7 次中位数 67–69 ms，范围到 126 ms，最慢复制块 624 µs；修复后测量窗口 1 分钟负载 2.65–16.61，7 次中位数 57–59 ms，范围 55–66 ms，最慢复制块 236 µs。没有调低/调高任何门槛，也没有把本批数字写成 CI 门槛。协调者应在空闲机器上复跑。
4. **真实桌面、真实 shell 和视觉对比未验证。** 本批只在进程内合成输出上验证模型；没有启动 SSH、PTY 或桌面窗口，也没有和 Tauri 版并排截图。
5. **P2-07 与 G2 未通过。** 本批只完成模型、失效规则、测试和性能基线；搜索的界面、视觉对比和真实桌面验收仍未完成。