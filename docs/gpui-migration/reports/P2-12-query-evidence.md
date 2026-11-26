# P2-12 查询唯一应答证据（G2 收口补强）

日期：2026-11-26。分支：`migrate/P2-12-query-evidence`，基线 `5aeba21e`。
环境：macOS arm64，rustc 1.99.1。

本报告只补 G2 核查要求的“同一查询只回复一次”端到端计数证据。没有修改
SSH、终端模型或 Tauri 的协议实现，没有读取真实用户连接，也没有启动正式数据
目录；测试使用现有的回环 SSH 夹具。

## 结论

现有 `saved_connections_tests::ssh_query`、`early_query` 与同步输出用例已经对
**完整服务端收包**做等值断言，顺序和重复都会被检查；但 G2 核查明确要求不能只
由顺序推断数量。新增
`saved_connections_tests::ssh_query::each_query_kind_is_answered_exactly_once`
后，同一个回环 SSH 会话的完整交互 shell 收包中，DA1、DA2、CPR、DSR、OSC 10/11/12
与 XTGETTCAP `Co`/`Ms` 的精确回复均只出现一次。测试同时保留完整转录等值断言，
因此也排除了未被计数的额外回复字节。

## 查询与应答方

`CSI` 表示 `ESC [`，`BEL` 表示 `0x07`。

| 查询 | 请求字节 | 唯一应答方 | 精确回复 | 收包计数 |
| --- | --- | --- | --- | ---: |
| DA1 | `CSI c` | SSH 固定应答器（`ssh_query_handler::ordered`） | `CSI ? 64;1;2;4;6;9;15;21;22 c` | 1 |
| DA2 | `CSI > c` | SSH 固定应答器 | `CSI > 0;276;0 c` | 1 |
| CPR | `CSI 6 n` | GPUI 终端模型，按当前网格 | `CSI 4;7 R`（先把光标移到第 4 行第 7 列） | 1 |
| DSR | `CSI 5 n` | GPUI 终端模型 | `CSI 0 n` | 1 |
| OSC 10 前景色 | `OSC 10 ; ? BEL` | GPUI 终端模型，默认调色板 | `OSC 10 ; rgb:d7d7/dddd/e8e8 BEL` | 1 |
| OSC 11 背景色 | `OSC 11 ; ? BEL` | GPUI 终端模型，默认调色板 | `OSC 11 ; rgb:1b1b/1d1d/2323 BEL` | 1 |
| OSC 12 光标色 | `OSC 12 ; ? BEL` | GPUI 终端模型，默认调色板 | `OSC 12 ; rgb:7979/d8d8/ffff BEL` | 1 |
| XTGETTCAP `Co` | `DCS + q 436f ST` | SSH 固定应答器 | `DCS 1 + r 436f3d323536 ST` | 1 |
| XTGETTCAP `Ms` | `DCS + q 4d73 ST` | SSH 固定应答器 | `DCS 1 + r 4d73=5c455d35323b25703125733b25703225735c303037 ST` | 1 |

同一协议族的其余查询没有放进这次新增计数用例，避免扩大改动范围：

| 查询 | 现有证据 | 说明 |
| --- | --- | --- |
| 窗口尺寸 `CSI 14 t` | `core_and_model_queries_are_answered_once_and_in_order` | 按模型实际网格和单元格大小生成回复，完整转录等值断言；未单独计数 |
| 私有 CPR `CSI ? 6 n` | 同一顺序用例与同步用例 | SSH 固定回复 `CSI ? 1 ; 1 R`，完整转录等值断言；未单独计数 |
| DECRQM（私有模式报告） | `pipeline` 的私有模式与同步顺序用例 | 由模型产生；不在 G2 这次最小查询清单内 |
| XTVERSION | 未纳入本轮 | 当前固定应答器没有为它定义回复；本轮未改协议，仍需真实应用现场判断 |

## 测试如何计数

`count_query_script` 在一个由普通输入触发的包中发送：

1. `CSI 4;7 H`，把光标移到第 4 行第 7 列；
2. CPR、DSR、DA1、DA2；
3. OSC 10、11、12 颜色查询；
4. XTGETTCAP `Co` 和 `Ms`；
5. 随后发送 `P2-12-COUNT-END` 栅栏，确认客户端已应用服务端输出。

测试等待栅栏出现在模型输出中，再等待 100 ms 稳定窗口并调用
`run_until_parked`，然后读取服务端记录的整个交互 shell 输入。`reply_count` 用
非重叠字节窗口统计每种精确回复；每个标签先打印一行
`P2-12 query=... responder=... count=...`，再断言计数为 1。完整收包还必须等于
“栅栏输入 + 全部预期回复”，所以重复、缺失和额外字节都会失败。

服务端脚本只监听 `127.0.0.1`，认证和主机密钥都是夹具生成的一次性值；没有使用
真实连接或正式数据目录。

## 命令与结果

最终定向运行：

```sh
cargo test -p tterm-gpui --lib \
  terminal_demo::window::saved_connections_tests::ssh_query::each_query_kind_is_answered_exactly_once \
  -- --exact --nocapture
```

结果：`1 passed; 0 failed`，用时约 0.30 s。证据输出：

```text
P2-12 query=CPR responder=model count=1
P2-12 query=DSR responder=model count=1
P2-12 query=DA1 responder=SSH count=1
P2-12 query=DA2 responder=SSH count=1
P2-12 query=OSC 10 foreground responder=model count=1
P2-12 query=OSC 11 background responder=model count=1
P2-12 query=OSC 12 cursor responder=model count=1
P2-12 query=XTGETTCAP Co responder=SSH count=1
P2-12 query=XTGETTCAP Ms responder=SSH count=1
```

第一次用短名过滤实际执行 0 项，不计为测试；改用完整测试名后，第一次运行发现
测试里把 DA1/DA2 写在 CPR/DSR 之前，而实际线序是先应用光标移动和状态查询再写
固定回复。只修正了测试的预期顺序，没有改生产代码；随后一次运行通过。
这 9 项的顺序不作为唯一证据，逐项计数断言才是本报告新增的签收依据。

检查命令：

```sh
cargo fmt --all --check
cargo clippy -p tterm-gpui --all-targets -- -D warnings
```

两项均通过。本次没有复跑完整的 `ssh_query`、`early_query` 或同步输出目录；这些
用例未被修改，且新增用例只增加计数断言。

## 已有覆盖与边界

- `core_and_model_queries_are_answered_once_and_in_order` 继续检查 DA1、状态、
  窗口尺寸、非原点 CPR、DA2、私有 CPR、XTGETTCAP `Co` 和同步包顺序的完整转录。
- `early_and_live_shell_output_share_query_state` 继续检查建连早期文字、CPR 与
  跨 exec 边界 DA1 各出现一次。
- `synchronized_output_keeps_query_replies_in_wire_order` 继续检查 BSU/ESU 内固定
  回复和模型回复的线序，并在最终栅栏后做完整转录等值断言。
- 这些回环测试证明客户端字节层面的唯一应答，不等于真实 tmux、neovim、远程主机
  或物理桌面的行为。

## 仍未验证

以下都是独立的人工或远程现场项，不能用本次回环计数替代：

- 真实远程 tmux 和 neovim 中不出现多余字符或重复应答；需要真实 SSH 主机和交互
  现场。
- 断链或写回失败后的用户可见行为；当前写回失败策略仍需产品决定和真实断链验证。
- 物理桌面渲染、窗口交互、平台矩阵和 G2 整机验收。
- 本次新增用例只覆盖上面表内的九类查询；窗口尺寸、私有 CPR、DECRQM 与
  XTVERSION 仍按各自现有证据或未验证状态记录。