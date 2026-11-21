# P1-11c 整卡复核（2026-11-21）

按 [P1-11c 计划](P1-11c.md) 从 `src/lib` 重新清点：13 个模块、112 个导出、9 个测试文件中的 55 个用例。以下各表合起来覆盖整张卡，不把浏览器机制记作原生实现。

## 导出对照（112 项）

| 范围 | 数量 | 逐项对照 |
| --- | ---: | --- |
| `tabSettings`、`terminalFont`、`scrollback`、`shellCommands`、`sshShellIntegration`、`remoteShellIntegration` | 27 | [c1 导出表](P1-11c-c1.md#27-个-ts-导出--rust-项) |
| `localShells`、`terminalShells` | 31 | [shell 导出表](P1-11c-c2-shells.md#28-个-ts-导出--rust-项)，包括后面的 3 个 shell profile 项 |
| `appConfig`、`appConfigNormalize` | 45 | 下表（同名常量合列） |
| `uiStyle`、`spinnerStyle`、`startup` | 9 | 下方浏览器专用表 |

| TS 导出 | Rust 去向（路径相对于 `tterm_core`） |
| --- | --- |
| `SecretBackendStatus`、`SecretStorageMode` | `ssh::secret_store::types` 下的同名类型 |
| `SavedSecretEntry` | `profiles::types::SavedSecretSummary`（五个同名序列化字段）；Tauri 命令另有同形 DTO |
| `LocalShell`、`AppConfig`、`AppState` | `config::local_shells::LocalShell`、`config::AppConfig`、`app_state::AppState` |
| `TerminalLogFormat`、`TerminalRenderer`、`BackgroundThrottling`、`CloseBehavior`、`WindowBlurMaterial` | 保留 `AppConfig` 的字符串字段 `terminal_log_format`、`terminal_renderer`、`background_throttling`、`close_behavior`、`window_blur_material`；合法值由归一化入口约束，不新增持久化枚举 |
| `UiStyle`、`ProfileSort`、`SftpExternalUpload`、`BellStyle`、`NotificationSoundName` | 同上，对应 `ui_style`、`profile_sort`、`sftp_external_upload`、`bell_style`、`notify_sound_name` |
| `getDefaultTerminalRenderer`、`DEFAULT_TERMINAL_FONT_FAMILY` | `config::limits::DEFAULT_TERMINAL_RENDERER`、`config::terminal_font::detected_font_family()` |
| `NOTIFICATION_SOUND_NAMES`、`HIDDEN_RENDERER_RELEASE_SECS_OPTIONS`、`DEFAULT_HIDDEN_RENDERER_RELEASE_SECS`、`DEFAULT_TERMINAL_FONT_SIZE` | `config::limits` 同名常量 |
| `DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS`、`PREDICTIVE_ECHO_THRESHOLD_MIN_MS`、`PREDICTIVE_ECHO_THRESHOLD_MAX_MS` | `config::limits` 同名常量 |
| `TERMINAL_LOG_MAX_RETENTION_DAYS`、`TERMINAL_LOG_MAX_TOTAL_MB`、`MAX_BANDWIDTH_LIMIT_KIB`、`RECONNECT_MAX_ATTEMPTS_MIN`、`RECONNECT_MAX_ATTEMPTS_MAX` | `config::limits` 同名常量 |
| `TERMINAL_FONT_SIZE_RANGE`、`TERMINAL_LINE_HEIGHT_RANGE`、`TERMINAL_LETTER_SPACING_RANGE`、`WINDOW_OPACITY_PERCENT_RANGE`、`WINDOW_BLUR_RADIUS_RANGE`、`NOTIFY_COMMAND_MIN_SECS_RANGE`、`TOAST_MAX_VISIBLE_RANGE` | `config::limits` 同名闭区间 |
| `defaultConfig`、`defaultAppState`、`defaultSecretStatus` | `AppConfig::default()`、`AppState::default()`、`ssh::secret_store::normalize::normalize_secret_status(&Value::Null)` |
| `normalizeConfig`、`normalizeUiScalePercent`、`normalizeTerminalLineHeight` | `config::app_config_normalize::{normalize_config, normalize_ui_scale_percent, normalize_terminal_line_height}` |
| `normalizeAppState`、`normalizeSecretStatus` | `app_state::normalize::normalize_app_state`、`ssh::secret_store::normalize::normalize_secret_status` |

| 浏览器专用导出 | 不移植理由 |
| --- | --- |
| `applyUiStyle`、`preloadUiStyle` | 设置 DOM class / 读取 localStorage；原生设置值仍由 `AppConfig.ui_style` 提供。 |
| `getSpinnerStyle`、`setSpinnerStyle`、`subscribeSpinnerStyle` | React 树的进程内订阅器。`TabAgentIndicatorConfig` 保留样式值，原生界面订阅属于 P3。 |
| `markConfigReady`、`announceThemeReady`、`markSessionReady`、`onAppReady` | HTML 启动画面的 `CustomEvent` 协议，不是 GPUI 的启动接口。 |

## 用例对照（55 项）

[c1 的 30 行测试表](P1-11c-c1.md#30-个-ts-用例--rust-测试)与 [shell 的 19 行测试表](P1-11c-c2-shells.md#19-个-ts-用例--rust-测试)给出原用例和 Rust 测试名；本次核对了表中标识符。剩余 6 项保留 TS 测试及 Node 实跑，不虚设 Rust 测试：

| 原 TS 测试 | 原生侧边界 |
| --- | --- |
| `uiStyle`: marks the page for islands and clears the mark for classic | DOM class |
| `uiStyle`: starts the next launch in the style last applied | 浏览器缓存的启动样式 |
| `uiStyle`: starts the next launch classic when classic was last applied | 同上，classic 分支 |
| `uiStyle`: starts in islands when nothing was remembered | 同上，空缓存分支 |
| `spinnerStyle`: starts with the default working style | React 单例；Rust 配置默认值已有逐字段测试 |
| `spinnerStyle`: tells subscribers about a change, and only a change | React 单例通知，不是 GPUI 实体订阅验收 |

## 验收与交接

Windows、Linux、macOS 稳定版与 macOS 预发布版的 Node 模拟各 72/72。与历史快照的 `values` 比较只出现 `defaultsParity` 来源路径随迁移变化。本次补上 `settings_limits_and_sound_choices_match_typescript`，从已有 TS 快照校验离散选项、数值和范围端点。另见[归一化阶段报告](P1-11c-c2-normalize.md)的 100 个默认字段、173 个局部输入与类型差异。

Windows 的六项 pnpm 检查、`cargo fmt --all --check`、严格 workspace/all-targets Clippy 和 `cargo test --workspace` 全部通过。原生范围按决策 016：Windows 验收；macOS 待补、Linux 推迟。此卡交付共享逻辑，不含 P3-02 实体、运行时配置通知、GUI 人工验收或发布关卡。进入 P3-02 前，应先检查其已有工作树中的未提交改动。
