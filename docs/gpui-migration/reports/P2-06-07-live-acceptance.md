# P2-06 / P2-07：本地 PTY 与桌面验收

本批只补前一批[搜索对照](P2-07-search-parity.md)没有覆盖的链路。验收分三层：无窗口的生产视图接收真实 PTY 输出、交互 shell 在真实 PTY 中发出命令标记、人在专用 macOS 桌面操作系统输入法和指针设备。前两层不能代替第三层。

## 已独立复跑（macOS）

- **本地 PTY 到 GPUI 搜索再回到 shell：通过。** 在生产工作区的测试窗口向关闭回显的真实 `/bin/sh` 输入 `run`；shell 确认标准输入、输出都是终端，再输出两个命中，每处后跟 128 行填充。初始视口看不到任何命中；查找 `needle` 恰有两项，Enter 将视口从第一项移到第二项。Esc 后输入 `back`，由同一 shell 输出 `shell-received:back`。这验证了双向字节流、保留历史与焦点，不是 macOS 原生窗口测试。
- **本地交互 zsh 钩子：通过。** 临时 HOME 与 ZDOTDIR 下，在真实 PTY 加载仓内 zsh 启动钩子，依次实际运行 `true` 和 `false`。两次均读到 `633;E;<命令> → 133;C → 133;D;<退出码> → 133;A`，退出码分别为 `0`、`1`。不代表应用自动安装钩子、默认登录启动路径或真实 SSH 已验证。
- **仓库检查：通过。** 前端六项、Rust fmt、严格 Clippy、workspace 测试通过。最后单独运行 release 门：前台帧 p95 **0.183 ms**；独立 30 MiB 全部确认用时 **151.504 ms**。不以这些数据判定桌面手感。

从仓库根复跑两项新增验收：

```sh
cargo test -p tterm-gpui --lib terminal_demo::window::search_workspace_tests::live_pty::find_reveals_retained_live_pty_output_and_returns_input_to_the_shell -- --exact
python3 -I -B scripts/migration/check-zsh-command-marks.py
```

## 原生桌面操作卡（未执行）

使用专用测试账户的图形会话或 macOS 测试 VM，从仓库根启动：

```sh
TMPDIR="$(mktemp -d)" TTERM_GPUI_CHECK_INSTANCE="$$" cargo run -p tterm-gpui
```

上述命令仅用于 **debug 构建**；release 忽略实例号。临时目录和实例号隔离应用配置、数据库和凭据命名空间，**不隔离系统输入法、桌面焦点或 shell 的启动文件和历史**。不要在日常账户调用这一步，无需使用个人连接或打开真实 SSH 会话。

| 场景 | 操作与通过标准 |
| --- | --- |
| 系统拼音 | 在两个测试窗格中让右窗格输出两行「中文」并获得焦点。`⌘F` 后输入 `zhongwen`，不确认；确认候选窗锚在搜索框。第一次 Esc 只取消组字，不关闭搜索栏；再组字并提交「中文」后能找到这两行。第二次 Esc 返回右窗格，输入一个 ASCII 字符不回车，只有右窗格应收到。若检查精确计数，预置输出时不要让命令回显也包含查询词。 |
| 滚轮与触控板 | 在默认主屏运行 `seq 1 200; sleep 8; seq 201 400; printf 'END\n'`；等待期间向上翻，记住顶部行号。新输出到达后，只要历史未被淘汰，顶部行号不应变化或跳到底。停止输入后观察触控板惯性，再用滚动条拖到顶、到底，确认能看到 `END`。窄分屏调整大小后重试；滚轮和触控板分开记，缺设备不记通过。 |
| 真实窗口的标尺 | 在一次性的默认 zsh 会话启用本地 shell 集成和命令标记，依次运行 `true`、`false` 并输出足够多的行。核对成功/失败标记与实际命令位置、滚动及窗格归属。不要以手写 `printf` 输出 OSC 控制串代替 shell 钩子。 |

手测记录至少包含：版本、macOS 版本、输入源、物理输入设备或 VM 转发方式、实际观察与失败步骤。当前仓库环境只确认有开发 Mac，未确认有授权的独立图形会话。此卡不涵盖真实远程 SSH、其它 shell/平台，也不签收 [G2](../acceptance.md)。
