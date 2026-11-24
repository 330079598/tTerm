# P2-07 第二批：GPUI 终端搜索栏

- 日期：2026-11-24
- 分支 / worktree：`migrate/P2-07-search-ui`，独立 worktree
- 起点：`7997ccc2`（第一批搜索模型已合入本地 `GPUI`）
- 平台：macOS 27.2（26B123），arm64，Apple M4，36 GB；rustc 1.99.1；cargo 1.102.0；pnpm 11.25.0；Node 24.20.0
- 状态：**本批完成生产搜索栏、高亮、概览标记、导航和自动化回归，只在本地分支提交，未推送、未合并，未修改 `docs/gpui-migration/status.md`。** P2-07、G2 和桌面验收都不因本批通过。没有读取真实连接、密钥或正式数据。

## 结论

本批把第一批的全量搜索模型接到生产 GPUI 终端：

1. 每个标签页按需创建一个搜索栏实体，绑定自己的 `TerminalView` 和模型。默认/自定义快捷键打开活动窗格的搜索，输入框获得焦点；Esc、关闭按钮和“终端夺回焦点”都会清除结果并把键盘交回终端。
2. 输入、正则、区分大小写、全词、结果计数、上一个/下一个、Enter / Shift+Enter、循环导航都已接通。搜索期间字符只进入输入框，不进入 PTY。
3. 扫描在 GPUI 后台执行器上调用现有的 `search_retained`，每栏最多一个待执行任务；新查询会取消旧任务并提升请求号，只接受“同一模型、同一请求（因此同一查询与选项）、同一内容版本”的完整结果。连续输入用 120 ms 去抖合并，输出后的重扫用 160 ms 去抖。
4. 视图每帧用快照的内容版本、缓存世代和总行数复核已安装的结果；不一致就丢弃结果、全部匹配高亮和概览标记，并通知搜索栏重扫。切窗格、关闭、同一行覆写、擦屏、清历史、重排都会经过同一条失效路径。
5. 匹配高亮按可见行用两次二分查找裁剪后绘制，不逐帧遍历全部匹配；概览标尺沿用 P2-06 的 10 px 轨道和 4096 条上限。选择与搜索高亮是两套独立的绘制输入。
6. 新增 headless GPUI 截图，记录普通/活动高亮、输入框和概览标记。**没有取得原版运行截图**，外观对比只基于锁定版本源码与 CSS，见“偏差与未验证”。

## 原版对照

对照来源是仓库锁定的前端源码，没有运行 Tauri 桌面版：

- `src/components/TerminalTab/TerminalSearchBar.tsx`：输入框、计数、三个选项、上一个/下一个、关闭，Enter / Shift+Enter 和 Escape。
- `src/components/TerminalTab/useTerminalSearch.ts`：打开时聚焦并全选；关闭时清除查询、结果和装饰并聚焦终端；选项变化时清空旧结果后重搜；输入改变时递增搜索。
- `src/components/TerminalTab/searchTypes.ts`：默认三个选项均为 false；匹配色 `#facc15` / `#fde047`，活动匹配色 `#fb923c` / `#fdba74`。
- `src/components/TerminalTab/TerminalOverlays.css`：右上角 12/14 px、12 px 圆角、6 px 间距、7 px 内边距、92% 背景、`backdrop-filter: blur(14px)`、背景色 28% 的 32 px 阴影；输入 220×28、12 px；计数最小 74 px；窄于 640 px 时换行。
- `src/lib/keymap/keymap.ts` 与 `crates/tterm-core/src/keymap/config.rs`：`terminal.find` 在 macOS 默认 `mod+f`，其他平台默认 `mod+shift+f`，持久化配置可覆盖或解绑。

| 项目 | 原版 | 本批 | 差异 |
| --- | --- | --- | --- |
| 位置 | 右上角，拖动后可移动 | 右上角固定 | 未实现拖动，见偏差 |
| 输入框 | 220×28，12 px，占位“Search terminal” | 220×28，复用 `tterm-ui` Input，Small 字号 | 输入框内部排版随组件主题 |
| 计数 | `current / total`、No results、Find in terminal | 同左；扫描中显示 `…`；非法正则显示 “Invalid regular expression” | 非法正则原版只在控制台报错并显示 No results |
| 选项 | CaseSensitive / WholeWord / Regex 图标按钮 | 同左，使用同一组固定颜色和图标 | 未做 hover/pressed 状态跟踪 |
| 匹配色 | `#facc15` 背景/边框，活动 `#fb923c` 背景/边框 | 同色，普通背景 alpha 90/255、活动 128/255，1 px 边框 | 原版运行时是否带透明度和边框宽度未截图核对 |
| 概览标记 | 10 px 轨道中区，普通/活动两色 | 同 P2-06 几何，普通/活动两色 | 超过 4096 条标记只保留最新一段 |
| 搜索方式 | addon-search 的增量 `findNext` | 每次完整扫描保留历史，带结果高亮 | 为“全部高亮”选择全量扫描 |
| 初始活动项 | 从 xterm 当前搜索位置开始 | 从当前视口顶部开始，命中不到时回到第一条 | 语义接近但不完全相同 |
| 拖动/移动 | 有 | 无 | 见偏差 |

## 改动

### 提交

| 提交 | 摘要 |
| --- | --- |
| `7d289bd3` | `:recycle:` 把失败标签占位从 `window.rs` 拆到 `failed_tab.rs`，为新增模块留出大小余量 |
| `ab2f7818` | `:sparkles:` 视图接收搜索结果、按可见行绘制普通/活动高亮、按内容版本丢弃过期结果；快照增加内容版本；模型增加按保留行滚动 |
| `82f18859` | `:sparkles:` 搜索栏翻译键、Input 高度选项和选项图标 |
| `1241da0e` | `:sparkles:` 每窗格搜索栏、后台扫描、导航、快捷键、窗格归属、真实键鼠测试和 headless 截图 |
| `b6a38055` | `:fire:` 删除未使用的拖动把手图标 |
| `4c4f53ba` | `:lipstick:` 搜索装饰绘制 1 px 普通/活动边框 |
| `a429b04a` | `:bug:` 删除调试探针，输入框改用 Small 字号 |
| `af29c553` | `:white_check_mark:` 视口 resize 使搜索覆盖层失效的测试 |
| `4fb92812` | `:bug:` 导航前拒绝旧内容版本的搜索结果 |

### 主要文件

| 位置 | 职责 |
| --- | --- |
| `crates/tterm-terminal-view/src/view/search.rs` | `SearchOverlay`、可见行裁剪、标尺标记生成、内容版本/世代/总行数校验 |
| `.../src/view/search_tests.rs` | 生产视图的 6 个搜索绘制/失效/裁剪/选择独立测试 |
| `.../src/view/paint.rs` | 普通/活动匹配背景和 1 px 边框，绘制在背景/选择之后、字形之前 |
| `.../src/view/prepare.rs` | 每帧校验覆盖层、计算可见区间统计、把裁剪结果交给绘制 |
| `.../src/view/mod.rs` | `set_search_overlay`、`clear_search_overlay`、`set_search_stale_listener`、`search_origin`、`reveal_search_line`、`model()` |
| `.../src/snapshot.rs`、`src/terminal/search_ops.rs` | 快照携带内容版本；`scroll_to_retained_line` 只在目标不在视口时移动 |
| `crates/tterm-gpui/src/terminal_demo/window/search/mod.rs` | `SearchBar` 状态、打开/关闭/计数文本、渲染根、窗格归属 |
| `.../window/search/controller.rs` | 去抖、取消、请求号、内容版本验收、Enter/导航、输出后的重扫 |
| `.../window/search/bar.rs` | 输入、计数、三个选项、上一个/下一个、关闭按钮和可访问标签 |
| `.../window/search/tests.rs` | 12 个生产输入/绘制/失效/取消/乱序测试 |
| `.../window/search/capture.rs`、`crates/tterm-gpui/examples/search_capture.rs` | debug + automation 的 headless 截图，模型为进程内合成输出 |
| `.../window/search_workspace_tests.rs` | 默认/自定义/解绑快捷键、Esc、双窗格归属 |
| `crates/tterm-core/src/i18n/keys.rs`、`src/i18n/locales/{en,zh}.json` | 新搜索键；新增 `invalidRegex`，两种语言同步 |
| `crates/tterm-ui/src/components/input.rs` | `InputOptions::height`，默认 36，搜索栏用 28 |
| `crates/tterm-ui/assets/icons/{case-sensitive,whole-word,regex}.svg` | 从 lucide 0.555.0 提取的同形图标；生成脚本的名单同步 |

### 状态与失效

```text
open ──输入/选项变化──▶ 清空旧结果 ──去抖──▶ 后台 search_retained
  │                                               │
  │                                     请求号/内容版本匹配？
  │                                     否 ──▶ 丢弃并重扫
  │                                     是 ──▶ 安装 SearchOverlay
  │                                               │
  └──────────── Esc/关闭/切窗格 ◀── 结果/高亮/标尺 ◀┘
```

| 事件 | 搜索栏 | 视图覆盖层 | 标尺 |
| --- | --- | --- | --- |
| 输入或切换选项 | 立即清空结果，状态 Waiting，去抖重扫 | 立即清空 | 立即清空 |
| 普通输出、追加 | 下一次绘制发现版本变化，清空结果并 160 ms 后重扫 | 快照版本不一致时丢弃 | 随覆盖层清空 |
| 同一行覆写 / ED2 | 同上 | 同上 | 同上 |
| 清历史 / ED3 / RIS | 同上 | 同上 | 同上 |
| 网格 resize / 重排 | 同上 | 同上 | 同上 |
| 仅 cell size resize | 保持结果 | 版本不变，继续绘制 | 继续绘制 |
| 滚动 | 结果不变 | 可见行重新裁剪 | 坐标不变 |
| 选择变化 | 结果不变 | 搜索与选择分别绘制 | 不变 |
| 切窗格 / 新标签 / 开关页 | 旧窗格关闭并清空 | 清空 | 清空 |
| 关闭搜索 | 清空查询、结果、任务 | 清空并移除监听 | 清空 |
| 扫描期间取消 | 旧请求取消，旧完成被请求号拒绝 | 不安装 | 不安装 |
| 扫描期间内容变化 | 模型返回 Stale；栏重排去抖重扫 | 不安装 | 不安装 |

### 后台扫描与交错

- 每个 `SearchBar` 最多持有一个 `Task`；`schedule_scan` 先取消并丢弃旧任务，再提升 `request`，因此连续输入不会堆积扫描，也不会让旧任务在新查询后继续占用模型锁。
- 扫描本身是 `cx.background_executor().spawn(...)` 上的阻塞调用，`search_retained` 在锁内只复制 512 行块；主线程只做结果验收和绘制。
- `accept_outcome(request, outcome)` 依次检查：栏仍打开、请求号等于当前请求、`Complete` 的内容版本等于 `model.content_version()`。视图在绘制前再检查内容版本、`scrollback_generation` 和 `total_lines`。
- 取消测试用真实的 `SearchCancellation`；乱序测试先构造旧版本结果，再构造新版本结果，按“旧完成 → 新完成 → 更旧请求”的顺序注入，旧请求和旧版本都不能替换当前结果。
- 第一次扫描的活动项从 `TerminalView::search_origin()`（最后绘制帧的视口顶部保留行）开始；重扫优先保留活动匹配的起点；找不到时回到视口顶部，再找不到就回到第一条。导航使用 `index_from` 的相邻项，并在末尾/开头循环。

### 绘制

- `SearchOverlay::visible` 对匹配按结束行和起始行各做一次 `partition_point`，只遍历与可见行相交的匹配；一个跨折行匹配按可见物理行拆成多个 span。
- `ViewStats` 新增 `search_spans`、`search_active_spans`、`search_matches_visited`、`search_stale_clears`，测试用它们证明“只绘制可见行”“活动项确实被绘制”“过期后消失”。
- 普通/活动背景分别使用 `#facc15` / `#fb923c` 的 alpha 90/255 和 128/255；1 px 边框使用 `#fde047` / `#fdba74`。字形在选择色和搜索高亮之上绘制，选择与搜索互不修改对方的数据。
- 概览标记由同一份匹配的起始行生成，最多 4096 条；导航时重新生成活动标记，不逐帧扫描全部匹配。

### 快捷键与焦点

- `register_actions` 新增 `TerminalFind`，`workspace_key` 在活动窗格视图聚焦时处理它并打开搜索。默认键值和自定义覆盖都走已有 `KeymapRouter`，没有新增一套键位解析。
- `workspace_key` 在没有搜索时不拦截 Esc；有搜索时关闭它，即使焦点已经在终端上。搜索输入框聚焦时，工作区快捷键不会被当作终端输入。
- 输入框 Enter 发 `InputEvent::PressEnter`，Shift 修饰决定上一个；Bar 自己的 `on_key_down` 处理 Escape，也检查 `prefer_character_input` 以避开 IME 字符输入。
- 关闭时 `FocusHandle` 回到 `TerminalView`；切窗格关闭时不抢活动窗格焦点。
- 打开的搜索栏从 `render_pane` 作为绝对定位子元素挂到对应窗格；关闭后实体不再渲染任何元素。切窗格、打开设置页或隧道页会先关闭活动搜索，`refresh_panes` 会关闭其他所有窗格的搜索并清空覆盖层。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 比对原版搜索栏、hook、CSS、快捷键与配置；接入真实活动窗格、可编辑输入、三个选项、计数、上下项、Enter / Shift+Enter、Esc / 关闭与焦点恢复；复用翻译、输入组件和键位路由，不抢无关快捷键，不把输入发给 PTY。 | 通过（拖动和原版运行截图除外） | `the_case_whole_word_and_regex_options_change_the_count`、`navigation_wraps_and_scrolls_an_off_screen_match_into_view`、`the_default_find_shortcut_opens_the_active_panes_search`、`a_custom_find_binding_replaces_the_default_and_unbinding_does_nothing`、`escape_closes_the_search_even_when_the_terminal_has_the_keyboard`、`typing_searches_without_reaching_the_terminal_and_esc_restores_it`、`the_close_button_restores_the_terminal_focus_and_hides_the_bar`；翻译键与选项图标提交 `82f18859` |
| 复用 `search_retained`、取消标记及内容版本；后台扫描、合并连续更新、只接受同会话同查询同版本结果；不得在渲染循环内扫描历史或无限堆积任务。 | 通过 | `controller.rs` 单一 `Task`、请求号、取消标记；`rescheduling_cancels_the_previous_scan`；`an_older_completion_and_a_stale_version_are_ignored`；`output_clears_the_result_and_a_settled_rescan_includes_the_new_rows`；`ViewStats::search_matches_visited` 只统计可见区间 |
| 切窗格、关闭、覆盖、清历史、重排时，栏内结果、可见匹配高亮及概览标记不得残留。 | 通过 | `switching_panes_closes_the_previous_panes_search_and_its_highlights`；`an_in_place_overwrite_and_a_history_clear_remove_the_result`；`output_clears_the_result_and_a_settled_rescan_includes_the_new_rows`；`a_hidden_view_keeps_no_search_overlay`、`a_grid_changing_resize_drops_the_overlay_before_painting`；视图的 `is_stale_for` 同时复核内容版本、世代和总行数 |
| 上/下一项循环且在屏外时滚入可见。 | 通过 | `navigation_wraps_and_scrolls_an_off_screen_match_into_view`（上一项跨顶部、下一项跨尾部、计数 `120 / 120`）；`revealing_an_off_screen_match_scrolls_it_into_the_viewport` |
| 选择与搜索高亮彼此独立，按可见行裁剪，不逐帧遍历所有匹配。 | 通过 | `a_large_result_is_clipped_to_the_viewport_and_selection_stays_independent`（>1000 条匹配，仅访问视口行；清除选择后搜索仍在）；`visible_spans_are_clipped_to_the_viewport_instead_of_the_whole_result`；`a_match_that_wraps_into_the_viewport_is_painted_on_both_rows`；`marks_keep_the_active_flag_and_are_bounded` |
| 用生产视图、隔离终端和真实键鼠事件回归：默认/自定义快捷键及回退、两窗格隔离、中英文、正则出错/空结果、选项、导航计数、Esc 后重新输入、搜索时无输入泄漏。 | 通过 | `search/tests.rs` 12 项、`search_workspace_tests.rs` 5 项、`view/search_tests.rs` 6 项；均挂载生产 `TerminalView` + `SearchBar`，用 `VisualTestContext` 派发真实键鼠事件；模型是进程内合成输出，没有启动 SSH/PTY/桌面窗口 |
| 用确定性交错验证取消、乱序完成和生产绘制（普通/活动高亮与标尺，结果过期后消失）。 | 通过 | `an_older_completion_and_a_stale_version_are_ignored`、`rescheduling_cancels_the_previous_scan`；`visible_matches_paint_normal_and_active_spans_and_feed_the_ruler`、`output_drops_the_overlay_before_the_next_frame_and_reports_it`；`ViewStats::search_active_spans`、`overview_marks`、`search_stale_clears` |
| 捕获原版与 GPUI 同状态界面以比较外观；如无法取得证据如实登记。 | **部分：仅 GPUI 截图** | `docs/gpui-migration/reports/evidence/P2-07-search-ui/search-bar.png`（1600×1200，2×）；原版运行截图未取得，见“偏差与未验证” |
| 按 AGENTS.md 全量检查与 idle release 双门；未通过不能调门槛。 | 通过 | 见“检查”；门槛负载、步进、采样窗口、16.667 ms p95、500 ms 截止和 30 MiB 总量均未改 |
| 成果与可复验步骤写入报告；核对本地提交和工作树。 | 通过 | 本文件；“复验步骤”；提交 `7d289bd3`…`4fb92812`，交付前工作树干净 |

## 交互与绘制证据

### 真实键鼠测试

`search/tests.rs` 用 `cx.add_window_view` 挂载：

```text
SearchRoot
├── TerminalView（生产视图，synthetic model，输入 sink 记录 PTY 字节）
└── SearchBar（生产搜索栏，真实 InputState）
```

测试通过 `VisualTestContext` 的 `dispatch_keystroke`、手动 `MouseDown/MouseUp`（由渲染后的 debug bounds 定位）和 `run_until_parked` 操作，不直接调用绘制函数伪造结果：

- 输入 `needle` 后结果计数、活动项、高亮数量都来自生产扫描和绘制；PTY 字节为空。
- Enter / Shift+Enter、上下按钮、首尾循环、屏外滚入、关闭按钮、Esc 后重新输入全部通过真实事件。
- 中文查询 `中文` 命中合成行；切换语言后计数和无结果文案都从 `Locale` 读取。
- 正则 `(` 产生 `Error` 且不绘制；`zzz` 产生空结果；三个选项用真实鼠标点击切换。
- 同一行覆写（CSI 上移 + `\r`）和 `ED2/ED3` 都让旧结果消失，重扫结果正确。
- 取消测试读出旧 `SearchCancellation` 并确认新请求将其置为 canceled；乱序测试用真实 `SearchResult` 的不同内容版本注入完成。

### 生产绘制测试

`view/search_tests.rs` 每一项都读取 `TerminalView::last_frame` 和 `ViewStats`：

- 普通/活动 span 同时存在，标尺有标记，且活动标记只有一个。
- 输出后下一帧 `search_spans == 0`、`search_stale_clears == 1`、监听器收到通知、覆盖层和标记为空。
- 3000 行、>1000 条匹配时 `search_matches_visited <= screen_lines`；选择存在时搜索仍绘制，清除选择后搜索仍在。
- 屏外匹配调用 `reveal_search_line` 后落在视口内。
- 网格 resize 后旧覆盖层在下一帧之前被丢弃，计数统计 `search_stale_clears` 增加。
- 隐藏视图立即丢弃覆盖层和标记。

### 截图

- 文件：`docs/gpui-migration/reports/evidence/P2-07-search-ui/search-bar.png`（1600×1200，2×，800×600 逻辑像素）。
- 清单：同目录 `capture.json`，记录生产 `TerminalView` + `SearchBar`、合成输出、dark 主题、英文、`query=needle`、`28 / 80`、活动项和概览标记。
- 可复验命令：

```sh
cargo run -p tterm-gpui --example search_capture --features tterm-gpui/automation -- \
  --output docs/gpui-migration/reports/evidence/P2-07-search-ui
```

截图不是桌面验收：窗口由 `HeadlessAppContext` 和 headless renderer 绘制，没有打开原生窗口，也没有输入真实 shell。

## 检查

### 前端

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
```

全部通过：vitest 187 个文件、1367 项；1517 个文件在大小限制内。

### Rust

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

全部通过。`tterm-gpui` lib 294 项、3 项 ignored；`tterm-terminal-view` lib 182 项；`tterm-terminal` lib 377 项、`search_model` 20 项；其余 workspace 测试二进制全部通过。搜索 benchmark 仍为 `#[ignore]`，默认套件只计数不执行。

### release 双门

```sh
node scripts/perf/gpui-release-frame-budget.mjs
```

- 机器：Apple M4（10 CPU），36 GB，macOS 27.2；运行前 `uptime` 的 1/5/15 分钟负载为 7.20 / 7.87 / 7.25。
- 帧：baseline p50 109.375 µs / p95 156.583 µs；30-background p50 138.917 µs / p95 163.416 µs；p95 预算 16.667 ms，远未触及。抽样窗口 16.111334 ms 内 30 个会话全部前进，确认 3,312,800 字节（196.09 MiB/s）。
- 吞吐：30 会话、30 MiB 全部应用并确认，174.231 ms 完成（172.18 MiB/s），低于 500 ms 截止；`completed=true`。
- 两项独立测量都通过；门槛代码、负载、步进、采样窗口和截止均未修改。

## 偏差与未验证

1. **没有原版运行截图。** 本批只对照锁定的 React/TS/CSS 源码；没有启动 Tauri 桌面版，也没有把 GPUI 截图与运行中的原版逐像素比较。headless 截图只是 GPUI 自证，不是外观验收。
2. **未实现拖动把手。** 原版可拖动搜索框位置并限制在终端表面内；本批固定右上角。为此加入了 `grip-horizontal` 图标后又删除，避免留下无行为控件。
3. **`backdrop-filter: blur(14px)` 未模拟。** GPUI 端只绘制 92% 背景和阴影，没有背景模糊。
4. **高亮透明度是近似值。** 原版 `searchTypes.ts` 只给出固定十六进制色和边框色，没有给出运行时 alpha；为避免完全遮住字形，本批用 90/255 和 128/255，并用 1 px 边框。没有原版运行截图核对实际效果。
5. **按钮没有 hover/pressed 动效。** `tterm-ui` Button 的静态变体已按选项状态切换，但没有像 React 那样跟踪 hover。
6. **非法正则的文案不同。** 原版捕获错误后在控制台打印并显示 “No results”；本批把翻译后的 “Invalid regular expression” 直接显示在计数位置，方便用户修正。
7. **全量扫描替代增量 findNext。** 为了让所有匹配同时高亮，每次查询都做完整保留历史扫描并去抖；原版 addon-search 可以在输入时走增量 findNext。扫描在后台和锁外进行，但 10 万行时仍需要第一批测得的几十毫秒。
8. **初始活动项从视口顶部开始。** 原版从 xterm 当前搜索位置开始；本批以最后绘制帧的视口顶部为起点，命中不到时回到第一条。重扫优先保留当前匹配起点。
9. **标尺上限沿用 4096 条。** 匹配高亮本身不设上限，因为按可见行裁剪；概览标尺只保留最新 4096 条匹配，和 P2-06 的源上限一致。
10. **中文测试是提交后的键事件。** 真实 IME 组合由共享 `InputState` 处理，本批没有在原生窗口里验证输入法候选框/组合中的 Escape。测试只覆盖已提交的 CJK 查询和翻译。
11. **没有真实桌面、真实 shell 集成或 SSH 验收。** 所有测试使用进程内合成模型；workspace 快捷键测试使用的是本地 `/bin/sh`/`cmd.exe` 会话，但不评估视觉或真实 SSH。
12. **P2-07 与 G2 未通过。** 本批完成搜索栏、高亮、结果导航、自动化回归和 GPUI 截图；桌面验收、原版视觉对比、拖动和原生 IME 仍待补。

## 复验步骤

```sh
git status --short                     # 应为空
git log --oneline 7997ccc2..HEAD       # 12 个本批提交（含两次报告和 failed_tab 预拆分）
cargo test -p tterm-terminal --test search_model
cargo test -p tterm-terminal-view --lib search
cargo test -p tterm-gpui --lib search::tests
cargo test -p tterm-gpui --lib search_workspace_tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
node scripts/perf/gpui-release-frame-budget.mjs
cargo run -p tterm-gpui --example search_capture --features tterm-gpui/automation -- \
  --output docs/gpui-migration/reports/evidence/P2-07-search-ui
```

复跑 release 门时请让机器空闲，并记录当时的 `uptime`；本报告的数字来自一次这样的运行，不代表桌面手测。

## 提交与工作树

```
7d289bd3 :recycle: Move the failed-tab placeholder out of the window root
ab2f7818 :sparkles: Paint accepted search matches and drop stale ones by content version
82f18859 :sparkles: Add the search-bar translations, compact input height and option icons
1241da0e :sparkles: Wire the per-pane search bar to the production retained-history scan
b6a38055 :fire: Drop the unused drag-handle icon
4c4f53ba :lipstick: Draw the search decoration borders from the frontend palette
a429b04a :bug: Remove the search debug probes and use the compact input size
af29c553 :white_check_mark: Cover a grid-changing resize invalidating the search overlay
4fb92812 :bug: Refuse to navigate a search result from an older content version
883b8cc0 :memo: Record the GPUI search-bar implementation and its evidence
890d4483 :bug: Leave find to the settings page instead of a terminal pane
```

报告先由 `883b8cc0` 提交，随后续两处修正由本文件所在的 memo 提交更新；历史中保留两次 memo 提交。

交付前 `git status` 为空。提交只在本地分支 `migrate/P2-07-search-ui`，未推送、未合并，未修改 `docs/gpui-migration/status.md`。
