# 给执行者的指令模板

把一张任务卡交给其他 coding agent（Codex、opencode 上的 DeepSeek 等）时使用。这些 agent 会自动读取 `AGENTS.md`，但不会主动阅读本目录的迁移规则，所以指令中要点名让它们阅读。工作方式与评估流程见 [README 第 1 节](README.md#1-工作方式)与[第 7 节](README.md#7-评估方式)。

## 1. 准备 worktree

每个执行者使用独立的 worktree，避免并行的 agent 互相覆盖文件：

```powershell
git worktree add ..\tTerm-<ID> -b migrate/<ID>-<英文短名> GPUI
```

每个 worktree 都要从头编译 GPUI，占用的磁盘与时间都很多。几个 worktree 可以设置同一个 `CARGO_TARGET_DIR`，cargo 会给同时进行的构建排队。

改动同一个 crate 的卡不要同时交给不同的执行者，例如 P0-12 与 P0-13 都会改 `tterm-terminal-view`。

## 2. 基本模板

把尖括号中的内容换成实际的值：

```text
你在 tTerm 仓库的 worktree <路径> 中执行 GPUI 迁移任务卡 <ID>，分支 migrate/<ID>-<短名>（已从 GPUI 切出）。

开始前按顺序阅读：
1. AGENTS.md（全部适用）
2. docs/gpui-migration/README.md 第 1、2、4、6 节（工作方式、执行规则、架构约定、报告模板）
3. 任务卡：docs/gpui-migration/<文件>.md 中的 “### <ID>” 一节，以及卡上“参考”列出的文件
4. 依赖卡的报告：docs/gpui-migration/reports/<依赖ID>.md，尤其是“遗留问题”

硬性规则：
- 只做任务卡“工作”中列出的内容，“不做”中的一律不碰。卡与代码不符时停下，写进报告的“偏差”，不要自行扩大范围。
- 新依赖只用 docs/gpui-migration/pins.md 中的版本，不加 git 依赖，不升级现有依赖。
- 使用独立的数据目录 com.stone.tTerm.gpui-dev，不读写 com.stone.tTerm 或 com.stone.tTerm.dev。
- 复制的第三方代码在文件头注明来源仓库、提交、原路径和许可，并登记到 docs/gpui-migration/third-party.md。
- 不修改 docs/gpui-migration/status.md，它由评估者维护。
- 本机只有 Windows。macOS、Linux 上的验收项，以及需要真机或真人操作的项，标为“未验证”或“[人工]”，写清缺什么，不要写“通过”。
- 规模为 L 的卡，先在报告中写好提交拆分计划再开始写代码。

完成标准：
- AGENTS.md 中“Before you finish”的全部命令通过，结果摘要贴进报告。
- 报告写在 docs/gpui-migration/reports/<ID>.md，格式按 README 第 6 节。验收表逐条照抄任务卡的验收条件，每条附上命令与原始输出、测试名或截图路径。
- 提交遵守 AGENTS.md：以 gitmoji 开头，一个提交只做一件事，移动代码与修改代码分开提交，不加 Co-Authored-By 或任何 AI 署名。
- 完成后停下，不要合并到 GPUI，也不要开始下一张卡。
```

## 3. 按情况追加的段落

**终端相关的卡**（P0-11 至 P0-15、P0-21、P2 全部）必须追加以下段落，规则见[执行规则第 6 条](README.md#2-执行规则)与[决策 008](decisions/008-zed-terminal-reading.md)：

```text
- 可以阅读 Zed 的 terminal、terminal_view 源码，可以参考 docs/gpui-migration/terminal-design.md，但是要注意不能直接抄袭 Zed 的源码，只能借鉴实现思路。
- 在报告中列出阅读过的 Zed 提交与文件，并声明没有复制 Zed 的代码、注释、测试或私有标识符。
```

**能力较弱或较快的模型**（例如 DeepSeek v4.1 flash）只交给规模为 S 或 M、边界清楚的机械性工作，例如夹具、`data-parity` 标记、测试数据与文档，并追加：

```text
- 每完成一个提交就运行一次检查，失败时先修好再继续。
- 遇到不确定的设计选择时停下，在报告中列出可选方案及其利弊，不要自行决定。
```

这类模型最常见的问题是把没有验证过的项写成“通过”，评估时要逐条核对证据栏。

**规模为 L 的卡或关键路径上的卡**交给较强的模型，并要求先提交拆分计划，经人确认后再开始：

```text
- 第一步只写报告中的拆分计划，列出每个提交的范围与验证方法，然后停下等待确认。
```

## 4. 交回评估

执行者完成后，告诉评估者（Claude）“评估 `<ID>`，分支 `migrate/<ID>-<短名>`”。评估者按 [README 第 7 节](README.md#7-评估方式)审查，给出“通过”“需修改”或“驳回”，并更新 `status.md`。结论为“需修改”时，评估者会列出具体条目，把这些条目原样转给原执行者，在同一分支上继续。
