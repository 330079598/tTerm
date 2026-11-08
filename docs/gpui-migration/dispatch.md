# 任务派发与验收

本文写给负责派发和验收的协调者（目前是 Claude Code）。协调者把任务卡派给 Codex 执行，验收结果，合并，再派下一张卡，决策人 stone 不必逐张手动下发提示词。换机器或开新会话时，从本文和 [status.md](status.md) 接手。

## 1. 分工

协调者自己决定：

- 执行计划是否可行，返工的要求和轮次；
- 一张卡是否通过，以及本地合并到 `GPUI`；
- 执行者说“缺某个工具或路径”时，先自己核实，确实缺了再找 stone。

要问 stone 的：

- 涉及行为取舍或新增例外的决定；
- 任何可能碰到真实数据的操作；
- 每一次 push；
- G 关卡。

## 2. 当前进度（2026-11-08）

- `GPUI` 已推送。逐卡状态见 [status.md](status.md)。
- **P0-24 暂停**，在分支 `migrate/P0-24-ui-primitives`，没有合并。Button 与 Input 已改为用 tterm-ui 令牌自绘，接近门槛。剩下的问题见该分支 `reports/P0-24.md` 末尾的“评估（2026-11-08，暂停）”。其余 6 个组件还没做，按两批做。
- 排队，按顺序：
  1. 继续 P0-24；
  2. P0-01 补跑 vtebench（WSL 里已装 cargo，`~/vtebench` 已克隆）；
  3. P0-21，机器空闲时单独跑；
  4. P0-17、P0-18 的 Windows 部分，要人动手的项整理成检查清单给 stone；
  5. P0-05、P0-22；
  6. 请 stone 过 G1。
- 平台顺序按[决策 012](decisions/012-windows-first.md)：Windows，然后 macOS，最后 Linux。macOS 与 Linux 的验收项记为“推迟（决策 012）”，不阻塞进度。手头有 Mac 时，可以补 macOS 的推迟项：P0-24 的 macOS 对比、P0-02、P0-01 的 macOS 基线、P0-18 的 macOS 部分。

## 3. 派发

### 3.1 准备 worktree

每张卡一个 worktree，放在主仓库旁边，分支从 `GPUI` 切出：

```sh
git worktree add ../tTerm-<ID> -b migrate/<ID>-<短名> GPUI
cd ../tTerm-<ID> && pnpm install --frozen-lockfile
```

- Windows 还要把主仓库的 `src-tauri/resources/conpty/*` 复制过去。
- 复制一份已有的 `target` 能省下首次编译的时间：Windows 用 `robocopy /E /MT:16`，macOS 用 `cp -Rc`（APFS 克隆，不占额外空间）。

### 3.2 提示词

提示词写成文件，放在派发目录（见 3.3）。每份都包含以下几节：

1. **位置**：worktree 路径与分支。
2. **开始前按顺序阅读**：AGENTS.md、本目录 README.md 第 1、2、4、6 节、任务卡所在章节、依赖卡的报告（尤其是“遗留问题”）、[environments.md](environments.md)。
3. **硬性规则**：
   - 只做任务卡“工作”中列出的内容；卡与代码不符时停下，写进报告的“偏差”。
   - 新依赖只用 [pins.md](pins.md) 中的版本。
   - 使用独立数据目录 `com.stone.tTerm.gpui-dev`。
   - 不修改 status.md。
   - 本机做不了的验收项标“未验证”或“[人工]”，写清缺什么。
   - Zed 的源码可以读，不能抄（决策 008）。
4. **完成标准**：
   - AGENTS.md 的全部检查通过。
   - 报告写在 `reports/<ID>.md`，验收表逐条照抄任务卡并附证据。
   - 提交以 gitmoji 开头，不加 AI 署名。
   - 完成后停下，不要合并。
5. **本轮的运行方式**：
   - 执行者由 `codex exec` 非交互启动，中途没人回答，遇到需要确认的事就停下。
   - 只在本 worktree 中写文件，不碰主仓库和其他 worktree。
   - 命令因沙箱权限失败时，申请提升权限后重跑，不要用 `git commit-tree`、临时 index 之类的底层命令绕开。
   - 长命令一次性前台等待，不要空输入反复读同一个进程（网关会当成死循环中断）。
   - 自己启动的后台进程在结束前全部关掉。
   - 最后一条消息按固定格式写：

     ```
     状态：完成待评估 / 需要确认 / 阻塞
     提交：本轮新增提交的哈希与摘要
     需要确认或阻塞的原因：没有就写“无”
     未验证与 [人工] 项：逐条列出
     ```

规模 L 的卡，第一轮只让执行者写拆分计划，然后以“需要确认”停下，协调者审过再实施。范围大的卡要分批派：一次铺得太开，执行者容易用静态示意图凑数（P0-24 第一轮就是这样）。

### 3.3 运行

```sh
# macOS、Linux
scripts/migration/run-codex.sh <ID> ../tTerm-<ID> <提示词文件> [thread]
# Windows
scripts/migration/run-codex.ps1 -Card <ID> -Worktree ..\tTerm-<ID> -PromptFile <提示词文件> [-ResumeId <thread>]
```

- 日志 `<ID>-<时间>.jsonl` 和最后一条消息 `<ID>-<时间>.last.md` 写在派发目录。派发目录默认是主仓库旁边的 `tTerm-dispatch`，可以用 `TTERM_DISPATCH_DIR` 改。
- 脚本最后一行打印 thread。返工时带上它续跑，执行者会保留上下文。thread 只存在于跑它的那台机器上，换机器后只能开新会话，让执行者先读报告里的评估记录。
- 沙箱用 workspace-write，按需审批，提升权限由自动审查者批准；可写目录是主仓库的 `.git` 和 `~/.cargo`，允许联网。执行者在沙箱里报权限错误时，先看是不是还缺某个可写目录。macOS 上这套配置还没实际跑过，第一次派发时要留意。
- 协调者在后台运行脚本，等它结束的通知，不要轮询日志。

## 4. 验收

每张卡按下面的顺序验收：

1. 读 `.last.md`。
2. 看三点 diff：`git diff GPUI...HEAD`。
3. 核对报告和证据。执行者写的数字，协调者要自己重跑一遍。
4. 在本机重跑 AGENTS.md 的全部检查。
5. 需要时做真实输入验证，截图放进报告的证据目录。
6. 在报告末尾加“## 评估”，写结论、复核的数字和遗留问题，然后提交。
7. 在主仓库执行 `git merge --no-ff`，更新 status.md 那一行，运行 prettier，提交。
8. 删除已合并的 worktree，分支保留。

部分通过的卡也可以合并，status.md 写明缺什么。不通过就写返工提示词：列出问题、原因和本轮范围，用同一个 thread 续跑。

## 5. 安全规则

- **真实数据**：`com.stone.tTerm` 与 `com.stone.tTerm.dev` 两个标识的数据目录里是 stone 的真实连接，不得以这两个标识运行 Tauri 版，也不得读写它们的目录。
- **隔离运行 Tauri**：
  - 用临时标识，写进 `target/` 下的一个 JSON 文件，传给 `--config`。直接在命令行写 JSON 会被 pnpm 弄丢引号。
  - 再把旧版数据的来源指向一个空目录，因为 `src-tauri/src/migrate.rs` 会把旧版目录里的 profiles 与 session 复制进新标识的目录：
    - Windows：设 `APPDATA`，旧版目录是 `%APPDATA%\tterm`。
    - macOS 与 Linux：设 `XDG_CONFIG_HOME`，旧版目录是 `$XDG_CONFIG_HOME/tterm`，否则是 `~/.config/tterm`。
  - 启动后先只读调用 `list_profiles`，确认为空。
  - 用完删除临时标识的数据目录：Windows 在 `%APPDATA%` 与 `%LOCALAPPDATA%` 下，macOS 在 `~/Library/Application Support/` 与 `~/Library/Caches/` 下。
  - 钥匙串按标识区分服务名（`tterm (<标识>)`），临时标识读不到正式版存的密码。
- **真实输入**：允许，但每次输入前确认前台窗口是目标窗口。锁屏时等 stone 解锁。
- **配置**：不修改 Claude Code、Codex 的全局配置和 hooks。
- **提交**：不加 `Co-Authored-By` 或任何 AI 署名。

## 6. 经验

执行者出过的问题。写提示词和验收时，提前防住：

- 在主仓库而不是 worktree 里写文件；用 `git commit-tree` 绕开沙箱。
- 反复空读同一个进程的输出，被网关中断。
- 后台进程没关：一个残留的 Vite 服务让之后的参照截图全部超时。
- 为了让对比通过，去改生产 CSS，或者在参照端夹具里写死颜色。参照端只能是 Tauri 版真实渲染的样子。
- GPUI 端交静态示意图而不做组件；样式快照手写，和绘制不同源。
- 手动运行 `scripts/parity/capture.mjs` 生成的参照目录没有经过颜色测量，`compare` 会把它当成有效缓存。要重新生成时，删掉目录再跑 `compare`。
- “两轮调不好就停”只适用于原因不明的情况。原因明确的失败要修完。
- 执行者说某个工具没装，可能只是沙箱里看不到（例如 WSL 里的 cargo）。先自己核实。
- 隔离运行后，报告说已清理，实际还留着目录。验收时自己检查。
