# P4-16 第六批：命令标记与本机 Shell 集成

## 结论

完成（自动化验收范围内）。终端页已接入「标记命令（Shell 集成）」与「本机 Shell 集成」两个开关：搜索、草稿编辑、整页一次保存、重开回读、已打开窗格与新建标签的运行时更新均已实现；命令标记行还提供与 React 相同的 bash / zsh 与 fish 脚本复制动作。Windows 隐藏本机集成行且不参与搜索。视觉证据与 macOS/Linux 真实 shell 行为未验证。

## 改动

未提交（按任务要求不创建提交）。

- `crates/tterm-core/src/config/terminal_font_file.rs`：`TerminalPageSettings` 增加 `command_marks` 与 `local_unix_shell_integration`；`From<&AppConfig>`、整页回读和 `save_page_at` 的差异写入都带上这两个字段，仍然只改与最新磁盘内容不同的字段。
- `crates/tterm-core/src/config/terminal_font_file/page_tests.rs`：新增两项随整页保存并回读的测试，补全既有结构体字面量。
- `crates/tterm-terminal/src/terminal.rs`：`Config.command_marks`（默认开）、`State.command_marks_enabled`，关闭时跳过标记扫描。
- `crates/tterm-terminal/src/terminal/marks.rs`（新）：运行时开关、启用时的扫描入口和标记历史记账。拆出后 `terminal.rs` 为 798 行（含共享工作树中 P2-12 的 `mod sync_output;`），低于 800 行上限。
- `crates/tterm-terminal/tests/overview_marks.rs`：关闭后清空已有标记、停止收集，重新开启后从下一条提示符继续。
- `crates/tterm-gpui/src/command_marks_setting.rs`（新）：两行开关接在「终端行为」块之后，无独立标题；本机集成行用编译期平台条件隐藏，并在命令标记关闭时禁用。
- `crates/tterm-gpui/src/command_marks_setting.rs`：命令标记行增加「复制 bash / zsh 脚本」与「复制 fish 脚本」两个按钮，直接引用 `tterm_core::remote_shell_integration` 的共享脚本写入应用剪贴板；不发命令、不启动 shell、不改草稿。复制后行内显示「脚本已复制」，对应 React 的 toast。
- `crates/tterm-gpui/src/terminal_demo/window/settings_page/`：`mod.rs` 增加 `CommandMarks` 小节及打开、关闭、提交、保存结果与搜索聚焦/滚动接线；`search.rs` 登记两条搜索项并按平台过滤本机集成；`shell.rs` 让新小节紧接剪贴板小节，不额外加 24px 间距；`test_access.rs` 增加测试读取。
- `crates/tterm-gpui/src/config_state.rs`：整页保存同时写入两个字段，`TerminalFontChanged` 携带已提交的 `command_marks`。
- `crates/tterm-gpui/src/terminal_demo/`：`SessionManager::set_command_marks_enabled` 更新所有在线会话和后续会话配置；`Session::set_command_marks_enabled` 作用于模型并唤醒视图；窗口启动时的模型配置读取共享值；配置事件到达后调用会话管理器。
- `crates/tterm-gpui/src/terminal_demo/window/settings_page_tests/command_marks.rs`（新）与同名 `mod` 声明、`layout.rs`：覆盖草稿、保存、重开、搜索、平台隐藏、写失败重试、在线窗格、新建标签与两个复制动作，并更新小节间距断言。
- `crates/tterm-core/src/i18n/keys.rs`：登记已有中英文文案的 8 个键，包括两个复制按钮与复制成功提示。
- `docs/gpui-migration/settings-inventory.md`：终端页改为 16/16 个与 GPUI 相关的字段已迁移，保留 renderer agent 对两个 WebView 字段的说明。

## 验收

| 验收条件 | 结果 | 证据 |
| --- | --- | --- |
| 终端页显示「标记命令（Shell 集成）」与「本机 Shell 集成」两行，初始值为共享配置 | 通过 | `settings_page_tests::command_marks::switches_draft_until_the_page_save_and_reload_on_reopen` |
| 本机集成仅 macOS/Linux 显示；Windows 不渲染且搜索不到；命令标记关闭时禁用 | 通过（Windows 分支实测；macOS/Linux 的显示分支为同一编译期判断，未在真机运行） | 同上（关闭时点击不生效）与 `search_finds_both_switches_and_scrolls_them_into_view` 的 Windows 分支 |
| 未保存只改草稿，不写配置、不影响窗格 | 通过 | 同一测试：保存前写入列表为空，`ConfigState` 仍为旧值 |
| 一次 Save 写入整页，并且只把变更字段写入最新文件 | 通过 | `tterm_core::config::terminal_font_file::page_tests::saves_command_marks_and_local_shell_integration_with_the_page`、`applies_only_the_changed_fields_to_the_latest_file` |
| 保存失败保留草稿并显示错误，重试可成功 | 通过 | `a_failed_save_keeps_the_marks_draft_and_retry_writes_it` |
| 重新打开设置读回已保存值 | 通过 | `switches_draft_until_the_page_save_and_reload_on_reopen` 的重开断言 |
| 已打开的窗格（含分屏）在保存后清空标记并停止收集 | 通过 | `saving_marks_updates_open_panes_and_new_tabs`：两个在线模型均变为关闭，已有标记为空，后续 OSC 133 不产生标记 |
| 新建标签继承保存后的设置 | 通过 | 同一测试：第三个标签的模型 `command_marks_enabled()` 为 false，喂入 OSC 133 后仍为空 |
| 搜索能找到两行并滚动、聚焦到对应开关 | 通过 | `search_finds_both_switches_and_scrolls_them_into_view`：Enter 后小节在面板内，Space 直接切换该开关；中文标签同样可搜索 |
| 两个复制按钮写入与 React 相同的脚本，且不改草稿、不保存、不执行 | 通过 | `copy_actions_write_the_shared_scripts_and_leave_the_draft_alone`：分别读出 `remote_shell_integration::{POSIX_SHELL_INTEGRATION, FISH_SHELL_INTEGRATION}`，草稿不变、无写入，并显示复制成功提示 |
| 关闭标记后重新开启可从下一条提示符继续收集 | 通过（模型层） | `terminal` 集成测试 `disabling_command_marks_drops_them_and_stops_new_ones` |
| 不修改 WebView 渲染器字段，不新增 `allow`/`expect`，不使用 `any` | 通过 | 改动路径清单；renderer agent 的配置/搜索说明保持原样 |
| 两端同状态视觉对照 | 未验证 | 本轮未运行截图示例，也没有采集 React 对照图；见“遗留问题” |
| macOS/Linux 真实 bash、zsh、fish 启动脚本与命令标记 | 未验证 | 当前机器为 Windows；只验证了开关保存和新标签继承，未验证脚本注入后的真实 shell 行为 |

## 检查

本轮按协调者要求提前收尾，只跑与改动直接相关的最小集合；全部在仓库根目录执行，Rust 测试二进制需要离开沙箱运行（沙箱内报 `STATUS_DLL_INIT_FAILED`，与代码无关）。

```text
cargo check -p tterm-gpui --all-targets
  Finished `dev` profile ... （通过，无警告）

cargo test -p tterm-gpui settings_page_tests
  test result: ok. 44 passed; 0 failed

cargo test -p tterm-gpui settings_page_tests::command_marks
  test result: ok. 5 passed; 0 failed（含复制动作测试）

cargo test -p tterm-terminal --test overview_marks
  test result: ok. 16 passed; 0 failed

$env:TEMP=(Resolve-Path target\tmp).Path; cargo test -p tterm-core config::terminal_font_file
  test result: ok. 17 passed; 0 failed

cargo fmt --all --check
  （通过）

node scripts/check-file-size.mjs
  file-size: 1565 files within their limits
```

核心配置测试需要把 `TEMP`/`TMP` 指到工作区内，因为本机 `%TEMP%` 对测试进程只读；这是环境限制，不是测试代码问题。

未运行（按“先收尾”的要求）：`pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`node scripts/perf/gpui-release-frame-budget.mjs`。

## 偏差

- **模块拆分。** `terminal.rs` 在共享工作树里同时收到 P2-12 的 `mod sync_output;` 和本批的开关逻辑，行数超过 800。按 AGENTS.md 的要求把命令标记的运行时控制与记账拆到 `terminal/marks.rs`，没有改变行为；拆分后 798 行。建议评估时把这一步视为本批的必要重构，而不是额外功能。
- **复制反馈。** React 复制脚本后弹 toast；GPUI 设置页没有 toast 基础设施，改为在按钮旁显示行内「脚本已复制」，复制内容与来源不变。
- **本机集成的生效范围。** 文案写明“对新打开的标签页生效”，实现也只把值写入后续会话配置，不重启已有 PTY；已有标签继续保持启动时的行为。
- **共享工作树。** renderer agent 对 `AppConfig`、搜索索引和文档的修改未被覆盖；`settings-inventory.md` 只改了终端页的计数和待补说明，并保留了两个 WebView 字段的兼容说明。
- **测试环境。** 沙箱内所有 Rust 测试二进制都以 `STATUS_DLL_INIT_FAILED` 退出（包括改动前的既有测试）；本轮在沙箱外运行测试。该限制不影响代码结论，但 CI 之外的复现者需要知道。

## 遗留问题

- **视觉证据待补。** 本轮没有生成 GPUI 截图，也没有 React 同状态对照；若评估需要，可运行 `cargo run -p tterm-gpui --example settings_capture --features tterm-gpui/automation`，再与 `FontSettingsTab.tsx` 的同状态截图比较行顺序、标签换行、禁用态和长文高度。
- **macOS/Linux 真机未验。** 本机为 Windows，无法确认真实 bash、zsh、fish 经集成脚本启动后是否上报 OSC 133，也无法确认“仅非 Windows 显示”的实际观感。
- **整体验收。** 本报告表格只列分支任务的定向测试；协调者在最终工作树跑完全部门禁。Windows 结果与两项 release 测量见[状态页](../status.md)。
- **`status.md` 未改。** 按任务要求没有更新状态文件和派发文件，评估结论由协调者写入。
