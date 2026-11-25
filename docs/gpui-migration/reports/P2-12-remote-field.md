# P2-12 远程现场验收：SSH、tmux 与 neovim

## 原生窗口端到端结果（2026-11-25）

**Windows GPUI → 内网 Linux SSH → tmux 3.6 / Neovim 0.11.6：本场景通过。** 普通窗口手动密码登录已由操作者确认。本轮另开隔离原生窗口，以固定指纹、内存口令和 GPUI 自身的 SSH/PTY 通道执行测试，不使用系统 `ssh` 或录制回放。驱动向已连接的会话输入字节，不模拟物理键盘。截图使用同一窗口的正常绘制路径，未通过切换调色板强制全屏重绘。

- tmux：私有 socket 上创建两个窗格，分别显示左右标记；缩放右侧并刷新后只剩右标记，状态栏显示 `Z`；分离后回到主屏，关闭测试会话。
- Neovim：打开临时文件，插入、撤销、重做并保存退出；从远端文件读回新增行与原始行，随后 Shell 正常应答。最终帧未见窗格边框或编辑器残影。
- 最终验收程序退出码 `0`，9/9 个阶段通过，生成 12 张 PNG。原始本地证据：`target/gpui-real-tui-xch4ot_n/frames/report.json`，同目录查看 `03-tmux-split.png`、`04-tmux-zoom.png`、`07-nvim-edited.png`、`10-shell-restored.png`。该目录可能含测试环境信息，不随仓库提交。
- 复验入口：`cargo build -p tterm-gpui --features automation --example ssh_tui_field`。按下方步骤先在编号实例准备连接、独立核对并记录主机指纹。运行 `ssh_tui_field --output <空目录>`，由受信任的本地父进程将一次性密码作为一行标准输入送入，不将密码放入命令参数、环境变量或配置文件。

下面是此前回环、模型回放和第一次手动连接的记录。以上结论不覆盖窗口改变尺寸、系统剪贴板 OSC 52、断链恢复或整个 G2。

## 2026-11-25 内网实机续验

**远端 TUI 与模型回放：** 保留用户已登录的 GPUI 窗口，另用固定同一主机指纹的 OpenSSH 建立 120×30 PTY。在私有临时 socket 上向 tmux 3.6 发送 `Ctrl+B`、`%`，新窗格报告 `window_panes=2`；依次退出两个窗格。Neovim 0.11.6 在无配置、无交换文件模式下插入 `gpui-ok`，`:q!` 退出后 Shell 回应 `SHELL-RESPONDED:roundtrip`。将这次会话的原始字节回放到项目实际使用的 Rust 终端模型，四个快照（分屏、退出 tmux、编辑、退出 Neovim）均通过内容和主/备用屏断言。此测试未通过 GPUI 自身的 SSH 通道执行，也未观察原生窗口像素，不能签收桌面重绘或 G2。

**人工登录复核：** 重新启动同一个隔离的 Windows GPUI 实例后，用户从已保存连接入口手动输入密码，并确认已进入远程 Shell。该结果覆盖 GPUI 的手动密码登录；不证明此前自动输入失败的原因，也不代替 tmux 分屏、Neovim 编辑及退出后的画面验收。

在用户授权的 Linux 主机上，通过独立渠道核对 ED25519 指纹；固定该指纹的 OpenSSH 客户端用标准 `password` 成功登录，读得 tmux 3.6、Neovim 0.11.6。未安装软件或修改服务器配置。下方“未连接远程主机”指此前的回环批次。

同一主机的 Windows GPUI 原生窗口显示了相符的指纹，手动确认字符后出现标准 `Password:` 提示；自动输入提交后窗口显示 `SSH authentication failed`，未进入远程 shell。该结果不能判定是输入环节还是认证环节导致失败，也不签收 tmux、Neovim 或 G2。停止对该账户自动重试，下一步以人工键盘输入作对照。不记录地址、账户、密码或截图。

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
| 隔离连接配置                  | 可离线准备         | 使用下方的 debug 建档工具；生成连接不代表已经连接或通过现场验收                     |
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

- 从仓库根目录在**专用测试账户**下选一个从未用过的编号，先离线建档。以下 `192.0.2.1` 是文档保留地址，只用于验证选择器；实机验收时换成获授权的测试主机、端口和测试用户名，**不要传入口令或密钥**。两个命令使用同一个终端、临时目录和编号。

  ```powershell
  cargo build -p 'tterm-gpui' --bin 'tterm-gpui' --example seed_ssh_check
  if ($LASTEXITCODE -ne 0) { throw '构建失败。' }
  $fieldRoot = Join-Path (Resolve-Path target) ([guid]::NewGuid().ToString())
  $env:HOME = (New-Item -ItemType Directory -Path (Join-Path $fieldRoot 'home') -Force).FullName
  $env:TEMP = $fieldRoot
  $env:TMP = $fieldRoot
  $env:TTERM_GPUI_CHECK_INSTANCE = "1201"
  & .\target\debug\examples\seed_ssh_check.exe 192.0.2.1 22 field-test
  if ($LASTEXITCODE -ne 0) { throw '建档失败；不要启动客户端。若编号已使用，请换新编号。' }
  & .\target\debug\tterm-gpui.exe
  ```

  或在 Unix 客户端：

  ```sh
  cargo build -p 'tterm-gpui' --bin 'tterm-gpui' --example seed_ssh_check || exit 1
  field_root=$(mktemp -d) || exit 1
  mkdir -m 700 "$field_root/home" || exit 1
  export TMPDIR="$field_root" HOME="$field_root/home"
  export TTERM_GPUI_CHECK_INSTANCE=1201
  ./target/debug/examples/seed_ssh_check 192.0.2.1 22 field-test &&
    ./target/debug/tterm-gpui
  ```

- 终端逻辑尺寸固定为 120×30，默认主题，记录缩放、字体和渲染尺寸。不要用
  release 或系统 `ssh` 的结果代替 GPUI 会话。
- 建档只在全新编号实例中生成 `p2-12-remote`，不从个人数据库导入连接，不写入密码或预置信任。如果只验离线链路，打开“已保存的 SSH 连接”并核对名称、主机、端口后取消，不要点“打开”。若需重看已有的测试实例，只运行启动客户端的命令，不重新建档。

编号隔离的是应用数据库和凭据服务名，并不会屏蔽系统级 OpenSSH 信任配置。上述步骤使用专用本地账户、空 `HOME` 和新临时根；请在单独的终端中执行，结束后关闭该终端。离线工具不初始化系统钥匙串，而实际启动 GPUI 可能在该实例的凭据命名空间中写入数据。

### 远端

- 此档案使用标准 SSH `password` 认证，不保存口令，也不使用 SSH agent 或私钥。测试服务器可开启 `PasswordAuthentication`；若服务器只提供 keyboard-interactive/PAM，客户端会用同一次非回显输入继续。先通过独立可信渠道取得测试主机指纹；对不上就取消，不预填或跳过主机信任。

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
- 离线建档补上了测试连接来源，不替代生产 profile 编辑器或远程验收。
- G2 验收和 release 性能门禁；本报告没有运行完整测试套件或性能脚本。
