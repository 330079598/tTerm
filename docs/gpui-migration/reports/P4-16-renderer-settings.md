# P4-16 渲染器设置：移除 WebGL/Canvas 与隐藏释放项

2026-11-25，共享工作树。范围是 GPUI 迁移设置；Tauri/React 的 WebView 渲染器行为不在本次改动内。

## 结论

完成（共享工作树未提交）。GPUI 设置页和 GPUI 使用的共享搜索索引不再把 `terminal_renderer`、`hidden_renderer_release_secs` 当作可迁移项；共享 `AppConfig` 字段、默认值、归一化和旧配置读回保留，GPUI 页面保存不会改写另一端写入的值。

## 改动

- `crates/tterm-core/src/settings/search_index.rs`：移除 `fontSettings.renderer` 与 `fontSettings.rendererRelease` 两个搜索条目，并在模块注释中说明它们是 WebView 专有项。
- `crates/tterm-core/src/settings/search_index/tests.rs`：新增回归测试，锁定 GPUI 共享索引不再列出这两个键。
- `crates/tterm-core/src/config/mod.rs`、`limits.rs`：只补充文档，明确两个字段和默认值服务于 Tauri/WebView，GPUI 原生渲染不读取。
- `crates/tterm-core/src/config/terminal_font_file/page_tests.rs`：新增测试，证明 GPUI 页面保存后 `terminal_renderer = "canvas"` 与 `hidden_renderer_release_secs = 30` 保持不变。
- `crates/tterm-gpui/src/terminal_demo/window/tests.rs`：启动归一化测试改用 `ui_scale_percent`，不再把隐藏释放字段当作 GPUI 设置示例。
- `docs/gpui-migration/settings-inventory.md`：把两个 WebView 渲染器字段排除出 GPUI 迁移计数。并发任务随后接入了命令标记与本机 Shell 集成，清单当前显示 16/16 个与 GPUI 相关的字段已迁移，见 [P4-16 shell integration 报告](P4-16-shell-integration.md)。
- `docs/gpui-migration/reports/P4-16-renderer-settings.md`：本报告。

本批未改动：`docs/gpui-migration/status.md`、`docs/gpui-migration/dispatch.md`、命令标记与本机 Shell 集成实现、React/Tauri 的渲染器控件以及 `useTerminalRenderer.ts` / `rendererRelease.ts`。

审阅结果：

- `AppConfig.terminal_renderer` 默认 `webgl`，归一化只接受 `webgl` / `canvas`；React 的 `FontSettingsTab`、`useTerminalSettingsDraft`、`useTerminalRenderer` 与 `rendererRelease` 仍在引用。
- `AppConfig.hidden_renderer_release_secs` 默认 10，归一化只接受 0/10/30/60/300；React 生命周期用它决定隐藏终端何时释放 WebGL/Canvas。
- GPUI 生产代码没有读取这两个字段；GPUI 的终端视图和生命周期没有 renderer 切换或 renderer 释放逻辑，因此这里没有可删的 GPUI 内部实现。
- `save_terminal_page` 从最新 `config.json` 读取，只写发生变化的页面字段；`compatible_file` 保留未知键，旧 renderer 值不会因 GPUI 保存而丢失。

## 验收

| 验收条件 | 结果 | 证据 |
| --- | --- | --- |
| GPUI 设置不再把终端渲染器 WebGL/Canvas 列为可迁移项 | 通过 | `search_index.rs` 删除 `fontSettings.renderer`；`search_index/tests.rs` 的负向断言 |
| GPUI 设置不再把隐藏渲染器释放时限列为可迁移项 | 通过 | `search_index.rs` 删除 `fontSettings.rendererRelease`；清单把它标为 WebView 专有项，不计入 GPUI 字段 |
| 保留旧配置互通，不破坏 Tauri/WebView 仍在使用的字段 | 通过 | `AppConfig` 字段、默认值和归一化未删；`a_page_save_preserves_the_webview_renderer_choices` |
| 不删除仍由 GPUI 生命周期使用的内部逻辑 | 通过 | `rg` 显示 GPUI 生产代码无这两个字段的读取；终端生命周期文件未改 |
| 不修改 `status.md`、`dispatch.md`、命令标记与本机 Shell 集成代码 | 通过 | 改动路径清单 |
| 新增本批报告 | 通过 | 本文件 |

## 检查

- `cargo test -p tterm-core --lib settings::search_index`：4 个测试通过，其中 `the_gpui_index_omits_the_webview_renderer_settings` 新增并通过。
- `cargo test -p tterm-core --lib config::terminal_font_file::page_tests::a_page_save_preserves_the_webview_renderer_choices`：1 个测试通过。
- `cargo test -p tterm-core --lib config::app_config_normalize::tests::a_strictly_loaded_config_is_normalized_like_the_tauri_frontend`：1 个测试通过。
- `cargo test -p tterm-gpui --lib startup_config_normalizes_before_views_read_it`：1 个测试通过。
- `cargo fmt --all --check`：通过。
- `pnpm format:check`：通过。
- 测试在 Windows 沙箱内会以 `STATUS_DLL_INIT_FAILED` 退出，以上测试是在沙箱外运行、并把 `TEMP`/`TMP` 指向工作树内临时目录后取得的；测试代码和断言未因环境改动。

## 偏差

- GPUI 共享搜索索引不再逐项镜像 React 的 `settingsSearchIndex.ts`；它保留 14 页目录，只去掉 WebView 专有的 `terminal_renderer` 与 `hidden_renderer_release_secs` 两个字段，其余条目是否显示由 GPUI 页面和平台再过滤；模块注释和清单已说明。
- 历史报告、基线与证据 JSON 仍包含这两个字段的旧值；它们是历史记录，本次不重写。
- 本批只验证共享文件读回，不做 Tauri 与 GPUI 双客户端桌面互写，也不做原生桌面视觉验收。
- 共享工作树中的 `p416_shell_marks` 并发批次同时修改了终端页结构、`page_tests.rs` 和 GPUI 设置页；本报告只把与渲染器字段有关的改动和测试算作本批证据，命令标记与本机 Shell 集成不计入本批实现。

## 遗留问题

建议由主代理决定的兼容策略：

1. **推荐保留字段**：在 Tauri/WebView 渲染器还存在的阶段，保留 `AppConfig.terminal_renderer`、`hidden_renderer_release_secs`、默认值和归一化；GPUI 不显示、不读取、不写入，但保存时原样保留旧值。
2. **等待 Tauri 退役再删字段**：若以后删除，应先移除 React 的渲染器控件和 `useTerminalRenderer` / `rendererRelease` 生命周期，再提供旧的 `config.json` 迁移；现在直接删会让 Tauri 丢失用户选择，或让旧键变成仅由兼容层保留的未知字段。
3. **保持 GPUI 搜索排除**：若以后用脚本从 React 索引生成 GPUI 索引，需要显式跳过这两个 WebView 专有项，并保留当前负向测试，避免它们重新出现在迁移搜索里。
4. **隐藏释放字段暂无 GPUI 语义**：GPUI 没有 renderer 可释放，当前保持 no-op 兼容读取；若未来要给后台标签做内存或绘制生命周期策略，应另建设置卡，不要复用这个 WebView 字段名。
5. **可选的互通验收**：补一个 Tauri → GPUI → Tauri 的 `config.json` 轮转测试，覆盖 `terminal_renderer`、`hidden_renderer_release_secs` 和未知键；本批已有 GPUI 侧保存保留测试，但没有双客户端桌面证据。

本批不改变命令标记与本机 Shell 集成的依赖关系；并发批次 `P4-16-shell-integration` 已接入这两项，清单当前为 16/16。
