# P2-12 远程现场验收：SSH、tmux 与 neovim

日期：2026-11-25。执行环境：Windows x64 共享工作树，调试构建，提交
`0a5e330c`。本报告只准备 P2-12 的远程现场验收，并复跑本机可用的既有
回环夹具；真实远程 SSH、tmux、neovim 和物理桌面均未执行。

## 结论

**远程现场未验证。** 本机通过的是 GPUI 回环 SSH 的查询顺序、早期输出和
已保存连接入口回归，共 7 项；这些结果不能代替真实远程主机、真实网络、tmux、
neovim 或 G2 桌面验收。

| 项目                          | 结果               | 说明                                                                                |
| ----------------------------- | ------------------ | ----------------------------------------------------------------------------------- |
| GPUI 回环 SSH 查询回归        | 通过（本机）       | `saved_connections_tests` 过滤集 7 passed，0 failed；见“实际执行结果”               |
| 无凭据的本地 OpenSSH 隔离设施 | 存在，本机不可运行 | 需要 Unix `sshd`、`ssh-keygen` 和 `which`；本机没有 `sshd`                          |
| 真实远程 SSH 会话             | 未验证             | 未连接任何远程主机，未读取用户 SSH 配置、密钥、历史或连接数据库                     |
| tmux                          | 未验证             | 仓库没有 tmux 的远程 SSH 自动化夹具                                                 |
| neovim                        | 未验证             | 仓库没有 neovim 的远程 SSH 自动化夹具                                               |
| vim 回环夹具                  | 未运行             | 用例仅 Unix，且被 `#[ignore]`；本机为 Windows，没有 `/usr/bin/vim` 的 Unix PTY 现场 |
| 现场可执行性                  | 受阻               | 当前 GPUI 没有 profile 新建/编辑页面，必须先提供隔离测试 profile 的注入步骤         |
| G2 / 物理桌面                 | 未签收             | 本报告不覆盖桌面渲染、输入法、物理键鼠或平台矩阵                                    |

## 边界与合规

本次没有读取 `~/.ssh`、SSH 配置、密钥、shell 历史或用户连接数据库；没有登录或
探测任何真实主机；没有安装软件。复跑的回环夹具只在测试进程内绑定
`127.0.0.1:0`，使用临时生成的 SSH 主机密钥和一次性测试口令，不连接外部服务，
测试结束即释放监听。

本报告不把以下任一项当作远程现场通过：

- 回环 SSH 脚本夹具；
- 在 Windows 上运行的本地 `cmd.exe` 或本地 PTY；
- 系统 OpenSSH 客户端、`ssh` 命令行或 `TestSshd` 的 OpenSSH 结果；
- 本地 tmux 性能负载 `scripts/perf/workload.sh`；
- 真实主机上的单次“能显示”而没有记录查询、重绘、复制和退出行为。

## 现有设施盘点

| 设施                                                                                                                                     | 是否使用用户凭据                        | 能证明什么                                                                                                                                                                              | 不能证明什么                                                               | 本机状态                                         |
| ---------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------ |
| GPUI 回环 SSH 脚本夹具：`crates/tterm-gpui/src/terminal_demo/window/saved_connections_tests/server.rs`、`ssh_query.rs`、`early_query.rs` | 否；一次性测试口令和临时主机密钥        | 实际 GPUI 窗口经 SSH 服务打开一个连接和一个 shell；DA1、DSR、CPR、窗口尺寸、XTGETTCAP 等查询只回答一次且按字节顺序；BSU/ESU 内顺序；exec 成功前的早期输出；已保存连接选择和分屏不被重拨 | 真实远程 shell、真实 sshd、网络延迟、tmux、neovim、OSC 52 的真实桌面剪贴板 | 通过，7 项                                       |
| GPUI 回环 Vim 夹具：`crates/tterm-gpui/src/terminal_demo/window/saved_connections_tests/vim.rs`                                          | 否；一次性测试口令                      | Unix 上真实 `/usr/bin/vim` 经 PTY 和回环 SSH 编辑、保存、退出备用屏幕                                                                                                                   | neovim；Windows；真实远程主机                                              | 未运行；`#[cfg(unix)]` 且 `#[ignore]`            |
| 核心隔离 OpenSSH 夹具：`crates/tterm-core/src/ssh/test_sshd.rs`                                                                          | 否；临时 `authorized_keys` 和客户端密钥 | 在 `127.0.0.1` 启动临时 OpenSSH，供认证、跳板、shell 集成、监控、SFTP、ZMODEM、proxy 等核心测试使用；无需 root，不改系统配置                                                            | GPUI 远程验收；tmux/neovim；Windows 桌面                                   | 未运行；本机 `sshd` 不在 PATH，`ssh-keygen` 存在 |
| tmux 自动化夹具                                                                                                                          | 不适用                                  | 仓库中没有面向 SSH 远端的 tmux 测试                                                                                                                                                     | 任何 tmux 远程结论                                                         | 不存在                                           |
| neovim 自动化夹具                                                                                                                        | 不适用                                  | 仓库中没有 neovim 测试                                                                                                                                                                  | 任何 neovim 远程结论                                                       | 不存在                                           |
| Docker sudo 测试主机：`.github/workflows/sudo-mode.yml`                                                                                  | 不适用；专用测试容器                    | sudo 和 sudo-rs 的专用模式                                                                                                                                                              | P2-12 的 SSH/tmux/neovim                                                   | 未使用；与本次范围无关                           |

`TestSshd` 的用例均标记为 `#[ignore = "spawns a real sshd; ..."]`，默认的
`cargo test --workspace` 不会运行它们。`.github/workflows/check.yml` 在
Ubuntu runner 上安装 `openssh-server`，并在 Ubuntu/macOS 上按名称运行部分
`real_sshd` 用例；它没有运行 tmux、neovim 或 GPUI 的远程桌面验收。仓库也没有
面向 GPUI 的 Docker、testcontainers 或其他无用户凭据的远程 SSH 验收设施。
在具备 Unix `sshd` 的主机上，可另行运行
`cargo test -p 'tterm-core' --lib -- --ignored real_sshd`；结果只属于
核心 SSH/SFTP/跳板验证。Unix 现场还可以运行被忽略的 GPUI Vim 回环用例，但它仍
不是 tmux、neovim 或真实远程验收。

## 实际执行结果

以下命令从仓库根执行。GPUI 测试二进制在当前沙箱内即使 `--list` 也会以
`STATUS_DLL_INIT_FAILED`（`0xc0000142`）退出，因此在受控的沙箱外执行；这与
代码失败无关，也不把沙箱限制当成通过证据。

```sh
cargo test -p 'tterm-gpui' --lib saved_connections_tests -- --nocapture
cargo test -p 'tterm-gpui' --lib ssh_query -- --nocapture
cargo test -p 'tterm-gpui' --lib early_query -- --nocapture
```

| 命令/过滤集               | 结果                                      | 覆盖的测试                                                                                                                                                            |
| ------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `saved_connections_tests` | **7 passed，0 failed，0 ignored**；1.24 s | 浏览已保存连接不拨号、失败读取不打开标签、重复确认只打开一个标签并拨号一次、已保存连接认证一次且保留原分屏、非同步块中的查询顺序、同步块中的查询顺序、exec 前早期输出 |
| `ssh_query`               | **2 passed，0 failed**；0.47 s            | `core_and_model_queries_are_answered_once_and_in_order`、`synchronized_output_keeps_query_replies_in_wire_order`                                                      |
| `early_query`             | **1 passed，0 failed**；0.13 s            | `early_and_live_shell_output_share_query_state`                                                                                                                       |

环境记录：

- OS：Microsoft Windows NT 10.0.26300.0；PowerShell 7.6.9。
- Rust：`rustc 1.99.1`，`cargo 1.99.1`；GPUI debug 测试目标。
- `Get-Command sshd`：未找到；`Get-Command ssh-keygen`：找到。因此本机没有运行
  `TestSshd` 的真实 OpenSSH 用例，不把“设施存在”写成“本机通过”。
- `vim.rs` 在 Windows 上不编译进测试二进制，报告中的 7 项也不包含真实 Vim。

复跑的回环测试确实经过 GPUI 的已保存连接入口：`open_saved_profile_launch`
创建新标签、连接一次、打开一个 shell，并把模型的查询回复写回 SSH 通道。它验证
的是 GPUI 客户端和模型之间的协议路径，不是远端程序。

## 必需现场

远程验收必须在一个专用的测试账户和一次性测试主机上进行，不得复用个人连接。

### 客户端

- Windows 或 macOS 的 GPUI 原生窗口，使用当前仓库的 debug 构建和隔离实例：

  ```powershell
  $env:TTERM_GPUI_CHECK_INSTANCE = "1201"
  cargo run -p 'tterm-gpui'
  ```

  或在 Unix 客户端：

  ```sh
  TTERM_GPUI_CHECK_INSTANCE=1201 cargo run -p 'tterm-gpui'
  ```

- 终端逻辑尺寸固定为 120×30，默认主题，记录缩放、字体和渲染尺寸。不要用
  release 或系统 `ssh` 的结果代替 GPUI 会话。
- 当前 GPUI 只有“已保存 SSH 连接”入口，没有 profile 新建/编辑页面。现场必须
  预先在隔离 GPUI 数据库中准备一条一次性测试 profile（例如 `p2-12-remote`）。
  这条 profile 只能由测试负责人用隔离数据准备，不能从个人 Tauri 数据库复制；
  如果无法准备，远程验收应停下，不能用系统 `ssh` 绕行。

### 远端

- 一台专用的 Linux 测试主机（推荐）或 macOS 测试主机，运行真实 OpenSSH
  `sshd`；不要使用生产主机或用户日常账户。
- 非 root 测试账户，bash 或 zsh 登录 shell；测试主机上已有真实 `tmux` 和
  `neovim`，版本在报告中记录。软件由现场负责人在测试前准备好；本次没有安装任何
  软件。
- 网络建议记录实际 RTT、丢包和是否经 VPN；不要只写“远程”。建议至少覆盖一次
  50–150 ms 的交互延迟，但以记录的实际值为准。
- 主机密钥只用于该一次性主机；不要读取或改动用户 `known_hosts`、私钥或代理。

### 设置

- 记录“允许程序写剪贴板（OSC 52）”的当前值。
- 先做一次关闭该设置的对照，再做开启后的复制；两次都以客户端系统剪贴板为准。
- 记录终端主题，因为 OSC 10/11/12 的回复应来自当前活动主题，而不是固定字节。

## 手工步骤

以下步骤全部在 GPUI 窗口内完成；远端命令由 GPUI 的 SSH 会话执行。

### 1. 打开会话

1. 用上面的隔离实例启动 GPUI。
2. 从标题栏点击无障碍名称为“已保存的 SSH 连接”、显示为“SSH”的入口；对话框
   标题应为“已保存的 SSH 连接”。
3. 选择预置的 `p2-12-remote`，点击“打开”。不得先浏览或自动拨号；只有确认后
   才应出现一个新标签。
4. 处理一次性主机的主机密钥和认证提示。记录指纹、认证方式和提示是否遮挡终端。
5. 也可以从命令行执行
   `cargo run -p 'tterm-gpui' -- --ssh 'p2-12-remote'`；这是同一
   GPUI 入口的启动快捷方式，但它走命令行选择路径，不能替代一次对话框操作。

### 2. 基线

在新标签中依次执行并记录：

```sh
printf 'P2-12-BASELINE\n'
tmux -V
nvim --version | head -n 3
```

通过条件：命令输出完整，输入不重复，提示符位置正确；没有因查询等待而卡住。
任何字符重复、缺失、乱码或超过 5 秒不可输入即失败。

### 3. tmux

1. 用独立的 socket，避免碰用户的 tmux 会话：

   ```sh
   tmux -L p2-12-remote new-session -s p2-12
   ```

2. 在 tmux 中创建新窗口和左右分屏，分别运行 `seq 1 200`、`printf` 和
   `top` 后退出；切换窗口和窗格，改变 GPUI 窗口大小。
3. 在一个窗格中运行未闭合的同步块，并在 500 ms 后关闭：

   ```sh
   printf '\033[?2026hBSU-OPEN\n'
   sleep 0.5
   printf 'AFTER-BSU\n\033[?2026l'
   ```

4. 观察 `BSU-OPEN` 和 `AFTER-BSU` 的顺序，以及之后 tmux 的重绘。
   如果该 tmux 版本不转发 DECSET 2026，记录为未覆盖，回到不使用 tmux 的 shell
   重复同一段；不能把 tmux 自己的处理当成 GPUI 的超时结果。

通过条件：

- tmux 状态栏、窗格边框和内容完整重绘，没有重复或错位的查询回复字符。
- 未闭合 BSU 超过 150 ms 后，`BSU-OPEN` 最终可见，屏幕不永久停住；
  `AFTER-BSU` 到达后不重复显示旧内容。
- 改变窗口大小后，tmux 的行列数和光标位置一致，没有把查询回复当成用户输入。
- 退出 tmux 后回到原 shell，原提示符和滚动内容仍在。

### 4. neovim

1. 运行 `nvim --clean /tmp/p2-12-nvim.txt`，输入几行文字，执行 `:set number`、
   `:colorscheme` 和一次撤销、一次保存、`:q`。
2. 先用 `:checkhealth clipboard` 或 `:echo has('clipboard')` 确认 neovim 有可用的
   剪贴板 provider；没有时记录该子项未覆盖，不能用普通内部 yank 冒充 OSC 52。
3. 用 `v` 选择文本并执行普通 yank；再在允许 OSC 52 的设置下执行
   `:let @+ = 'P2-12-OSC52'`，或使用 neovim 已配置的系统剪贴板寄存器复制。
4. 关闭 OSC 52 设置，重复一次复制，检查客户端剪贴板未被改写。
5. 重新打开文件，确认保存内容和主屏幕恢复。

通过条件：

- 配色、行号、状态栏和备用屏幕正确；没有多出的 `R`、`c`、`n`、`rgb:` 或
  `XTGETTCAP` 响应文本。
- 退出 neovim 后主屏幕恢复，shell 提示符只出现一次。
- OSC 52 开启时客户端剪贴板只写入一次，内容与复制文本一致；关闭时不写入。
- 任何查询导致超过 5 秒不可输入、备用屏幕无法退出或字符损坏，均记为失败。

### 5. 断链和写回失败

1. 在会话中保持一个正在等待输入的远端程序。
2. 由测试主机侧临时断开该测试会话，或关闭测试 sshd 的连接。
3. 观察 GPUI 是否崩溃、是否重复写回查询回复、是否留下悬挂标签。

当前实现即使写回失败也可能静默丢弃这一条回复，且没有用户可见提示；本项先记录
实际行为，不把“没有崩溃”单独写成产品策略通过。若要判定写回失败处理通过，需
先确定是否提示、重连或重发，并另开任务卡。

## 通过/失败判定汇总

| 条件                  | 通过                                                         | 失败                                                |
| --------------------- | ------------------------------------------------------------ | --------------------------------------------------- |
| 连接入口              | 确认后才打开一个新标签，一次连接、一个 shell；原分屏不被重启 | 浏览即拨号、重复确认创建多个标签、认证后无 shell    |
| DA1/DA2/CPR/XTGETTCAP | 只在真实程序需要时回答一次；vim、tmux、neovim 没有多余字符   | 同一查询回答两次、回复乱序、屏幕上出现原始响应文本  |
| 窗口尺寸/CPR          | resize 后远端行列数和光标一致                                | 行列数不更新、光标漂移、查询回复被当成输入          |
| 同步块                | 未闭合 BSU 在超时后推进，后续 ESU 不重放                     | 屏幕永久停住、旧内容重复出现                        |
| tmux                  | 状态栏、分屏、重绘、缩放和退出均正常                         | 重复字符、错位、卡住、退出后主屏损坏                |
| neovim                | 编辑、保存、撤销、备用屏幕、颜色和复制均正常                 | 多余字符、备用屏幕残留、复制内容错误                |
| OSC 52                | 设置开启时只写一次系统剪贴板；关闭时不写                     | 关闭时仍写入、重复写入、程序读取剪贴板              |
| 远程网络              | 记录 RTT/丢包后仍能完成上述操作，无无限等待                  | 不记录网络条件，或用本地回环代替远程                |
| 证据                  | 有 GPUI 窗口截图或录屏、命令输出、远端版本和设置值           | 只有“看起来正常”的口头结论，或拿系统 `ssh` 结果代替 |

## 未验证项

- 真实 Linux/macOS 远端的 sshd、登录 shell、PTY 和网络延迟。
- tmux 的所有交互和整屏重绘。
- neovim 的 XTGETTCAP、颜色查询、CPR、备用屏幕和 OSC 52。
- 真实 vim 在 Windows 上的行为；Unix Vim 回环用例本次未运行。
- 真实桌面剪贴板、物理键鼠、输入法和多显示器缩放。
- 断链、重连和写回失败的用户可见策略。
- 当前 GPUI 没有 profile 新建/编辑入口；在提供隔离测试 profile 的可复现注入步骤
  之前，本报告的远程手工步骤只能作为待执行方案，不能算现场通过。
- G2 验收和 release 性能门禁；本报告没有运行完整测试套件或性能脚本。
