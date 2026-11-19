# P1-11c-c2-shells 本地 Shell 列表与标签（仅 shell 子集）

## 结论

**最终验收：Shell 子集在 Windows 上通过了仓库规定的全部检查。** 修复 ConPTY 重复写入后，进程枚举测试改为由测试进程启动并等待就绪的子进程，断言其父 PID 确实指向测试进程；没有改变生产代码或忽略测试。带 PTY、使用 `target/workspace-test-tmp` 临时目录运行 `cargo fmt --all --check`、完整 workspace Clippy（`-D warnings`）和 `cargo test --workspace` 均退出 0。Rust 共 2710 项通过、29 项原有忽略；前端六项检查通过。后文保留此前未通过轮次的记录。

部分完成。只实现了 P1-11c c2 中的 `localShells` 与 `terminalShells` 两个纯逻辑模块及其行为测试：`src/lib/localShells.ts` 的 28 个导出与 `src/lib/terminalShells.ts` 的 3 个导出已移植为 `tterm-core::workspace::local_shells`，19 个现有 TypeScript 用例对应 20 个 Rust 测试，另有 19 个标签、边界、未知 id 与 oracle 测试，共 39 个。

**这不是完整的 c2，也不是完整的 P1-11c。** `appConfig` 的默认值与限额、`appConfigNormalize`（含 `normalizeAppState`、`normalizeSecretStatus`）、c1 的 `TabSettings` 与 `AppConfig` 之间的类型转换都不在本轮范围内；P3-02 未完成，本轮也没有任何保存/读取/UI 接线。

## 改动

本批 Shell 迁移的文件如下：

| 文件 | 职责 |
| --- | --- |
| `crates/tterm-core/src/workspace/local_shells.rs` | 模块入口、`AUTO_SHELL_KEY`、已知 shell id 的解析（`shell_type_from_id`，只供标签查表）、隐藏键、`programName` 等价实现及内部字符串/路径辅助。 |
| `crates/tterm-core/src/workspace/local_shells/types.rs` | `ShellConfig`（借用字段，`From<&AppConfig>`、`with_default`）、`DefaultShellSettings`、`LocalShellChoice`、`ShellEntry`/`ShellEntryKind`、`CopiedShellCommand`、`FailedShellFallback`、`DefaultShellChange`、`ShellListUpdate`。 |
| `crates/tterm-core/src/workspace/local_shells/entries.rs` | `build_shell_entries`、`copied_shell_command` 与条目私有辅助。 |
| `crates/tterm-core/src/workspace/local_shells/default_shell.rs` | `default_shell_key`、`shell_choice_key`、`default_shell_updates`、`default_shell_choice`。 |
| `crates/tterm-core/src/workspace/local_shells/connections.rs` | `local_shell_connection`、`with_current_local_shell`、`starts_shell`、`starts_as_saved`、`with_local_shell`。 |
| `crates/tterm-core/src/workspace/local_shells/saved.rs` | `own_shell_choice`、`saved_local_shell`、`saved_local_shell_updates`、`removed_local_shell`、`moved_local_shell`、`toggled_hidden_shell`、`new_local_shell_id`、`failed_shell_fallback`。 |
| `crates/tterm-core/src/workspace/local_shells/shell_label.rs` | `is_detected_custom_shell`、`shell_label_key`、`terminal_shell_label`，来源 `src/lib/terminalShells.ts`。 |
| `crates/tterm-core/src/workspace/local_shells/tests/` | 19 个 TypeScript 用例的对照测试、标签测试、未知 id 与 sessionNonce 回归测试、JavaScript 字符串边界与 P1-11c oracle 全等测试。 |

唯一改动的既有文件是 `crates/tterm-core/src/workspace/mod.rs`，只加了一行 `pub mod local_shells;` 与文档注释。没有改动 `src/`、`src-tauri/`、`docs/gpui-migration/status.md`、文件大小基线或任何已有模块。

设计要点：

- 复用既有类型：`config::local_shells::LocalShell`、`workspace::tab::TabConnection`/`Tab`/`TerminalShellType`（只用于已知 id 的标签查表）、`terminal::pty::TerminalShellProfile`、`tterm_terminal::js_text::js_trim`。没有新增依赖，没有 `pub use`，没有 `unwrap`/`expect`/`allow`/`expect(too_many_lines)`。
- shell id 一律按保存的 `String` 处理：`LocalShellChoice.shell` 是 `String`，`buildShellEntries` 不再丢弃未知 profile，`defaultShellChoice` 不再把未知 id 换成 Auto。连接里的 `terminalShell` 用 `saved_field`/`set_saved_field` 读写，未知 JSON 原样保留。
- `startsShell` 直接比较保存的原始字符串：字段缺失或为 `null` 时才当作 `auto`，未知 id 既不等于 Auto 也不等于其他 id；字段是数字等非文本值时一律不相等（TS 的严格比较同样为假）。
- JavaScript 字符串语义逐条照抄：路径与名称的 trim 用 `js_trim`（含 U+FEFF 等 JS 空白），`programName` 只去掉**一个**首尾双引号、不再 trim 词干、扩展名判定等价于 `\.[^.]+$`（末尾点不剥、`.hidden` 保留、`nu .exe` 得到 `"nu "`、`""nu.exe""` 得到 `"nu`）。
- 顺序：条目固定为 Auto → 检测到的 shell（含“WSL 自身默认”条目插在第一个发行版之前）→ 用户自建；`hidden_shells` 重复键全部移除；用户自建的 shell 按保存顺序。
- `sessionNonce` 按 `((保存的数值 ?? 0) + 1) >>> 0` 计算：从 canonical 字段读原始 JSON 数字，四舍五入前的截断与 2^32 取模按 JS `ToUint32` 实现，覆盖小数、负数与超过 `u32::MAX` 的值。
- 保存的 `TabConnection` 键序与前端对象字面量一致（`type,cwd,terminalShell,terminalShellCustomPath,terminalShellCustomArgs,terminalShellId[,terminalShellWslDistro]`），`None` 表示 `undefined`、不写键；有测试逐键断言。

## 验收

| 验收条件 | 结果 | 证据 |
| --- | --- | --- |
| 只实现 `localShells` 与 `terminalShells` 的纯逻辑等价物 | 通过 | 新增文件只在上表；`git status --porcelain` 中除既有未跟踪的 `.pi-*` 外只有 `workspace/mod.rs` 与本卡新增目录。 |
| 不实现 config 归一化、不改既有持久化/默认值 | 通过 | 未新增 `config/limits.rs`、`config/app_config_normalize.rs`；未改 `config/mod.rs`、`config/defaults.rs`、`config/local_shells.rs`、`config/deserialize.rs`。 |
| 不开始 P3，不接线 UI/save/load | 通过 | 新模块没有任何命令注册、事件、读写文件或 `AppConfig` 的可变写入；`with_local_shell` 等只返回新值。 |
| 19 个 TypeScript 用例全部移植且断言不弱化 | 通过 | 见下方“19 个 TS 用例 → Rust 测试”，共 20 个 Rust 测试（`lists Auto…` 拆成键列表与条目字段两条）。 |
| 另加 terminal-shell 标签测试 | 通过 | `tests/shell_label.rs` 4 个测试 + oracle 的 `terminalShells.labels`/`isDetectedCustomShell` 全等。 |
| 精确的 JS 语义（真值、trim、顺序、nonce 回绕、缺失 vs 空） | 通过 | `tests/program_name.rs`、`tests/default_shell.rs::keeps_missing_choice_fields_missing`、`tests/connections.rs::wraps_the_session_nonce_like_javascript`、`tests/oracle.rs`。 |
| 未知 shell id 不被换成 Auto、不被丢弃 | 通过 | `tests/default_shell.rs::keeps_an_unknown_default_shell_id`、`tests/entries.rs::lists_an_unknown_detected_shell_under_its_own_id`、`tests/connections.rs::tells_an_unknown_shell_from_auto`。 |
| 未知 id 在连接与重启中原样保留，其他 tab 字段不受影响 | 通过 | `tests/connections.rs::keeps_an_unknown_shell_id_through_a_connection_and_a_restart`、`tests/connections.rs::keeps_the_other_fields_of_a_restarted_tab`。 |
| 未知 shell 的标签仍走翻译回调，使用前端拼出的 `undefined` 键 | 通过 | `tests/shell_label.rs::keeps_the_label_of_a_shell_found_on_this_machine_or_a_wsl_distribution`、`tests/entries.rs::lists_an_unknown_detected_shell_under_its_own_id` 断言 `connection.terminalShellOptions.undefined`。 |
| 复用 core 既有类型，无 re-export、无新依赖 | 通过 | `Cargo.toml` 未改；`git grep -n 'pub use' -- crates/tterm-core/src/workspace/local_shells*` 无匹配，本卡未新增 `pub use`。 |
| 小模块、函数 <150 行、参数 ≤7 | 通过 | 生产文件 50–200 行；最长函数 `build_shell_entries` 约 80 行，`wsl_default_entry` 3 个参数。 |
| 公共项有文档注释 | 通过 | 所有 `pub` 类型、字段、常量与函数都有 `///` 或 `//!`。 |
| 不修改超限既有文件 | 通过 | 只改 `workspace/mod.rs`（未超限），并只加模块声明。 |
| 写本报告、不改 `status.md` | 通过 | 本文件；`git status` 中 `status.md` 未出现。 |

## 28 个 TS 导出 → Rust 项

`localShells.ts`：

| TS 导出 | Rust 项 |
| --- | --- |
| `ShellConfig`（类型） | `local_shells::types::ShellConfig<'a>`，`From<&AppConfig>` 与 `with_default` |
| `DefaultShellSettings`（类型） | `local_shells::types::DefaultShellSettings` |
| `LocalShellChoice`（类型） | `local_shells::types::LocalShellChoice`（`Option` 表示 `undefined`） |
| `ShellEntry`（类型） | `local_shells::types::ShellEntry` + `ShellEntryKind`（`"auto"｜"detected"｜"custom"`） |
| `FailedShellFallback`（类型） | `local_shells::types::FailedShellFallback` |
| `AUTO_SHELL_KEY` | `local_shells::AUTO_SHELL_KEY` |
| `detectedShellKey` | `local_shells::detected_shell_key` |
| `customShellKey` | `local_shells::custom_shell_key` |
| `programName` | `local_shells::program_name` |
| `buildShellEntries` | `local_shells::entries::build_shell_entries` |
| `copiedShellCommand` | `local_shells::entries::copied_shell_command` + `CopiedShellCommand` |
| `defaultShellKey` | `local_shells::default_shell::default_shell_key` |
| `shellChoiceKey` | `local_shells::default_shell::shell_choice_key` |
| `defaultShellUpdates` | `local_shells::default_shell::default_shell_updates` |
| `defaultShellChoice` | `local_shells::default_shell::default_shell_choice` |
| `localShellConnection` | `local_shells::connections::local_shell_connection` |
| `withCurrentLocalShell` | `local_shells::connections::with_current_local_shell` |
| `startsShell` | `local_shells::connections::starts_shell` |
| `startsAsSaved` | `local_shells::connections::starts_as_saved` |
| `withLocalShell` | `local_shells::connections::with_local_shell` |
| `ownShellChoice` | `local_shells::saved::own_shell_choice` |
| `savedLocalShell` | `local_shells::saved::saved_local_shell` → `ShellListUpdate` |
| `savedLocalShellUpdates` | `local_shells::saved::saved_local_shell_updates` → `ShellListUpdate` |
| `removedLocalShell` | `local_shells::saved::removed_local_shell` → `ShellListUpdate` |
| `movedLocalShell` | `local_shells::saved::moved_local_shell` |
| `toggledHiddenShell` | `local_shells::saved::toggled_hidden_shell` |
| `newLocalShellId` | `local_shells::saved::new_local_shell_id` |
| `failedShellFallback` | `local_shells::saved::failed_shell_fallback` |

`terminalShells.ts`：

| TS 导出 | Rust 项 |
| --- | --- |
| `TerminalShellProfile`（类型） | 复用 `terminal::pty::TerminalShellProfile`，不重复定义 |
| `isDetectedCustomShell` | `local_shells::shell_label::is_detected_custom_shell` |
| `terminalShellLabel` | `local_shells::shell_label::terminal_shell_label`（翻译函数按 A8 以闭包传入） |

另加一个 TS 类型系统自带的桥接：`shell_type_from_id`（保存的 id → `TerminalShellType`），只用于已知 id 的标签查表，并有与 serde 序列化结果逐项相等的测试（`tests/shell_id.rs`）。未知 id 在这里返回 `None`，但调用方不会因此丢弃或改写它。

## 19 个 TS 用例 → Rust 测试

测试模块路径省略公共前缀 `tterm_core::workspace::local_shells::tests::`。

| # | TS 文件 › 用例 | Rust 测试 |
| --- | --- | --- |
| 1 | `localShells.test.ts › buildShellEntries › lists Auto, the shells found here, then the user's own` | `entries::lists_auto_the_shells_found_here_then_the_users_own` 与 `entries::the_users_shell_carries_its_path_arguments_and_id` |
| 2 | `buildShellEntries › lists each WSL distribution under its own key, and WSL's default while that is the default` | `entries::lists_each_wsl_distribution_under_its_own_key_and_wsls_default_while_that_is_the_default` |
| 3 | `buildShellEntries › keys a shell found on PATH by its path and shows one the user added once` | `entries::keys_a_shell_found_on_path_by_its_path_and_shows_one_the_user_added_once` |
| 4 | `the default shell › is found by the id of the user's shell, or else by its kind or path` | `default_shell::is_found_by_the_id_of_the_users_shell_or_else_by_its_kind_or_path` |
| 5 | `the default shell › starts with the current path of the user's shell` | `default_shell::starts_with_the_current_path_of_the_users_shell` |
| 6 | `the default shell › is set with the path older versions read` | `default_shell::is_set_with_the_path_older_versions_read` |
| 7 | `the default shell › can be a WSL distribution, or WSL's own default` | `default_shell::can_be_a_wsl_distribution_or_wsls_own_default` |
| 8 | `the user's shells › keep the default's path in step when the default is edited` | `saved::keep_the_defaults_path_in_step_when_the_default_is_edited` |
| 9 | `the user's shells › leave Auto as the default when the default is removed` | `saved::leave_auto_as_the_default_when_the_default_is_removed` |
| 10 | `the user's shells › move one place at a time and stop at either end` | `saved::move_one_place_at_a_time_and_stop_at_either_end` |
| 11 | `the user's shells › hide and show detected shells by key` | `saved::hide_and_show_detected_shells_by_key` |
| 12 | `local tab connections › carry the user's shell id so a restart follows its edits` | `connections::carry_the_users_shell_id_so_a_restart_follows_its_edits` |
| 13 | `local tab connections › leave out custom fields for a built-in shell` | `connections::leave_out_custom_fields_for_a_builtin_shell` |
| 14 | `programName › names a shell after its program` | `program_name::names_a_shell_after_its_program` |
| 15 | `restarting a local tab with another shell › tells whether a tab starts the same shell` | `connections::tells_whether_a_tab_starts_the_same_shell` |
| 16 | `restarting a local tab with another shell › keeps the tab's directory and starts a new session` | `connections::keeps_the_tabs_directory_and_starts_a_new_session` |
| 17 | `restarting a local tab with another shell › tells whether a tab starts the user's shell as it is saved now` | `connections::tells_whether_a_tab_starts_the_users_shell_as_it_is_saved_now` |
| 18 | `restarting a local tab with another shell › offers the default shell, or Auto when the default is the one that failed` | `saved::offers_the_default_shell_or_auto_when_the_default_is_the_one_that_failed` |
| 19 | `restarting a local tab with another shell › saves a shell and makes it the default in one change` | `saved::saves_a_shell_and_makes_it_the_default_in_one_change` |

断言逐条保留：`startsShell` 的 7 个组合、`startsAsSaved` 的 4 个组合、`failedShellFallback` 的 4 个结果、WSL 条目的 5 个键列表与 `copiedShellCommand` 都照抄，没有删减或放宽。

另外 19 个测试：

- `shell_label`：`translates_the_shells_the_app_picks_itself`、`keeps_the_label_of_a_shell_found_on_this_machine_or_a_wsl_distribution`、`knows_a_custom_shell_found_on_this_machine`、`names_the_catalogue_key_of_every_shell_kind`（7 种 id 全部断言）。
- `oracle`：`oracle_matches_the_ported_shell_labels`（labels 9 项 + isDetectedCustomShell 4 项）、`oracle_matches_the_ported_shell_keys_and_names`（`localShells.keys`、`detectedShellKey`、`programName`、`extra` 的 `sessionNonceWrap`/`programNameTrailingSpace`/`programNameTwoQuotes`）、`oracle_matches_the_ported_choice_keys`（`shellChoiceKey`）。
- 边界：`program_name::keeps_the_javascript_string_operations`、`default_shell::keeps_missing_choice_fields_missing`、`default_shell::names_the_entry_a_choice_would_have_as_the_default`、`connections::saves_a_connection_in_the_frontend_key_order`、`saved::gives_a_new_shell_a_fresh_uuid`、`shell_id::reads_each_known_shell_id_as_its_kind`。
- 未知 id 与 nonce 回归：`default_shell::keeps_an_unknown_default_shell_id`、`entries::lists_an_unknown_detected_shell_under_its_own_id`、`connections::tells_an_unknown_shell_from_auto`、`connections::keeps_an_unknown_shell_id_through_a_connection_and_a_restart`、`connections::keeps_the_other_fields_of_a_restarted_tab`、`connections::wraps_the_session_nonce_like_javascript`（整数、`u32::MAX`、`2^32`、`.5`、负数、`1e12`、缺失与 `null`）。

oracle 期望值直接读 `docs/gpui-migration/reports/P1-11c-evidence/node-macos.json`（`include_str!`），与本卡不依赖平台的常量一致，不手抄数字；标签测试用 `key|fallback` 形式的假 `t`，与 oracle 脚本同形，因此能同时核对“查了哪个键”和“用哪个兜底”。

## 检查

环境：Windows 11，Rust 1.99.1。所有 cargo 命令都在仓库根执行，带 `--workspace`（不使用 `-p`）。

```sh
cargo test --workspace --exclude tTerm workspace::local_shells
```

通过：`tterm-core` 39 passed、0 failed（1810 filtered out），其余 workspace 成员 0 matched。

```sh
cargo clippy --workspace --exclude tTerm --all-targets -- -D warnings
```

通过：`Finished`，无 warning。

```sh
cargo fmt --all --check
```

通过，无输出；`cargo fmt --all` 只改了本卡新增文件。

**为什么排除 `tTerm`（`src-tauri`）。** 不加 `--exclude` 时 `cargo test --workspace` / `cargo build --workspace` 在 `src-tauri` 的构建脚本处失败，与本卡改动无关：

```text
error: failed to run custom build command for `tTerm v0.8.1`
  D:\code\rust\tTerm\scripts\prepare-conpty.ps1 : Exception calling "ExtractToFile"
  with "3" argument(s): "Access to the path
  'D:\code\rust\tTerm\src-tauri\resources\conpty\conpty.dll.new' is denied."
  Failed to prepare ConPTY. Check the download/cache error above and rebuild.
```

执行者记录同一个脚本直接执行时退出码为 0，而由 cargo 构建脚本派生时写入该路径被拒。主控在沙箱外复跑完整 workspace 检查也遇到同一错误，根因尚未确定。`src-tauri` 依赖 core，不能用排除它的结果代表集成验证；**这不等于 `cargo test --workspace` 通过**。

**未运行**（留给评估者）：`pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`、`cargo clippy --workspace --all-targets -- -D warnings`（未排除版本）、`cargo test --workspace`（未排除版本）。本轮没有改前端，但这不代表这些检查已通过。`node scripts/migration/P1-11c-oracle.mjs` 也没有重跑：模块代码没有改 TS，期望值用的是已提交的 oracle 输出。

## 偏差

1. **`with_current_local_shell` 返回克隆值。** TS 在找不到匹配的 shell 时返回同一个对象引用，Rust 返回等值克隆；调用方若依赖引用相等需自行判断，测试用 `PartialEq`（即规范化 JSON 字段相等）断言。
2. **`savedLocalShell`/`savedLocalShellUpdates`/`removedLocalShell` 返回 `ShellListUpdate`。** TS 用对象展开返回“部分 `AppConfig`”，键是否存在（`terminal_shell_custom_path` 等五个字段 vs 仅 `local_shells`）本身是语义的一部分。Rust 用 `ShellListUpdate { local_shells, default_shell: Option<DefaultShellChange> }` 表达同一件事：`Edited` 对应“只跟进 path/args”，`Chosen` 对应“整组五个字段”，`None` 对应“只改列表”。合并回 `AppConfig` 留给 P3，本卡不做写入。
3. **`movedLocalShell` 的偏移量是 `i32`。** TS 是 `-1 | 1` 字面量联合，Rust 用 `i32` 加“越界即原样返回”的处理；越界值（例如 `0`）会得到不变的列表，与 TS 不可表达该输入并不冲突。
4. **`newLocalShellId` 用 `uuid::Uuid::new_v4()`**（core 已有依赖），与 `crypto.randomUUID()` 同为 UUID v4 小写带连字符格式，测试断言长度、字符集与可解析性。
5. **只保留 `shell_type_from_id`，没有反向的 `shell_type_id`。** 保存的 shell id 全程是字符串，只有标签查表需要枚举；`shell_type_from_id` 因此只负责“已知 id → 目录键”，并有与 serde 序列化结果逐项相等的测试（`rename_all = "kebab-case"` 漂移会立即失败）。
6. **标签翻译接口是 `impl Fn(&str, &str) -> String`**（与 P1-11c 计划的“需要确认”第 6 项一致）。`shell_label_key` 另以 `&'static str` 暴露键名，便于 P3-03 的 i18n 注册表复用；`translation_keys!` 里目前没有这些键，本卡不扩展该注册表。
7. **保存的 `sessionNonce` 只按 JSON 数字解释。** 字段缺失或为 `null` 时按 0，非数字（字符串、布尔等）也按 0；TS 的类型声明是 `number`，两个客户端写入的都是数字，因此只有损坏的文件才可能走到这条分支（TS 此时会做字符串拼接等隐式转换，本卡不复现）。
8. **`with_local_shell` 只在旧连接的 `cwd` 是字符串时携带它。** TS 的 `cwd` 类型是 `string | undefined`；非字符串的损坏值会被丢弃，而 TS 会把原值抄进新连接。
9. **连接里的非文本字段一律按“不相等”处理。** `terminalShell`/`terminalShellId`/`terminalShellCustomPath` 等字段若不是字符串，`starts_shell`/`starts_as_saved` 返回 `false`（TS 的严格相等对数字与字符串同样为假；`startsAsSaved` 在 TS 里会因对数字调用 `.trim()` 抛错，本卡不复现该异常）。

## 遗留问题

### 主控复核（2026-11-18）

- 实际执行代理为 pi，模型标识 `magpie/auto/deepseek-v4.1-flash`；完成实施及一轮审查返工，没有切换模型。旧配置中的 `sunline-ais` 标识不在本机当前模型列表中。
- 主控要求修复未知 shell id 被回退成 Auto 的行为偏差，并补充原始 sessionNonce 的数字边界测试；没有修改全局 Tab 类型或持久化结构。
- `pnpm lint`、`pnpm format:check`、`pnpm check:tokens`、`pnpm typecheck` 通过；`pnpm test` 在沙箱 spawn EPERM 后于沙箱外通过：183 文件、1354 测试，另有 parity 2 项、migration 8 项；`pnpm check:size` 在沙箱外通过，1432 文件。
- `cargo fmt --all --check` 通过。完整 `cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo test --workspace` 均在沙箱外尝试，均被上述 ConPTY staging 写入拒绝阻止；不得记为通过。
- 本批保持未提交、未合并、未推送，未改状态总表。因完整检查未通过，停止推进下一批；本报告不是整张 P1-11c 或 P3-02 的验收通过结论。

### 提交前复验

- ConPTY 准备脚本已改为读取固定版本压缩包条目的 SHA-256，与目标文件比较，只在缺失或不匹配时解压。在独立测试目录验证了首次安装、损坏后修复以及内容一致时不重写；没有跳过压缩包校验。
- 六项 pnpm 检查、`cargo fmt --all --check` 和 `cargo clippy --workspace --all-targets -- -D warnings` 全部通过；39 项本地 shell 回归也全部通过。
- `cargo test --workspace --no-fail-fast` 使用 `target/workspace-test-tmp` 作为 `TMP`、`TEMP`、`TMPDIR`。管道环境中 Ctrl+C 测试超时，带 PTY 复跑时通过；该轮唯一失败为 `agent_integration::console_report::tests::finds_this_process_and_its_parent`：进程快照含测试进程，但不含其父 PID。单独运行也失败；根因未确认。
- 随后从仓库根另起 Windows PowerShell 进程，沿用工作区临时目录，执行 `cargo test --workspace --no-fail-fast`。退出码 101，仍只有 `finds_this_process_and_its_parent` 失败，断言为 `processes.contains_key(parent)`。这说明换一个 PowerShell 进程不足以消除问题，但不能排除它继承了宿主的进程可见性限制。ConPTY 修复和 Shell 迁移仍保留在工作区，未提交。

- c2 的其余部分仍未实现：`appConfig` 默认值与限额常量、`appConfigNormalize`（含 `normalizeAppState`、`normalizeSecretStatus`），以及 c1 的 `TabSettings` 与 `AppConfig` 三套类型转换。本卡没有 `AppConfig` 层入口，`ShellConfig::from(&AppConfig)` 只是借用视图。
- P3-02 未完成；`ShellListUpdate` 如何写回 `AppConfig`、`build_shell_entries` 如何接 i18n（`translation_keys!` 注册表）、`terminal_shell_label` 的闭包形态是否与 P3-03 最终接口一致，都要在 P3 落地时确认。
- 未做平台运行验证：本卡只有纯逻辑与 `cargo test`（Windows）。macOS/Linux 的真实 shell 探测列表与 WSL 行为仍待 CI 与人工验证（决策 016）。
- `config::local_shells::program_name`（后端迁移用的那份）与新的 `local_shells::program_name` 目前同时存在：前者用于旧版默认 shell 迁移（去全部引号、trim 词干），后者逐字复现前端。本卡按“不改既有行为”没有合并它们；是否让迁移改用前端语义需要单独判断（会影响已迁移 shell 的 id）。
