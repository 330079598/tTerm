# W0-05 参照合并前的 Rust 复核（2026-11-23）

基点 `954e4261`，独立修复分支 `migrate/W0-05-validation-repairs`，macOS。本卡不改参照资产或 GPUI 设置界面。

| 阻塞 | 修复与证据 | 结论 |
| --- | --- | --- |
| 严格 Clippy 的 7 项 dead code | `48157589` 将私有用途的 sshd 辅助模块限定到 Unix 单测目标；实际跨 crate 的 test-support API 未动。协调者复跑 `cargo clippy --workspace --all-targets -- -D warnings` 通过。 | 通过 |
| Alt 鼠标上报 | `f813b8bd` 在用例中显式选择要测试的策略，不依赖机器默认策略。原有按下与抬起的协议字节断言及 macOS Option 选择路径保留。 | 通过 |
| 30 个后台解析器下的前台帧预算 | `edec6ecf` 优化可见行缓存键的计算，未改性能测试。实施者的测量中，单帧基线由约 1.54 ms 降至 0.67 ms；但标准套件仍出现前台 p95 或累计处理量不达标。 | 改善，尚不能认定稳定通过 |

协调者在最终代码上复跑六项 pnpm 检查、Rust fmt、严格 Clippy 均通过；不改 profile、并发和断言，`cargo test --workspace` 独立连续四次通过。四次通过不抹掉实施阶段同提交的失败。每次测量除了 p95 ≤ 16.667 ms，还须在原工作负载下处理至少 30 MiB；降低后台优先级却使吞吐不过线不算修复。

**结论：** 保留修复提交与三屏参照分支，暂不合并或推送。下一轮单独测量前台帧采样窗口的实际长度、后台生产与确认速率、解析线程可运行但未获调度的时间，再针对后台解析开销或会话之间的 CPU 分配优化；不以重复运行直到成功代替验收。设置流程待此项收口。

## 2026-11-23 第二轮：6b8924fb 的 30 会话帧预算诊断

范围与约束：独立 worktree `migrate/W0-05-validation-repairs`，macOS 10 核（`hw.ncpu=10`），系统默认调度器。本轮未改生产者、断言、采样范围或顺序、时钟、Cargo profile、依赖版本、文件大小基线和缓存基线；没有 push、合并或读取用户数据。临时插桩（`Terminal::write`/`Reports::observe` 的 `Instant` 计时、macOS `thread_info(THREAD_BASIC_INFO)` 前台线程 CPU 探针）已全部还原；沙箱拒绝 `ps`/`sample`，线程 CPU 只能由该临时探针取得。最终提交只保留测试内两处只读快照和一行速率输出，不改变任何断言。

### 采样方法

- 基准二进制：`cargo test -p tterm-gpui --lib thirty_background_parsers --no-run`；单测运行：`cargo test -p tterm-gpui --lib thirty_background_parsers -- --nocapture --test-threads=1`（或直接运行 `--no-run` 输出的二进制）。
- 两个快照点：100 ms 预热结束（`warmup_end`）和 120 帧结束（`warmup_end.elapsed()`）。窗口内确认量取 30 个会话 `acknowledged_bytes()` 的差值；累计量在生产者 join 后再读，可能包含生产者退出期间的部分 drain，并非等所有解析线程退出后的总量。
- 每个候选固定样本：隔离测量 3–4 次；全量并行套件 A/B 为未改侧 8 次、闸门侧 7 次。失败样本保留在下表，没有追加运行直到全部通过。
- 结果分三类：隔离单测（`--test-threads=1`，无同级测试）、`cargo test -p tterm-gpui --lib`（默认并行，268 个测试）、`cargo test --workspace`。三者不是同一个测量环境，不能混用通过率。

### 未改负载的隔离读数（最终提交的诊断版本，n=3）

| # | 基线 p50 / p95 | 30 会话 p50 / p95 | 预热确认 | 帧窗口 | 窗口确认与速率 | 累计确认 | snapshot hold 最大 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 673.25 µs / 692.125 µs | 691.791 µs / 1.170 ms | 19,860,400 B | 215.800 ms | 38,654,800 B（36.86 MiB，170.83 MiB/s） | 59,482,800 B | 118.599 ms |
| 2 | 683.042 µs / 740.542 µs | 719.625 µs / 4.786 ms | 20,647,600 B | 429.882 ms | 80,737,200 B（77.00 MiB，179.11 MiB/s） | 102,516,400 B | 20.119 ms |
| 3 | 686.083 µs / 729.458 µs | 704.917 µs / 742.75 µs | 22,500,800 B | 89.324 ms | 14,956,800 B（14.26 MiB，159.69 MiB/s） | 38,113,600 B | 2.699 ms |

这三次都通过当前的 p95 和 30 MiB 断言，但 p95 从 0.74 ms 到 4.79 ms 波动；第 1 次单次持锁被抢占达 118.6 ms，仍只影响不超过 6 帧，所以 p95 没被推过 16.667 ms。未改代码在所有版本上的隔离样本是 10 次 5 过（p95：27.263、33.317、20.080、5.941、26.081、19.515、3.169、1.170、4.786、0.743 ms），不是稳定的通过或稳定的失败。

### 解析路径的累计耗时（临时插桩，n=3）

| # | 窗口确认量 | 帧窗口 wall | 前台线程 CPU / wall | `Terminal::write` 累计 wall | 其中 `parser.advance` | 其中 OSC 扫描 | 其中 Reports |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 80,130,400 B | 439.5 ms | 133.5 / 439.5 ms（30%） | 7.909 s | 7.870 s（99.5%） | 39.2 ms | 49.0 ms |
| 2 | 51,824,000 B | 287.5 ms | 84.9 / 287.5 ms（30%） | 4.460 s | 4.425 s（99.2%） | 34.9 ms | 22.3 ms |
| 3 | 59,630,400 B | 334.5 ms | 131.8 / 334.5 ms（39%） | 6.393 s | 6.361 s（99.5%） | 32.5 ms | 34.9 ms |

解析线程用 `Instant` 计时；不同线程的墙钟时间相加包含同时执行与等待调度的时段。三次样本中这些时段主要落在 `advance`，但不能据此折算核心需求、单核吞吐或 CPU 占比。前台线程 CPU / wall 是另一组独立测量。

单态化落点：vte 0.15 的 `Processor::advance<H>`、`Performer<H>` 和 alacritty 0.26 的 `impl<T: EventListener> Handler for Term<T>` 都以本仓库的 `Collector` 为类型参数，实例化在 `tterm-terminal` 自己的代码生成单元里。`[profile.dev.package."*"] opt-level = 2` 只覆盖 registry 依赖，不覆盖 workspace 成员，因此这段 VT 状态机和 `input` 在测试/dev 下以 opt-level 0 运行。这提示应比较相同负载的 release 与 dev 数据，不能据此认定生产瓶颈。

### 尝试 1：后台解析并发闸门（已实现、测量、随后还原）

做法：进程级计数闸门限制同时执行 `apply_and_release` 的会话数，默认 `available_parallelism - 2`（本机 8）；许可在单批解析后释放，队列仍是原来的有界队列，生产者按背压阻塞，不丢字节。

隔离结果（`-2`，n=8，含 4 次带临时计时和 4 次干净构建）：p95 = 1.911、2.432、2.026、1.916、1.364、1.926、1.964、1.641 ms，8/8 通过；累计 37.98–44.90 MB（全部超过 30 MiB）；窗口速率 152–160 MiB/s；前台 CPU/wall 为 101.3/102.0、108.1/119.5、97.2/101.8、95.2/96.9 ms（已不再被饿死）；生产者停止尾 2.3–12.7 ms，解析器 drain 9.7–16.9 ms。`-1` 变体（本机 9 个许可，n=4）同样全过：p95 0.708、0.706、1.522、0.719 ms，累计 35.29–36.92 MB，速率 156–159 MiB/s。

但同一改动在全量并行套件里变差：

| 条件 | 通过 / 样本 | 观测到的 30 会话 p95 |
| --- | --- | --- |
| 未改，`cargo test -p tterm-gpui --lib` | 4 / 8 | 1.695、4.807、8.878 ms 通过；21.980、33.857 ms 失败；另 1 次累计仅 13.66 MB 失败；1 次失败未捕获到该用例输出 |
| 闸门 `-2`，同上 | 0 / 7 | 19.639、22.757、28.043、20.715、30.881 ms 失败；另 2 次该用例失败但 stdout 被并发测试截断 |

解释：默认并行套件里，前台是测试 worker 线程，和其余 268 个测试争抢 10 个核。未改代码的 30 个解析器把机器占满，反而压住了同级测试，前台 p95 有时更低；闸门留出两个核后，同级测试可以突发并抢占前台，p95 反而更高。也就是说，这个基准只有在它自己的工作负载独占机器时才容易变绿——这是测量隔离缺陷，不是“前台能承受 30 个后台会话”的生产结论。按“仅达标且无已知退化才保留”的规则，闸门已还原，没有留在提交里。

### 尝试 2（评估后未实现）：可打印 run 跳过状态机

跳过 VT 状态机需正确处理分段、混合和跨块控制序列。没有足够的语义正确性与性能证据，因此没有实现。

### 结论（分开回答）

1. **负载和吞吐是否物理可达：可达，但累计门槛不是速率门槛。** 实测帧窗口吞吐 96–180 MiB/s（隔离 152–180，并行套件 96–103），30 MiB 只需 0.17–0.33 s。与此同时，累计确认值随帧窗口长度变化：同一未改代码在并行套件里出现过窗口缩短到累计只有 13.66 MB 的失败，而窗口内速率仍高于 30 MiB/s 所需。因此“累计字节不够”不能单独证明机器吞吐不够；本轮的吞吐结论以窗口内 MiB/s 为准，累计值只作为现有断言的结果记录。
2. **前台预算是否通过：不能认定通过。** 最终未改代码的隔离样本 5/10 通过（p95 0.74–33.32 ms），并行 gpui 套件 4/8 通过；单次 `cargo test --workspace` 通过，但同一环境只跑了 1 次，不能覆盖 4/8 的波动。闸门版本在隔离下 8/8、p95 0.7–2.4 ms，在并行套件下 0/7，所以“预算物理上是否可达”和“仓库当前命令是否稳定通过”是两个不同答案：前者可达，后者不通过。默认并发下也存在失败，不宜仅归因于机器负载。

### 需要进一步决策

1. **隔离该基准**：把 `thirty_background_parsers_keep_the_foreground_within_its_frame_budget` 放到串行/独立性能任务里运行，或至少在该用例期间不与其余 268 个测试并行。这会改变测试调度，超出本卡“不得调度该测试本身”的限制，需要评估者决定。
2. **是否允许 dev/test 对 workspace 成员开启优化**：这是消除 `Processor::advance` 单态化落在 opt-level 0 的直接办法；本轮明确禁止改 profile。
3. **是否采用解析闸门**：它在隔离条件下有 8/8 的 p95 余量和 152–160 MiB/s 吞吐，但会让默认并行套件的该用例 0/7；只有先做决策 1 才建议采用。`-1` 与 `-2` 都已测量，两者隔离全过，`-2` 的尾延迟余量更大。

### 不需要改门槛的替代方案

- 保留本轮的测试内两处只读快照和速率输出，按 `--test-threads=1` 单独重跑，并按“窗口 MiB/s”解读吞吐；不改 16.667 ms 和 30 MiB。
- 不降低 p95 或累计门槛，不增加重试次数，也不把 `cargo test --workspace` 的单次通过当作稳定证据。
- 在决策 1 完成前，不引入闸门或任何以牺牲后台吞吐换前台的实现；本轮最终代码没有生产行为变更。

### 复验记录（全部保留）

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：1 次，退出码 0；该次基准用例的 stdout 未被捕获，因此不把它算作带数值的通过样本。
- `cargo test -p tterm-gpui --lib`：未改代码 4/8 通过；闸门 0/7；最终诊断版本 1/2（p95 8.381 ms 通过、23.180 ms 失败）。
- `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`：通过；186 个测试文件、1364 项测试，layout 9 项、migration 8 项，1496 个文件在大小限制内。
- 隔离基准：最终诊断版本 3/3 通过；未改代码全部版本合计 5/10。临时文件只在 `/tmp`（`w0-05-build.log`、`w0-05-tests-original.rs`、`w0-05-bin.txt`），未进入仓库。

### 协调者独立复核

在 `1809b4ab` 上，Rust fmt 与严格 Clippy 通过，默认 `cargo test --workspace` 失败。120 帧窗口 391.70 ms，窗口确认 33,603,600 B（81.81 MiB/s），累计 42,476,000 B，超过 30 MiB 下限；前台 p95 从空载 4.67 ms 升至 25.70 ms，超过 16.667 ms。只读诊断可复现，本次没有可验收的生产修复。

## 2026-11-23 第三轮：独立 release 门禁

决策人已确认把第二轮“需要进一步决策”的第 1 条落实为[决策 019](../decisions/019-release-frame-budget-gate.md)：只把这一个用例移出默认并行套件，改成独立、无条件的 release 门禁。本轮不改负载、生产节奏、采样窗口、16.667 ms 预算、30 MiB 下限、Cargo profile 或产品代码，也没有 push、合并或读取用户数据。

### 改动

| 位置 | 内容 |
| --- | --- |
| `crates/tterm-gpui/src/terminal_demo/pipeline/tests.rs` | 给 `thirty_background_parsers_keep_the_foreground_within_its_frame_budget` 加带理由的 `#[ignore]`。断言一条未改；只把一行诊断输出移到 30 MiB 断言之前，使未达累计量的运行也能留下 p95 与 bytes。 |
| `scripts/perf/gpui-release-frame-budget.mjs` | 唯一入口：校验 `rust-toolchain.toml` 的版本，`cargo test -p tterm-gpui --lib --release --no-run` 构建，从 cargo JSON 唯一取到 lib 测试二进制，`--list --ignored --exact` 要求恰好一项，再以 `--exact --ignored --nocapture --test-threads=1` 运行，最后核对“1 passed / 0 failed / 0 ignored”、用例自己的 `... ok` 行和 `P2-02 foreground frames:` 证据行。任一不符即非零退出。 |
| `scripts/perf/gpui-release-frame-budget.test.mjs` | 覆盖读取端的失败路径：改名、0 项、2 项、`FAILED`、缺结果行、缺证据行、构建消息里 0 个或 2 个可执行文件。已加入 `pnpm test`。 |
| `.github/workflows/check.yml` | 新增 `rust-frame-budget` 作业：`runs-on: macos-15`、`rustup show`、`swatinem/rust-cache`、执行上面这条命令。无 `if`、无 `continue-on-error`。 |
| `AGENTS.md`、`CONTRIBUTING.md`、`acceptance.md`、`decisions/019…`、`decisions/README.md` | 入口命令、失败判定、平台与测量范围；把 G2 的这一项指向门禁，并写明门禁不是桌面验收。 |

核对过实际承担 GPUI 校验的 workflow：只有 `check.yml` 跑 `cargo test --workspace`；`sudo-mode.yml` 只跑 sftp/sudo 的真实主机用例；三个发布 workflow 不跑测试。

### gpui 测试宏的 ignore 转发

`#[gpui_kit::test]` 来自 `gpui-pre-macros 0.3.11` 的 `test`。它把内层函数的属性取出，挂在生成的 `#[test] fn` 上（`outer_fn.attrs.extend(inner_fn_attributes)`），所以写在宏前面的 `#[ignore]` 落在真正被 libtest 发现的函数上。实测两条都成立：`cargo test --workspace` 中该用例显示为 `... ignored, measures wall-clock frames under 30 background parsers; ...`，gpui lib 套件为 `267 passed; 2 ignored`（改前为 268 passed; 1 ignored），其余覆盖与默认并发未变；门禁按完整名运行时，panic 位置与证据行都来自同一个 `pipeline/tests.rs` 用例。

### 环境与测量范围

macOS 27.2（`sw_vers`），Apple M4，10 核，32 GB，arm64；`rustc 1.99.1 (58cf8f9c3 2026-10-13)`；工作树基于 `a32fbb0d`。测量仍是原定义：12 帧预热、120 帧采样、100 ms 预热窗口、30 个后台解析器、2 ms 生产节奏；断言为后台不请求前台帧、累计 ≥ 30 MiB（31,457,280 B）、p95 ≤ 16.667 ms。三次运行都在同一台机器上独占执行，期间不跑其他任务。

### 三次本机独占 release 结果（预先定三次，全部保留）

| # | 基线 p50 / p95 | 30 会话 p50 / p95 | 累计确认 bytes | 帧窗口 | 窗口确认与速率 | 退出码 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 113.292 / 118.167 µs | 111.792 / 128.583 µs | 26,404,000 | 13.946 ms | 2,788,000 B（190.65 MiB/s） | 1 |
| 2 | 113.833 / 117.583 µs | 111.458 / 121.25 µs | 26,469,600 | 13.767 ms | 2,771,600 B（191.99 MiB/s） | 1 |
| 3 | 114.875 / 121.208 µs | 110.083 / 137.084 µs | 24,436,000 | 14.071 ms | 2,558,400 B（173.40 MiB/s） | 1 |

三次的 `snapshots=253 frames=253`；快照等待合计 2.376–2.619 µs，持锁合计约 6.93–6.98 ms，单次最大 84.125–132.916 µs。每条日志中 libtest 的结论都是 `FAILED. 0 passed; 1 failed; 0 ignored; 267 filtered out`，panic 信息为 `the measurement processed only N bytes across 30 background sessions`。

另有一次在同一台机器、同一命令下的开发期运行（诊断输出上移之前）：累计 26,141,600 B，同样在 30 MiB 断言处失败；因为当时 p95 没有打印，才把那一行移到断言之前。这次也保留在结论里。

### 为什么失败，以及没有动什么

release 下前台一帧约 0.11 ms，120 帧的采样窗口只有约 14 ms，加上 100 ms 预热，生产者按 2 ms 节奏只交出约 24–26 MB；dev 构建里窗口长得多，累计量才越过 30 MiB。窗口内速率是 173–192 MiB/s，远高于 30 MiB 所需，所以这次失败是“累计下限与很短的采样窗口耦合”，不是吞吐或帧预算不足：p95 只有 121–137 µs，距 16.667 ms 还有两个数量级余量。

按决策 019 第 7 条，本轮没有为了变绿去延长窗口、调生产节奏、改 2 ms 间隔、加预热或重试、降低 30 MiB 下限或 p95 预算，也没有改产品代码。门禁的真实状态是**红**：实现与证据完整，30 MiB 一项不达标。

### 负向验证（全部非零退出）

| 运行 | 结果 |
| --- | --- |
| `node scripts/perf/gpui-release-frame-budget.mjs --test-name <名字末尾少一个 t>` | `frame budget gate FAILED: expected exactly one ignored test named …, listed []`，退出码 1 |
| `--test-name thirty_background_parsers_keep_the_foreground_within_its_frame_budget`（只有函数名，没有模块路径） | 同上，`listed []`，退出码 1；libtest 的 `--exact` 按模块全名匹配，只写函数名会 0 项且自身退出 0，靠这一步才拦住 |
| `--test-name terminal_demo::pipeline::tests::thirty_hidden_sessions_parse_without_waking_the_foreground`（存在但不 ignored） | `listed []`，退出码 1 |
| 门禁正常路径（上面三次） | `FAILED. 0 passed; 1 failed` → 退出码 1 |

`--test-name` 只用于证明失败路径，CI 作业不传它。

### 复验记录

- `pnpm install --offline --frozen-lockfile`：Already up to date。
- `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`：通过；`pnpm test` 含新增的 11 项 perf 脚本用例；`check:size` 1496 个文件在限制内。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过（退出码 0），默认并发；gpui lib 为 `267 passed; 2 ignored`。
- 门禁：3 次独占 release 运行均退出 1，数值见上表。

### 结论与剩余决策

门禁本身已按要求独立、无条件、fail closed，本地与 CI 走同一入口；它目前不通过，不通过项是 30 MiB 累计下限，p95 预算有大量余量。本轮不把门禁调整成绿，也不宣称桌面验收完成。是否把采样窗口改成固定时长、或把累计下限改成窗口内速率，属于改变测量定义，需要决策人另开记录；在此之前 W0-05 与 G2 都保持未通过。Windows 同一项按决策 016 记为待补，Linux 推迟。
