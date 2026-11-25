# P2-13 Unix Shell 现场验收：bash、zsh 与 WSL

日期：2026-11-25。执行环境：Windows x64 主机上的 WSL2 Ubuntu 26.04.1 LTS，
使用隔离的临时 HOME 和真实 PTY。**这是仓库真实 shell 钩子与终端模型层的
现场验证，不是物理 GPUI 桌面验收，也不是 SSH 远端验收。**

## 结论

**部分完成。** bash 5.3.9 与 zsh 5.9 的仓库真实钩子在隔离 PTY 中通过，
覆盖本机启动脚本 `unix/start.sh`、WSL 启动脚本 `wsl/launch.sh`、命令标记
开关、用户启动文件共存、含 `%` 的目录和真实终端模型解析。fish 在本机未安装，
PowerShell、SSH 远端和物理 GPUI 桌面未验证；现有
`real_shell_integration` 测试本次未运行。

| 范围 | 结果 | 说明 |
| --- | --- | --- |
| bash（`/usr/bin/bash` 5.3.9） | 通过 | 本机路径与 WSL 路径各 2 次运行（标记开/关），共 4 个捕获 |
| zsh（`/usr/bin/zsh` 5.9） | 通过 | 本机路径与 WSL 路径各 2 次运行（标记开/关），共 4 个捕获 |
| 真实终端模型解析 | 通过 | 8 个原始捕获全部按预期得到 4 条或 0 条命令标记 |
| fish | 未验证 | `command -v fish` 为空，未安装软件 |
| PowerShell | 未验证 | 本报告只覆盖 Unix shell，未启动 PowerShell 现场 |
| SSH 远端 | 未验证 | 未连接任何远程主机；WSL 不是 SSH |
| 物理 GPUI 桌面 | 未验证 | 没有运行 GPUI 窗口、概览标尺或键盘跳转 |
| 现有 `real_shell_integration` 测试 | 未执行 | 本次只静态核对测试与 CI 接线，没有运行测试 |

## 环境

| 项目 | 值 |
| --- | --- |
| Windows | 10.0.26300.10075（x64；WSL 内报告 `x86_64`） |
| WSL | 3.0.5.0；WSLg 1.0.79 |
| WSL 内核 | 6.18.58.1-1；`uname -r` 为 `6.18.58.1-microsoft-standard-WSL2` |
| 发行版 | Ubuntu 26.04.1 LTS，WSL2，State `Running`，默认发行版 |
| bash | `GNU bash, version 5.3.9(1)-release (x86_64-pc-linux-gnu)`，`/usr/bin/bash` |
| zsh | `zsh 5.9 (x86_64-ubuntu-linux-gnu)`，`/usr/bin/zsh` |
| fish | 未安装；`command -v fish` 无输出 |
| 辅助工具 | `python3 3.14.4`、`/usr/bin/script`、`/usr/bin/setsid` |
| 仓库路径 | WSL 内为 `/mnt/d/code/rust/tTerm` |

沙箱内直接执行 `wsl.exe -l -v` 会得到
`Wsl/EnumerateDistros/Service/E_ACCESS_DENIED`；经用户批准在沙箱外重跑后得到
上面的发行版信息。这是宿主的访问限制，不是仓库或 shell 集成的失败。

## 边界与合规

- 没有安装或升级任何软件；没有执行 `apt`、`brew` 或其他包管理操作。
- 没有读取或修改用户 home 下的 shell 启动文件、配置、历史或 SSH 数据。
  两次 shell 运行都使用 `/tmp/p2-13-field/` 下的临时 HOME，并设置
  `HISTFILE=/dev/null`。
- 没有连接、探测或登录远程主机；没有使用用户 SSH 配置、密钥或连接数据库。
- 没有修改仓库代码、测试、配置或其他文档；本报告是唯一写入的仓库文件。
- 没有执行 Git 命令、全量测试、`pnpm` 检查或性能门禁。
- 临时脚本、原始捕获和模型检查器只写在 `target/p2-13-field/` 与 WSL 的
  `/tmp/p2-13-field/`，不属于仓库内容。

## 被测路径与脚本

脚本对应关系以
`crates/tterm-core/src/terminal/shell_scripts.rs` 的
`UNIX_SCRIPTS` 为准：

| 安装后的路径 | 仓库源文件 | 用途 |
| --- | --- | --- |
| `unix/start.sh` | `unix/start.sh` | 选择 bash、zsh 或 fish 并以登录 shell 启动 |
| `wsl/launch.sh` | `wsl/launch.sh` | 恢复 WSL 标签的目录，报告 OS，再执行 `unix/start.sh` |
| `bash/init.bash` | `bash/init.bash` | 加载登录文件后接入 bash 提示符钩子 |
| `bash/report-cwd.bash` | `bash/report-cwd.bash` | 关闭标记时报告 OSC 7 目录 |
| `bash/prompt.bash` | `bash/prompt.bash` | 输出 OSC 133 提示符与退出状态 |
| `zsh/.zshenv` | `zsh/zshenv` | 接入 zsh 启动链 |
| `zsh/.zprofile` | `zsh/zprofile` | 接入 zsh 登录文件 |
| `zsh/.zshrc` | `zsh/zshrc` | 接入 zsh 提示符、目录和命令标记钩子 |
| `zsh/.zlogin` | `zsh/zlogin` | 恢复用户的 `ZDOTDIR` |
| `fish/tterm.fish` | `fish/tterm.fish` | fish 集成脚本；本机未执行 |

两条启动路径分别在
`crates/tterm-core/src/terminal/local_integration.rs`
和
`crates/tterm-core/src/terminal/shell_integration.rs`
中。前者把本机 bash、zsh、fish 改成经
`/bin/sh <config>/shell-integration/unix/start.sh` 启动；后者写入
`wsl/launch.sh` 与同一组 Unix 脚本，`wsl_args` 生成
`--cd <dir> -e sh ./wsl/launch.sh <cwd> <marks>`。本次两条路径都用 `sh`
直接驱动，没有经过 GPUI 窗口。

## 方法

1. 从仓库的 `shell_integration/` 目录按 `UNIX_SCRIPTS` 复制脚本到
   `/tmp/p2-13-field/staged`，统一写成 LF。重点是安装后的 zsh 文件名是带点的
   `.zshenv`、`.zprofile`、`.zshrc`、`.zlogin`，而源文件在仓库里没有前导点。
2. 为每个 shell 和模式建立临时 HOME，并创建 `100%dir`。bash 的
   `.bash_profile` 设置 `PROMPT_COMMAND=true` 并打印
   `P2-13-USER-BASH-PROFILE`；zsh 的 `.zprofile` 和 `.zshrc` 各打印一个标记，
   `.zshrc` 还清空 `precmd_functions` 与 `preexec_functions`，用来确认用户配置
   不会覆盖集成钩子。
3. 用 `pty.fork()` 启动 shell。WSL 模式在 staged 目录执行
   `sh ./wsl/launch.sh <home> <marks>`；本机模式在临时 HOME 执行
   `sh <staged>/unix/start.sh`。环境包含 `HOME`、`XDG_CONFIG_HOME`、
   `TERM=xterm-256color`、`SHELL`、
   `HISTFILE=/dev/null`、`LANG=LC_ALL=C.UTF-8` 和
   `tterm_marks`。
4. 依次向 PTY 写入 `cd '<home>/100%dir'`、打印 shell 版本的 `echo`、`false`
   和无参 `exit`，然后解析 OSC 133、633、7、777。
5. 把原始捕获交给 `target/p2-13-field/check_marks.exe`。该程序链接当前
   `target/debug/deps` 中真实的 `libtterm_terminal-*.rlib`，用
   80×24、单元格 8×16、scrollback 100 的配置喂给
   `terminal::Terminal`，再读取
   `snapshot().command_marks()`；它不是另一套模拟解析器。

运行的命令：

```powershell
wsl.exe -l -v
wsl.exe --version
wsl.exe -d Ubuntu --exec sh -lc '. /etc/os-release; printf "%s\n" "$PRETTY_NAME"; uname -r; bash --version | head -1; zsh --version; python3 --version; command -v fish || true'
wsl.exe -d Ubuntu --exec python3 /mnt/d/code/rust/tTerm/target/p2-13-field/check_unix_shells.py
```

模型检查：

```powershell
$rlib = (Get-ChildItem target\debug\deps\libtterm_terminal-*.rlib | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
$native = "$env:USERPROFILE\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\windows_x86_64_msvc-0.52.6\lib"
rustc --edition 2021 -L dependency=target\debug\deps -L "native=$native" --extern "tterm_terminal=$rlib" target\p2-13-field\check_marks.rs -o target\p2-13-field\check_marks.exe
Get-ChildItem target\p2-13-field\*.raw | Sort-Object Name | ForEach-Object { .\target\p2-13-field\check_marks.exe $_.FullName }
```

## 实际执行结果

8 次 PTY 运行的结果如下。`OSC 7` 一列的值均为
`file:///tmp/p2-13-field/<home>/100%25dir`，即目录中的 `%` 被正确转义为
`%25`；`OSC 777` 只在 WSL 模式出现，载荷为
`tterm-os;ubuntu;debian;Ubuntu`。

| shell | 模式 | marks | 133;A | 133;C | 133;D;0 | 133;D;1 | 633;E | OSC 7 | OSC 777 | 模型标记 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: |
| bash | 本机 `unix/start.sh` | 1 | 4 | 4 | 3 | 1 | 0 | 有 | 无 | 4 |
| bash | WSL `wsl/launch.sh` | 1 | 4 | 4 | 3 | 1 | 0 | 有 | 有 | 4 |
| bash | 本机 `unix/start.sh` | 0 | 0 | 0 | 0 | 0 | 0 | 有 | 无 | 0 |
| bash | WSL `wsl/launch.sh` | 0 | 0 | 0 | 0 | 0 | 0 | 有 | 有 | 0 |
| zsh | 本机 `unix/start.sh` | 1 | 4 | 4 | 2 | 2 | 4 | 有 | 无 | 4 |
| zsh | WSL `wsl/launch.sh` | 1 | 4 | 4 | 2 | 2 | 4 | 有 | 有 | 4 |
| zsh | 本机 `unix/start.sh` | 0 | 0 | 0 | 0 | 0 | 0 | 有 | 无 | 0 |
| zsh | WSL `wsl/launch.sh` | 0 | 0 | 0 | 0 | 0 | 0 | 有 | 有 | 0 |

原始捕获为
`target/p2-13-field/{bash,zsh}-{local,wsl}-marks{0,1}.raw`；WSL 侧的同一组
文件在 `/tmp/p2-13-field/` 下。

逐项观察：

- 标记开启时，两个 shell 都输出 OSC 133 A、C、D。bash 不输出 633 E，
  因为 bash 没有能在命令运行前看到命令行的钩子；这是设计差异，不是缺失。
- zsh 输出 4 条 `633;E`，其中包含 `633;E;false`，与
  `real_shell_integration_marks_commands_in_each_shell` 的断言一致。
- 标记关闭时，两个 shell 和两条启动路径都不输出任何 `133;` 或 `633;`，
  但仍报告 OSC 7 目录；WSL 路径仍报告 OSC 777。这与
  `real_shell_integration_reports_the_directory_with_marks_off` 的语义一致。
- 临时 bash `.bash_profile` 的 `P2-13-USER-BASH-PROFILE` 和 zsh
  `.zprofile`、`.zshrc` 的用户标记都出现在捕获中。即使
  `.bash_profile` 重新赋值 `PROMPT_COMMAND`、zsh `.zshrc` 清空
  `precmd_functions` 和 `preexec_functions`，集成钩子仍然工作。
- 模型检查的 8 个捕获全部通过：标记开启的 bash/zsh 各 4 条标记，退出码序列
  为 `0, 0, 1, 未闭合`；标记关闭的 4 个捕获都是 0 条。例如：

  ```text
  bash-local-marks1.raw marks=4 history=9
    prompt=25 output=Some(26) end=Some(26) exit=Some(0)
    prompt=26 output=Some(28) end=Some(29) exit=Some(0)
    prompt=29 output=Some(30) end=Some(30) exit=Some(1)
    prompt=30 output=Some(31) end=None exit=None
  zsh-local-marks1.raw marks=4 history=0
    prompt=2 output=Some(3) end=Some(3) exit=Some(0)
    prompt=3 output=Some(4) end=Some(5) exit=Some(0)
    prompt=5 output=Some(6) end=Some(6) exit=Some(1)
    prompt=6 output=Some(7) end=None exit=None
  ```

- 最后一个标记在无参 `exit` 后没有闭合，因为 shell 在下一个提示符前退出；
  每个 PTY 运行最后的 shell 退出码为 1，是前一条 `false` 的状态被无参
  `exit` 沿用。这两点都是夹具的预期收尾，不是钩子失败。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| bash、zsh、fish、PowerShell 的 shell 集成在本地与 SSH 中都工作；现有的 `real_shell_integration` 测试继续通过。 | 未验证 | bash/zsh 的本地脚本路径与 WSL 启动路径在隔离 PTY 中通过；fish 未安装，PowerShell 和 SSH 未执行，`real_shell_integration` 未运行。分项见下表。 |

| 分项 | 结果 | 证据 |
| --- | --- | --- |
| bash 本机 `unix/start.sh`，标记开 | 通过 | 4 次 A/C，`D;0` 3、`D;1` 1；模型 4 条标记，含 `exit=1` |
| bash 本机 `unix/start.sh`，标记关 | 通过 | 0 条 133/633；仍有 OSC 7；模型 0 条 |
| zsh 本机 `unix/start.sh`，标记开 | 通过 | 4 次 A/C，`D;0` 2、`D;1` 2，4 条 633 E（含 `E;false`）；模型 4 条 |
| zsh 本机 `unix/start.sh`，标记关 | 通过 | 0 条 133/633；仍有 OSC 7；模型 0 条 |
| bash WSL `wsl/launch.sh`，标记开/关 | 通过 | 与上表 bash 行相同，并额外有 OSC 777 `tterm-os;ubuntu;debian;Ubuntu` |
| zsh WSL `wsl/launch.sh`，标记开/关 | 通过 | 与上表 zsh 行相同，并额外有 OSC 777 |
| 用户启动文件共存 | 通过 | bash profile 标记、zsh zprofile/zshrc 标记均出现；重设 `PROMPT_COMMAND`、清空 zsh 钩子数组后仍输出标记 |
| 含 `%` 的目录 | 通过 | OSC 7 为 `file:///tmp/p2-13-field/<home>/100%25dir` |
| 真实终端模型解析 | 通过 | 8/8 捕获的 `command_marks()` 符合开/关预期 |
| fish 本地 | 未验证 | 本机没有 fish；没有安装软件 |
| PowerShell 本地 | 未验证 | 本次未启动 PowerShell |
| bash/zsh SSH 远端 | 未验证 | 未连接远程主机；WSL 路径不是 SSH |
| 现有 `real_shell_integration` 测试 | 未验证 | 未执行；测试与 CI 命令只做了静态核对 |

## 检查

本次没有改动代码，按任务范围只写报告；以下未运行的检查不作“通过”陈述。

| 命令/检查 | 结果 |
| --- | --- |
| `wsl.exe -l -v`（沙箱外） | Ubuntu，`Running`，Version 2 |
| `wsl.exe --version` | WSL 3.0.5.0，WSLg 1.0.79，Windows 10.0.26300.10075 |
| WSL 内版本命令 | Ubuntu 26.04.1 LTS；内核 `6.18.58.1-microsoft-standard-WSL2`；bash 5.3.9；zsh 5.9；Python 3.14.4；fish 未找到 |
| `python3 target/p2-13-field/check_unix_shells.py` | 8 次运行全部完成，结果与上表一致 |
| `check_marks.exe` 处理 8 个 `.raw` | 8/8 通过；标记开启 4 条，关闭 0 条 |
| `cargo test -p 'tterm-core' --lib -- --ignored real_shell_integration` | 未运行 |
| `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` | 未运行；本次没有前端或代码改动 |
| `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` | 未运行 |
| `node scripts/perf/gpui-release-frame-budget.mjs` | 未运行；不属于本卡现场验收 |
| Git 状态、日志或提交 | 未执行 |

`.github/workflows/check.yml` 在 `ubuntu-24.04` 上运行
`cargo test --lib -- --ignored real_shell_integration`，在 `macos-latest`
上运行两个标记用例。测试源码在
`crates/tterm-core/src/ssh/shell_integration/real_shell_integration_tests.rs`，
使用安装脚本写入临时 HOME，再在 `portable_pty` 上启动真实 shell。它们仍是
后续可重复运行的首选自动化证据；本次没有把它们的结果写成通过。

## 偏差

- **不是物理桌面。** 本次只启动 shell 和真实终端模型，没有 GPUI 窗口、
  概览标尺、命令跳转、输出选择或复制交互，因此不能签收 G2 或整页视觉。
- **WSL 不是 SSH。** 两条启动路径都覆盖了，但 WSL 是 Windows 上的本地发行版
  路径，不等于远端 SSH 的登录 shell；SSH 项仍是未验证。
- **zsh 源文件与安装文件名不同。** 仓库里的 zsh 文件没有前导点，安装器写出的
  是 `.zshenv` 等带点文件。第一次直接把 `ZDOTDIR` 指向未暂存的仓库目录时，
  zsh 没有读取 `.zshrc`；改为按 `UNIX_SCRIPTS` 暂存带点文件名后通过。这是
  测试装置的方法修正，不是产品缺陷，也没有修改仓库脚本。
- **fish 未安装。** 本机只有 bash 和 zsh；没有为了本卡安装 fish。
- **PowerShell 未执行。** 任务卡的验收句包含 PowerShell，但本报告的范围是
  Unix shell；Windows 上的 PowerShell 钩子仍待现场。
- **沙箱权限。** `wsl.exe -l -v` 在沙箱内返回 `E_ACCESS_DENIED`，在用户批准的
  沙箱外重跑成功；报告中的 WSL 数据来自后者。
- **没有提交。** 按本次约束没有执行 Git 命令，因此没有可记录的提交哈希；
  报告留待负责人提交。

## 遗留问题

1. **物理 GPUI 桌面验收未做。** 需要 macOS 或 Linux 上有图形会话的构建，
   验证「标记命令（Shell 集成）」与「本机 Shell 集成」两个设置、概览标尺
   标记、上/下一条命令跳转、选中命令输出后的复制，以及关闭标记后的对照。
   Windows 上本机集成行被编译期隐藏，不能用 Windows 桌面代替这一项。
2. **Windows WSL 标签未从 GPUI 端到端驱动。** 本次只验证了
   `wsl/launch.sh` 的 shell 行为；还需从 GPUI 的 WSL 入口确认
   `wsl.exe --cd ... -e sh ./wsl/launch.sh ...` 的目录恢复、OSC 777 图标/OS
   报告和标记显示。
3. **fish 未验证。** 需在预装 fish 的 Linux/macOS 测试机上重跑同一组开/关标记
   用例，并补一次用户 `config.fish` 共存检查。
4. **PowerShell 未验证。** 需在 Windows 上验证 PowerShell 与 PSReadLine 的
   A/C/D、OSC 9;9 目录和标记开关。
5. **SSH 远端未验证。** 需在专用测试主机上分别用 bash、zsh、fish 登录，
   确认 SSH 会话同样收到 A/C/D、633 E（非 bash）和 OSC 7；不能使用生产主机。
6. **自动测试未在本机运行。** 可在具备 bash、zsh、fish 和 `sshd` 的 Unix
   测试机上运行：

   ```sh
   cargo test -p 'tterm-core' --lib -- --ignored real_shell_integration
   ```

   其中 `real_shell_integration_over_sshd_...` 还要求可用的 `sshd`；CI 的
   Ubuntu runner 已覆盖该组合。该命令本次未执行，不能作为本报告的本机结果。

## 后续物理验收步骤

以下步骤用于把本报告的钩子层结果推进到 GPUI 桌面层；软件和测试主机由现场
负责人预先准备，本卡不安装任何东西。

### macOS / Linux 本机 Shell

1. 在仓库根目录用隔离配置启动 GPUI，避免读写日常配置：

   ```sh
   XDG_CONFIG_HOME="$(mktemp -d)" HOME="$(mktemp -d)" \
     cargo run -p 'tterm-gpui'
   ```

2. 在设置中打开「标记命令（Shell 集成）」和「本机 Shell 集成」，分别新建
   一个 bash、zsh 和 fish 的本地标签；记录设置重开后的值。
3. 在每个标签依次执行：

   ```sh
   false
   true
   mkdir -p "$HOME/100%dir" && cd "$HOME/100%dir"
   printf 'cwd=%s\n' "$PWD"
   ```

   确认概览标尺出现命令标记，`false` 与 `true` 的退出状态颜色不同，目录
   更新为含 `100%dir` 的路径；用上/下一条命令跳转，选中一条命令的输出并
   复制，粘贴核对内容。
4. 关闭标签再重新打开，确认从上次目录启动；把「标记命令」关闭后重复一次，
   确认没有新标记但目录仍更新。
5. 用临时 HOME 中的 `.bash_profile`、`.zshrc` 或 fish `config.fish` 写入一条
   可辨认的输出，确认用户文件仍被加载且不会覆盖集成钩子。

### Windows WSL 标签

1. 在 Windows 上启动同一 GPUI 构建，打开 WSL 标签，确认 `wsl/launch.sh`
   恢复上次目录，并显示 OS 报告（OSC 777 对应的图标或名称）。
2. 用 WSL 的 bash 与 zsh 各执行一次 `false`、`true` 和进入 `100%dir` 的
   `cd`，确认标记、退出状态和目录报告；重启标签后核对目录恢复。
3. 记录 WSL 版本、发行版和 shell 版本；若现场没有 fish，不把 fish 写成通过。

### SSH 远端

1. 使用一次性测试主机和非 root 测试账户，分别配置 bash、zsh、fish 登录 shell；
   不读取或复用个人连接。
2. 从 GPUI 的 SSH 入口打开每个 shell，重复上面的 `false`、`true`、`cd` 和
   关闭标记对照；记录远端 shell 版本、主机系统和网络延迟。
3. 保存截图、命令输出和实际时间；本报告中的 WSL 结果不能替代这一项。
