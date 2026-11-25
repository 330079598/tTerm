# P2-12：SSH 查询的应答来源与顺序

普通 CPR（`CSI 6 n`）询问当前光标的实际行、列。SSH 传输层没有终端网格：旧客户端沿用固定的 `CSI 1 ; 1 R`，但 GPUI 已有渲染同一会话的终端模型。对 GPUI，把这条查询原样交给模型，SSH 层不再代答。Tauri 适配器不改变，私有 DEC CPR（`CSI ? 6 n`）也不在此批变更范围。

## 应答分工

下表记录第一、二批的分工；第三批保留这些回复字节，把 GPUI SSH 的固定回复移到解析队列里，见下文。`CSI` 表示 `ESC [`。

| 查询 | 普通 GPUI SSH 会话 | 与 Tauri SSH 的关系 |
| --- | --- | --- |
| 普通 CPR `CSI 6 n` | 模型按当前网格返回 `CSI 行 ; 列 R`，坐标从 1 开始 | Tauri 继续由 SSH 层返回 `CSI 1 ; 1 R` |
| 私有 CPR `CSI ? 6 n` | SSH 层固定返回 `CSI ? 1 ; 1 R` | 未改，不作为真实位置的证据 |
| DA1 `CSI c`、DA2 `CSI > c` | SSH 层分别返回 `CSI ? 64;1;2;4;6;9;15;21;22 c` 和 `CSI > 0;276;0 c`，不送入模型 | 保留现有 SSH 身份；不能据此推断本地 PTY 的身份 |
| 状态 `CSI 5 n` | 第二批改由模型返回 `CSI 0 n`，与普通 CPR 走同一条写回路径 | Tauri 的 SSH 层继续返回同样的字节 |
| XTGETTCAP `Co` / `Ms` | SSH 层报告 256 色 / OSC 52 能力；未知能力走失败分支 | 保留旧 SSH 兼容答复，不代表已打开剪贴板权限 |
| 颜色、窗口尺寸 | SSH 层不代答；GPUI 路径由模型产生意图，再由会话写回 | 旧前端走 xterm 路径；本批不签收颜色或全部尺寸查询 |

## Windows 回环 SSH 验证

`core_and_model_queries_are_answered_once_and_in_order` 启动仅监听回环地址的 SSH 服务端，通过 GPUI 的已保存连接打开会话。进入交互 shell 后，依次发送分包 DA1、状态查询、文本区像素尺寸查询和普通 CPR。尺寸应答按当前网格与单元格大小计算；最后先将光标移到第 4 行第 7 列，再查询，服务端应仅收到 `CSI 4 ; 7 R`。

服务端核对该 shell 的完整输入字节，而非只搜索一段期望文本。第一批逐个发问；模型读到最后的可见标记后，客户端再发送结束标记，圈定计数范围。

## 第二批：同一数据包中的两条标准状态查询

在 `144c5e2f` 上增加用例后，服务端发一个包含 `CSI 4;7 H`、`CSI 6 n`、`CSI 5 n` 的包，收到的却是 `CSI 0 n`、`CSI 4;7 R`。原因是 SSH 层在交给模型之前就写出了状态应答。修复后，GPUI 将两条标准 DSR 原样交给同一模型；Tauri 的两条应答仍在 SSH 层。回归覆盖 CPR 在前和状态在前两个方向，要求服务端完整字节序列中两条应答各出现一次且顺序正确。这不改变 DA、XTGETTCAP 或私有 CPR 的应答策略。

## 第三批：跨应答方、建连早期输出与回环 Vim

旧路径收到「普通 CPR、DA1、状态、DA2、普通 CPR」同包请求时，实测服务端先收到了两个设备属性回复。现在 GPUI SSH 让同一解析队列按字节流顺序处理：遇到固定 SSH 能力查询先处理前面的显示数据和模型产生的回复，再发送固定回复；它不会将已回答的查询重复送入模型。Tauri SSH 保留原有回复路径；本地 PTY 不使用 SSH 能力识别器。

- **交错和分包：** 回环服务端对照完整输入转录，覆盖上述五条同包查询，以及私有 CPR、实际窗口尺寸、跨包的 XTGETTCAP `Co` 和普通 CPR；坐标不是固定的 `(1,1)`。普通客户端输入作为尾部界标，检查重复回复。
- **建连早期：** 测试让 SSH 服务端在集成 exec 的 `SUCCESS` 前发送文字、光标定位和查询，并在 `SUCCESS` 后补完设备属性请求。修复前跨边界 DA1 收到的是本地模型身份，修复后返回 SSH 身份；同时检查普通 CPR 为第 3 行第 9 列、前后文字各出现一次。服务端模拟集成握手，不部署真实远程脚本。
- **真实 Vim：** 单独运行下列 opt-in 用例。GPUI 通过回环 SSH 连到临时 PTY 的 `/usr/bin/vim`，使用临时 HOME，不加载个人配置、插件或历史。在备用屏编辑后用 `ZZ` 保存退出，检查真实文件内容和退出状态、生产模型返回主屏。

```sh
cargo test -p tterm-gpui --lib saved_connections_tests::ssh_query
cargo test -p tterm-gpui --lib saved_connections_tests::early_query
cargo test -p tterm-gpui --lib terminal_demo::window::saved_connections_tests::vim::vim_edits_saves_and_restores_the_primary_screen_over_ssh -- --exact --ignored --test-threads=1
```

协调者在 macOS 独立复跑了上述三个用例（每个实际执行一项）；前端六项、Rust 格式、严格 Clippy 与 workspace 全量测试通过。最后单跑 release 门：30 个后台会话时前台帧 p95 为 191.833 µs；另起 30 个会话处理并确认 30 MiB，用时 151.445 ms。此门不专门测量 SSH 吞吐，也不代表桌面性能验收。

**仍待验证：** 同步更新期间延迟的模型回复、剩余协议矩阵（如 DECRQM、XTVERSION、OSC 颜色）、写回失败行为及 tmux/neovim。回环 Vim 不等同远程实机、物理桌面或 G2 签收。

### 本批本机验收（2026-11-25）

在基于 `1d896e9c` 的工作区、macOS arm64（10 核，Rust 1.99.1）上，前端六项检查、Rust 格式检查、工作区严格 Clippy 与测试均通过。另行单跑的回环 Vim 用例通过；它不在默认工作区测试中执行。

独立 release 性能门禁的两项测量均通过，未调整负载或阈值：30 个后台会话下前台帧 p95 为 **0.192 ms**（预算 16.667 ms，采样窗口内 30 个会话均有进展）；吞吐用例在 **151.445 ms** 内应用并确认全部 30 MiB（198.09 MiB/s，时限 500 ms）。这些数据来自本机测试夹具，不是桌面渲染或与 Ghostty 的对照测试。
