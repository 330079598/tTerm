# 019 30 会话帧预算是独立、必跑的 release 门禁

- 日期：2026-11-23
- 决策人：stone
- 状态：已决定。只把已存在的 30 会话帧预算用例移出默认并行套件，改成独立 release 门禁；不改负载、采样窗口、p95 预算（16.667 ms）、30 MiB 吞吐下限，也不改产品代码与 Cargo profile。

## 背景

[W0-05 第二轮](../reports/W0-05-validation-repairs.md#2026-11-23-第二轮6b8924fb-的-30-会话帧预算诊断)在未改代码上测量了 10 次隔离运行，只通过 5 次；默认并行的 `cargo test -p tterm-gpui --lib` 是 8 次过 4 次。原因不是前台承受不住 30 个后台会话，而是这个用例用墙钟时间测帧耗时，却和其余 268 个测试争同一批核：同套件里 30 个解析器把机器占满时反而压住了同级测试，腾出核以后同级测试又会抢占前台。并行套件的通过率描述的是调度，不是 30 个后台会话下的前台帧预算。W0-05 把“是否隔离该基准”列为需要决策的一步，本轮按已确认的结论实施。

## 决定

1. **只分离这一个用例。** `thirty_background_parsers_keep_the_foreground_within_its_frame_budget` 在 `crates/tterm-gpui/src/terminal_demo/pipeline/tests.rs` 里加上带理由的 `#[ignore]`。`#[gpui_kit::test]` 会把外层属性转发到它生成的 `#[test]` 函数上，所以普通套件真的不跑它；普通 `cargo test --workspace` 的默认并发、过滤条件与其余 267 项覆盖不变，不加 `--test-threads`、不拆套件、不改 profile。
2. **一个入口，本地与 CI 同一条路。** 仓库根脚本 `scripts/perf/gpui-release-frame-budget.mjs` 是唯一入口。它按 `rust-toolchain.toml` 校验 `rustc`，以 `-p tterm-gpui --release` 构建 lib 测试二进制（AGENTS.md 的 `--workspace` 约定在这里明确例外：只选性能专用包，不为一个用例重编整个 workspace 的 release），从 cargo 的 JSON 构建消息里唯一定位可执行文件，然后：
   - 用 `--list --format terse --ignored --exact <完整名>` 要求恰好列出这一个 ignored 用例；
   - 直接运行该二进制，参数固定为完整测试名、`--exact`、`--ignored`、`--nocapture`、`--test-threads=1`；
   - 只接受“恰好 1 项执行、0 failed、0 ignored”、该用例自己的 `... ok` 行和 `P2-02 foreground frames:` 证据行。
3. **失败必须闭死。** 用例改名或写错、过滤匹配 0 项或多于 1 项、没进 ignored 模式、跑成 0 项、断言失败、工具链与仓库锁定版本不一致、构建失败或超时，脚本都以非零退出。libtest 对“0 项匹配”和“全部通过”都返回 0，因此不能只看退出码；门禁以列出的名字与实际执行的 1 项为准。
4. **CI 独立作业，无条件、不吞失败。** `.github/workflows/check.yml` 新增 `rust-frame-budget`，`runs-on: macos-15`（固定系统版本），`rustup show` 安装仓库锁定的工具链，`swatinem/rust-cache` 只缓存 `target`，执行上面这条命令。没有 `if`、没有 `continue-on-error`、没有重试。核对过实际承担 GPUI 校验的 workflow：只有 `check.yml` 跑 `cargo test --workspace`，`sudo-mode.yml` 只跑 sftp/sudo 的真实主机用例，发布 workflow 不跑测试。
5. **测量范围写清楚。** 门禁只覆盖：release 构建、固定 macOS 15 runner（本机为 macOS 10 核 arm64）、30 个后台解析器、12 帧预热后 120 帧采样、100 ms 预热窗口、2 ms 生产节奏的现有负载；断言仍是 p95 ≤ 16.667 ms、原负载累计 ≥ 30 MiB、后台不请求前台帧。它不给其他平台、其他负载或其他采样窗口的结论。
6. **现有平台规则不变。** 按[决策 016](016-mac-windows-either.md)，门禁反映的是 macOS 这一端；Windows 同一项记为待补，Linux 推迟到 Linux 发布前。门禁变绿不等于桌面验收：它不替代 G2 的整机日常使用验收，也不改变 W0-05 的剩余项。
7. **不为变绿改动测量。** 出现失败时，保留 p95、bytes 与退出码，按真实失败记录；不得靠降低 p95 预算或 30 MiB 下限、延长或缩短采样窗口、改生产节奏、加预热、加重试或挑样本通过。

## 影响与风险

- 门禁第一次实现就在本机 3 次独占 release 运行中失败：p95 为 128.583、121.25、137.084 µs（预算内），累计确认 26,404,000、26,469,600、24,436,000 字节（低于 30 MiB）。原因是 release 下 120 帧只要约 14 ms，而生产者按 2 ms 节奏供数，窗口内累计量随窗口缩短而变小；窗口内速率仍有 173–192 MiB/s。按第 7 条保留真实失败，本轮不改窗口、节奏或下限。该结果的完整记录见 [W0-05](../reports/W0-05-validation-repairs.md#2026-11-23-第三轮独立-release-门禁)。
- 该用例在默认套件里仍然存在，重命名它会同时让门禁以“列出的名字不匹配”失败，不会静默变成“0 项通过”。
- GitHub 托管 runner 是虚拟机且与他人共享宿主机，墙钟结果会比自己独占的机器差；固定 `macos-15` 只能保证系统版本，不保证机器空闲。门禁变绿只能说明该 runner 上的这一次测量达到预算，不能替代本机独占复核。
- 30 MiB 下限与采样窗口耦合，release 下窗口越短越难达到。是否把窗口改成固定时长或把下限改成速率，属于改变测量定义，留给决策人另开记录；本轮不擅自处理。
