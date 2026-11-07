# 迁移期间的新功能

本文件收录迁移期间新增、两个客户端都要有的功能，任务 ID 以 `N-` 开头。按 [README 执行规则第 9 条](README.md#2-执行规则)：

- 先在 Tauri 版实现，业务逻辑写在 Rust 中可以移入 core 的模块里（P1-09 之后直接写在 core），不写在只有 React 能用的 TypeScript 里。
- 在 `feature-matrix.md` 中登记一行，GPUI 版由对应阶段的卡实现，或在本文件另开卡。
- 这些卡不是重构，可以改变行为，但改变必须是任务卡写明的范围。

## N-01 重启后续接 AI 会话

- 依赖：无（[决策 009](decisions/009-agent-session-resume.md)）
- 规模：L，开始前在报告中拆成可以独立审查的提交
- 并行：与迁移的各卡独立；动到 `agent_integration` 时避免与该模块的其他开发并行
- 参考：
  - AI 工具集成：`src-tauri/src/agent_integration/`（`claude_code.rs`、`codex.rs` 的钩子，`console_report.rs` 的 Windows 报告，`remote.rs` 的远程安装）、`src/lib/agentStatus.ts`、`src/components/TerminalTab/attentionReports.ts`
  - 工作目录：`src-tauri/src/terminal/cwd_watch.rs`、`terminal/shell_integration.rs`、`ssh/shell_integration.rs`、`src/components/TerminalTab/cwdReports.ts`、`src/lib/terminalCwd.ts`、`src/lib/remoteCwd.ts`
  - 会话恢复：`src/components/TTermApp/useSessionRestore.ts`、`src/hooks/useSessionPersistence.ts`、`src/components/TerminalTab/savedTerminalContent.ts`
  - 命令提示符就绪：`src/components/TerminalTab/commandMarks.ts`
- 工作：
  1. **会话 ID。** 让 Claude Code 与 Codex 的钩子在报告中带上会话 ID。钩子从标准输入的 JSON 读取 `session_id`，校验后放进 `OSC 777 ; tterm-agent ; <agent> ; <state> ; <session>`；Codex 的钩子已经在共享后台进程的情况下这样做。区分“这个会话在本终端中运行”与现有的“发给所有运行该工具的终端”两种报告，不能让一个标签记下别的标签的会话。Windows 上经 `--agent-report` 的报告同样带上会话 ID。已安装的旧钩子显示为“需要更新”，沿用现有的 `Outdated` 状态。
  2. **记录续接信息。** 每个终端标签记录当前运行的 AI 工具、会话 ID 与工作目录。工作目录取工具启动时 shell 报告的目录；拿不到时用钩子输入中的 `cwd`。收到 `ended`、或 shell 回到命令提示符时清除记录。
  3. **保存。** 记录写进保存的工作区，作为终端标签的可选字段；字段变化时随现有的防抖保存写盘，不额外增加写盘次数。旧文件没有该字段时按现状恢复；旧版本读到新字段时忽略它，用测试证明两个方向都不出错。
  4. **构造续接命令。** 一个纯 Rust 模块，输入为工具、会话 ID、工作目录与目标 shell（bash、zsh、fish、sh、PowerShell、cmd，以及远程主机上由 shell 集成识别的 shell），输出要输入终端的一行命令：先切换到工作目录，再执行续接。会话 ID 只接受字母、数字与连字符；目录按目标 shell 转义；不认识的 shell 不续接。续接命令以实现时 Claude Code 与 Codex 的官方文档为准，并在本机实测，报告中记录版本。
  5. **恢复。** 重开 tTerm、按现有流程恢复标签后，有续接记录的标签在 shell 第一次显示命令提示符时输入续接命令；SSH 标签在重新连接并认证成功之后才会出现提示符。一段时间内（例如 15 秒）没有等到提示符，就不再自动输入，改为在标签中提示用户可以手动续接，并提供一个按钮。用户在续接前已经开始输入时不再自动输入。
  6. **设置。** 在会话恢复的设置旁加开关“重启后继续 AI 会话”，默认开启；文案加入 `en.json` 与 `zh.json`。
  7. **登记。** 在 `feature-matrix.md` 的对应行填写现有实现的位置。
- 产出：钩子与报告的修改、续接信息的记录与保存、续接命令模块及其测试、恢复流程、设置项、功能矩阵中的一行、`reports/N-01.md`。
- 验收：
  - 本地：在 macOS 或 Linux 的 zsh 与 bash、Windows 的 PowerShell 中，分别运行 Claude Code 与 Codex，发一条消息后完全退出 tTerm；重开后标签自动续接同一个会话，能看到之前的对话。报告附录屏或截图，以及测试过的工具版本。本机没有的平台标“未验证”。
  - SSH：在一台测试用的 SSH 主机上同样完成 Claude Code 与 Codex 的续接。
  - 同一目录的两个标签各自运行 Claude Code，重开后各自接回自己的会话。
  - 用户自己退出工具后再退出 tTerm，重开时不续接；关闭设置开关后不续接。
  - 续接命令模块对每种 shell 的转义有单元测试，包括含空格、引号、`$`、反引号、`%` 与中文的目录；非法会话 ID 与不认识的 shell 返回不续接。
  - 保存格式的兼容测试：新旧版本的工作区文件互相读取不出错。
  - AGENTS.md 的全部检查通过。
- 评估重点：
  - 不同标签之间的会话 ID 是否会串；共享后台进程的 Codex 报告是否被误记为本标签的会话。
  - 自动输入的命令是否只可能是 tTerm 构造的续接命令，是否有路径能让保存文件中的文本被当作命令执行。
  - 等待提示符的时机：密码输入、主机密钥确认、MOTD 很长、shell 启动很慢时是否会误输入。
- 不做：
  - 让进程在 tTerm 退出后继续运行（决策 009 中的 L3）。
  - 在远程主机上自动套用 tmux、screen 或 dtach。
  - 为没有安装 AI 工具集成的工具猜测会话。
