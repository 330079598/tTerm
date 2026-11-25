# P2-12 收尾：未关闭同步块、颜色查询与写回失败核对

日期：2026-11-25。环境：Windows x64 共享工作树，调试构建。上一份
[SSH 查询顺序报告](P2-12-ssh-query.md)留下的四项是：同步块一直未关闭时的超时、
颜色查询及其余矩阵、写回失败处理、tmux/neovim/远程实机。本报告只签收本机可重复
验证的代码和回归；远程实机、真实 tmux/neovim 与物理桌面仍为未验证。

## 结论

| 项目                     | 状态             | 证据                                                            |
| ------------------------ | ---------------- | --------------------------------------------------------------- |
| 未关闭同步块超时         | 已实现并回归     | 模型两个用例、GPUI 解析线程一个用例；既有五项目录内同步顺序用例 |
| 颜色查询                 | 已实现并回归     | OSC 4/10/11/12 各自只由模型回答一次，同步块内保持字节顺序       |
| 写回失败                 | 已审计，未改行为 | 两条路径的失败语义已定位；重试/提示策略仍需产品决定和远程验证   |
| tmux / neovim / 远程实机 | 未验证           | 本机没有可重复的远程现场，不能用回环夹具代替                    |

没有修改 release 负载、采样窗口、16.667 ms 预算、500 ms 时限或 30 MiB 总量；
没有改 `status.md`、`dispatch.md`，也没有改 P4 设置代码。

## 未关闭同步块超时

**问题。** `vte` 在 BSU（DECSET 2026）时记录 150 ms 的
`SYNC_UPDATE_TIMEOUT`，但只有 Alacritty 自己的 event loop 会检查它。GPUI 的
解析线程只处理输入队列，从未调用 `Processor::stop_sync`。因此一个中断的整屏
重绘会一直隐藏，直到 2 MiB 的 `SYNC_BUFFER_SIZE` 上限被迫刷新；期间屏幕上的
旧内容、模型查询回复和 SSH 固定回复都停在缓冲区里。

**实现。**

- `crates/tterm-terminal/src/terminal/sync_output.rs`：新增
  `synchronized_output_deadline` 与 `flush_expired_synchronized_output`。后者在
  到期后调用 `Processor::stop_sync`，用和普通写入相同的 `HistoryRelease`
  处理区域、命令标记与 `OrderedReplies`，再应用缓冲区里的模型查询回复。
- 超时值直接来自 `vte` 的 `sync_timeout()`，没有另立阈值。空缓冲区也会记录
  截止时间，所以“只有 BSU、随后长时间无输出”的会话不会把下一段输出继续扣住。
- `crates/tterm-gpui/src/terminal_demo/pipeline.rs`：解析线程缓存
  模型返回的截止时间；进入下一次读取前若已到期，先刷新再处理新数据，避免持续
  输出把时间耗过去。等待时取同步截止时间与既有鼠标前台检查截止时间中较早者；
  到期走同一个 `Applier`，固定回复仍按 `DeferredReplies` 的顺序写回。
- 行为仍是“整块应用”：到期前不暴露半帧；到期后缓冲区的文字和回复按原始字节
  顺序出现，不会为了回答查询提前显示半帧。
- 到期后再到达的固定回复（测试里用 DA1）立即写回一次，不会被旧同步块延迟，
  也不会补发。
- 解析线程在会话已死时直接清空缓存的截止时间；即使模型已经自行结束同步块，
  超时回调也会重新读取截止时间，过期值不会继续唤醒队列。
- 同步超时提前调用 `settle()` 时，其内部 `begin_check_at` 在截止时间前返回但不
  消耗它；新增断言确认 200 ms 检查仍按原计划调度。

**权限与重放。** 超时不重跑 `Osc52Scanner` 或命令标记扫描。OSC 52 store 在原始
写入时只生成一次，模型缓冲区只在 `stop_sync` 解析一次。新增回归把一个 OSC 52
store 放在未关闭的同步块里，确认超时刷新和后续输出都没有再次交付它；没有发现
新的权限或重放问题。

为了保持单文件规模，同步超时逻辑放在新子模块，`terminal.rs` 只增加
`mod sync_output;`。写这份报告时该文件为 798 行，`node scripts/check-file-size.mjs`
报告 1565 个文件均在限制内。

## 颜色查询

SSH 层继续只负责固定能力查询：`crates/tterm-core/src/terminal/ssh_query_handler.rs`
里的 `process_ssh_output_for_ui` 把 OSC 10/11 原样交给显示路径，不代答。GPUI
会话由模型产生 `Intent::ColorRequest`，`terminal_demo/intents.rs` 用
`TerminalPalette::default()` 回答；因此答案来自实际绘制使用的默认主题，
不是固定字节。

本机回归覆盖：

- OSC 4 索引色、OSC 10 前景、OSC 11 背景、OSC 12 光标各自只回答一次，回复顺序
  与请求一致。
- 同样的查询放在 BSU/ESU 内时，颜色回复和固定 DA1 回复仍按请求字节顺序出现，
  不因“模型回复”与“SSH 固定回复”的来源不同而交换。
- 分包覆盖每个字节切分和几种更大的块大小。

默认主题的精确回复已写进测试：
`ESC ]4;0;rgb:1616/1b1b/2222 BEL`、`ESC ]10;rgb:d7d7/dddd/e8e8 BEL`、
`ESC ]11;rgb:1b1b/1d1d/2323 BEL`、`ESC ]12;rgb:7979/d8d8/ffff BEL`。

## 写回失败：审计结论，未改行为

**GPUI 路径。** 解析线程把模型意图和固定回复交给
`crates/tterm-gpui/src/terminal_demo/session.rs` 的写入闭包，闭包执行
`let _ = write_services.write_pty(&key, &bytes)`。写失败时：

- 模型已经应用了对应输出；失败只影响这一条回复，没有本地屏幕、滚动区或排序
  队列损坏。
- 没有重试，也没有把同一条回复再写一次；`DeferredReplies` 在模型到达标记时
  已经取走该项，不会留下悬挂条目。
- 远端程序可能一直等这条回答，直到它自己的超时或重新连接；当前没有用户可见
  提示，也没有把失败升级为会话错误。这是一个策略缺口，不是本批能靠单元测试
  签收的产品行为。

**Tauri 旧路径。** `process_ssh_output_for_ui` 在写回失败时返回 `io::Error`；
`crates/tterm-core/src/ssh/client/reader.rs` 记录错误后把当前数据包当作
原始输出回退。审计发现：如果失败发生在跨包查询的完成包上，上一包留下的
`pending` 前缀可能已从显示路径丢失；GPUI 的 SSH 会话在 `owns_queries` 时根本不
走这条函数，所以没有在本批改它。修改回退语义会影响 Tauri 行为，且需要真实链路
验证，未纳入本次代码改动。

因此写回失败目前是“已定位、未签收”：没有为了变绿而吞掉错误或伪造成功。

## 测试命令与结果

以下均在本工作树执行；GPUI 测试二进制在当前沙箱内会以
`STATUS_DLL_INIT_FAILED` 退出（连 `--list` 都失败），所以 GPUI 命令在沙箱外运行，
既有无关测试同样受影响。

```sh
cargo test -p tterm-terminal --lib
cargo test -p tterm-terminal --lib synchronized_update
cargo test -p tterm-core --lib ssh_query_handler
cargo test -p tterm-gpui --lib color_queries -- --nocapture
cargo test -p tterm-gpui --lib an_unclosed_synchronized_update_is_flushed_at_its_deadline -- --nocapture
cargo test -p tterm-gpui --lib synchronized_output -- --nocapture
cargo test -p tterm-gpui --lib private_mode -- --nocapture
cargo test -p tterm-gpui --lib marker_aliases_and_cancellations_keep_their_real_reports
cargo check -p tterm-gpui --lib
cargo clippy -p tterm-terminal -p tterm-gpui --lib -- -D warnings
node scripts/check-file-size.mjs
```

| 命令                                            | 结果                                                 |
| ----------------------------------------------- | ---------------------------------------------------- |
| 模型 lib                                        | 397 passed，0 failed                                 |
| 模型 `synchronized_update`                      | 2 passed（未关闭缓冲回复、空缓冲区超时）             |
| core SSH 查询处理器                             | 24 passed（含 `ordered` 子模块）                     |
| GPUI `color_queries`                            | 2 passed（OSC 4/10/11/12、同步块内顺序）             |
| GPUI 同步超时用例                               | 1 passed，0.17 s（含 150 ms 截止时间与超时后的 DA1） |
| GPUI 超时刷新不重放 OSC 52                      | 1 passed（块内 store 只交付一次）                    |
| GPUI 鼠标 settle 截止时间                       | 1 passed（同步超时提前 settle 不改原截止时间）       |
| GPUI `synchronized_output`                      | 5 passed，含回环 SSH 的既有同步顺序用例              |
| GPUI `private_mode` / 标记别名                  | 各 1 passed                                          |
| `cargo check -p tterm-gpui --lib`               | 通过                                                 |
| 两个 crate 的严格 Clippy（`--lib -D warnings`） | 通过                                                 |
| `node scripts/check-file-size.mjs`              | 1565 files within their limits                       |

报告时完整 `cargo test -p tterm-gpui --lib` 的结果是 **419 passed / 3
failed / 11 ignored**。三个失败都与 P2-12 无关：两个 restart 用例在共享工作树里无法创建
测试数据库目录（`Access is denied`），一个 scrollback 设置页用例断言
`is_save_pending()`，属于并行进行的 P4 设置工作。本报告没有把这些失败算作通过。

### 分支任务期间的仓库级检查（最终复验见[状态页](../status.md)）

- `cargo test -p tterm-core --lib ssh_query_handler` 在并行设置工作修复合入
  后通过：24 passed，0 failed。先前同一命令因 `page_tests.rs` 的
  `TerminalPageSettings` 初始化缺字段而无法编译测试目标，本报告没有把那次
  失败算作通过。
- `cargo fmt --all --check` 仍报告一个非本卡文件：
  `crates/tterm-core/src/config/terminal_font_file/page_tests.rs`。
  本卡涉及的四个 Rust 文件已用 `rustfmt --edition 2024` 格式化；没有再改并行工作的
  文件来让全仓格式检查变绿。
- 上述是并行实现期间的结果，不作为整体验收依据。随后协调者在最终工作树复验了完整 workspace 和独立性能门禁，均已通过；G2 仍未签收。

## 仍需人工或远程验证

| 项目                   | 为什么本机不能签收                                                                  | 需要的现场                          |
| ---------------------- | ----------------------------------------------------------------------------------- | ----------------------------------- |
| tmux                   | 需要真实 tmux 在 SSH 远端运行并观察是否有重复字符、错位回复；回环脚本只覆盖字节协议 | 远程 SSH + tmux，交互与整屏重绘     |
| neovim                 | 需要真实 neovim 的 XTGETTCAP、颜色与 CPR 组合；回环 Vim 用例不等于 neovim           | 远程 SSH + neovim，编辑、复制、退出 |
| 远程实机               | 本机没有用户可用的远程主机、网络延迟与真实 shell 集成                               | 真实 SSH 主机，含断线/重连          |
| 写回失败的用户可见行为 | 需要决定是否提示、是否重连、是否重发；单元测试无法替代真实断链                      | 人为断链或远端关闭通道              |
| 物理桌面 / G2          | 本报告只有 Windows 调试构建的单元与回环结果                                         | 桌面渲染、窗口交互、平台矩阵        |

回环 SSH 测试、模型单元测试和解析线程测试都不等同于远程实机、真实 tmux/neovim、
物理桌面或 G2 验收。
