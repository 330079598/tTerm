### 准备命令与原始输出

以下命令从 `D:\code\rust\tTerm-P0-01` 运行，宿主为 Windows 11。机器规格沿用 W0-03 的 `environments.md` Windows 一节，本轮没有重新采集硬件信息。

确认分支与工作区的命令：

```powershell
git status --short
git branch --show-current
```

原始输出（status 为空）：

```text
migrate/P0-01-perf-baseline
```

WSL 可达性检查：

```powershell
wsl.exe -d Ubuntu -- pwd
```

沙箱内首次执行退出码为 1，输出（工具返回 UTF-16 文本，以下去掉显示用的 NUL 字符）：

```text
Access is denied.
Error code: Wsl/Service/E_ACCESSDENIED
```

同一命令申请提升权限重跑后退出码为 0：

```text
/mnt/d/code/rust/tTerm-P0-01
```

随后尝试把外部基准工具克隆到本 worktree 的 `target/`，没有在主仓库或其他 worktree 写文件：

```powershell
git clone https://github.com/alacritty/vtebench target/perf-tools/vtebench
```

退出码为 1，原始输出：

```text
Cloning into 'target/perf-tools/vtebench'...
fatal: unable to access 'https://github.com/alacritty/vtebench/': schannel: AcquireCredentialsHandle failed: SEC_E_NO_CREDENTIALS (0x8009030e) - No credentials are available in the security package
```

执行环境要求网络相关失败必须以 `require_escalated` 重跑相同命令；该规则优先于本轮任务的失败即停止要求。重跑上述同一命令后退出码为 0，原始输出：

```text
Cloning into 'target/perf-tools/vtebench'...
```

只读核对检出版本：

```powershell
git -C target/perf-tools/vtebench rev-parse HEAD
```

原始输出：

```text
ead80032e57dee2e75f0b51f2ea67528647d9944
```

已阅读该检出的 README：它说明 vtebench 测量 PTY 读取性能，不能替代帧率或延迟测量。尚未编译、运行或下载其构建依赖。没有更改 Git TLS 配置、下载压缩包、切换下载方式或在 WSL 中再次尝试克隆。

### 本轮恢复后的工具链检查

2026-11-08，在同一 worktree 用提升权限命令检查 WSL Ubuntu 的登录 shell：

```powershell
wsl.exe -d Ubuntu -- bash -lc 'command -v cargo; command -v rustc; rustc --version; cargo --version; printenv HOME'
```

原始输出：

```text
bash: line 1: rustc: command not found
bash: line 1: cargo: command not found
/home/stone
```

注意该命令的退出码为 0，因为最后的 `printenv HOME` 成功；这不表示 Rust 工具链可用。继续只读检查标准安装位置，排除仅缺少 PATH 配置：

```powershell
wsl.exe -d Ubuntu -- ls -ld /home/stone/.cargo/bin /home/stone/.rustup/toolchains /usr/bin/cargo /usr/bin/rustc /usr/local/bin/cargo /usr/local/bin/rustc
```

退出码为 1，原始输出：

```text
ls: cannot access '/home/stone/.cargo/bin': No such file or directory
ls: cannot access '/home/stone/.rustup/toolchains': No such file or directory
ls: cannot access '/usr/bin/cargo': No such file or directory
ls: cannot access '/usr/bin/rustc': No such file or directory
ls: cannot access '/usr/local/bin/cargo': No such file or directory
ls: cannot access '/usr/local/bin/rustc': No such file or directory
```

按评估者明确的停止要求结束，没有安装工具链，没有新增下载，也没有在 WSL home 创建检出。本 worktree 的 `target/perf-tools/vtebench` 仍保留上轮的临时检出，不是最终运行路径。

