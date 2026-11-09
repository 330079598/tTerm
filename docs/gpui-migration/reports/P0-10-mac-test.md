# P0-10 macOS 字形测试修复

## 结论

完成。字形排布测试改为由 `harness = false` 的集成测试二进制在 `main` 主线程创建真实平台文字系统；macOS 上不再因 `Mac platform not created on main thread` 失败。

## 原因与入口选择

`gpui_kit::platform::current_platform(true)` 是 `gpui-pre-platform` 提供的公开跨平台构造入口。它在 macOS 上调用 `gpui_macos::MacPlatform::new(true)`，而 `MacPlatform::new` 通过 `main_thread_marker()` 强制要求主线程。`TestAppContext` 和 `HeadlessAppContext` 都只接受调用方传入的 `PlatformTextSystem`；它们不会自行提供 CoreText 实例。仓库当前也没有直接依赖 `gpui-pre-apple`、`gpui-pre-wgpu` 或 `gpui-pre-windows` 的入口，且任务禁止新增依赖，因此不能在测试工作线程上绕过平台构造而仍保留三平台真实文字系统。

选用任务卡允许的 `harness = false` 路径：`tests/shaping.rs` 的 `main` 在测试进程主线程调用 `current_platform(true)`，再把平台的文字系统交给 `TextSystem`/`WindowTextSystem`。这样 macOS 使用 CoreText，Windows 使用 DirectWrite，Linux 使用 cosmic-text；测试断言和原有字形排布逻辑不变。

## 改动

- 从 `crates/tterm-terminal-view/src/view/tests.rs` 移出 `shaping_places_each_glyph_on_its_column`，避免 Rust 测试 harness 在线程池工作线程执行。
- 新增 [`crates/tterm-terminal-view/tests/shaping.rs`](../../crates/tterm-terminal-view/tests/shaping.rs)，以 `harness = false` 声明，主线程运行同一断言。
- 将 `view::row` 设为公开模块，使集成测试复用生产的 `plan_row` 与 `shape_row`，没有复制排布实现。
- 搜索 workspace 中所有 `current_platform` 调用；其余命中位于 `tterm-gpui` 示例和 `tterm-parity` 的应用入口路径，不是 cargo 测试工作线程调用，未作改动。

## 验收

| 验收条件                                                                 | 结果                                       | 证据                                                                                                                                                           |
| ------------------------------------------------------------------------ | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS、Windows、Linux 在 cargo test 工作线程里运行且使用真实平台文字系统 | 通过（macOS 已验证；Windows/Linux 未验证） | `cargo test -p tterm-terminal-view --test shaping` 在 macOS 主线程测试二进制中通过；代码使用 `current_platform(true).text_system()`，未使用 fake text system。 |
| 不使用 `ignore`、平台 cfg 跳过或降低断言                                 | 通过                                       | 新测试无 `ignore`/跳过 cfg；断言仍检查 3 个 glyph、列 `[0, 1, 2]` 和零列内偏移。                                                                               |
| workspace 中其他测试线程调用 `current_platform` 一并修复                 | 通过                                       | `rg -n 'current_platform\\(' --glob '*.rs'` 复核；除新测试外仅有示例和 parity 应用入口。                                                                       |
| macOS `cargo test --workspace` 失败项为 0                                | 通过                                       | 见“检查”：沙箱外 `cargo test --workspace` 退出码 0；评估者复跑结果相同。                                                                                       |

## 检查

已完成：

- `cargo test -p tterm-terminal-view --lib`：137 passed, 0 failed, 0 ignored。
- `cargo test -p tterm-terminal-view --test shaping`（沙箱内）：AppKit 粘贴板服务不可用，失败于系统服务初始化，不是测试断言。
- 同一命令（沙箱外）：进程正常退出，0 failed。
- `cargo test --workspace`（macOS、沙箱外）：退出码 0；workspace 汇总中所有失败数为 0。主测试包为 `817 passed; 0 failed; 52 ignored`（这些 ignored 是仓库既有的真实 sshd/外部服务测试），`tterm-terminal-view` 为 `137 passed; 0 failed`，`tests/shaping.rs` 正常运行并退出 0；`macos_lists_the_applications_for_text_with_icons` 为 `ok`。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size`：通过；Vitest `1294 passed`，文件大小检查 `1034 files within their limits`。
- `git diff --check`：通过。

macOS 沙箱中已知的 `[人工]` 项 `macos_lists_the_applications_for_text_with_icons` 已按要求在沙箱外复跑并通过。

## 偏差

- Windows、Linux 未在本机验证，原因是当前环境为 macOS Apple Silicon；两平台使用的真实文字系统构造分别由 `current_platform(true)` 选择 DirectWrite 与 cosmic-text。

## 遗留问题

无新的代码遗留问题。平台矩阵中的 Windows/Linux 验证仍需各自 CI 或真机结果。

## 评估（2026-11-09）

- 结论：**通过**。
- 评估者在本机沙箱外重跑 AGENTS.md 的全部检查：pnpm 六项通过，fmt 与 clippy 通过；`cargo test --workspace` 不跳过任何测试，1136 项通过、0 项失败，`tests/shaping.rs` 在主线程运行并正常退出。
- 用 `harness = false` 是合理的选择：`TestAppContext` 与 `HeadlessAppContext` 都要调用方自己提供平台文字系统，而不新增依赖时，要拿到真实的文字系统只有 `current_platform` 这一个入口。`view::row` 改为公开，与已经公开的 `font`、`element` 同级，公开项都有文档注释，可以接受。
- 评估时把验收表中“macOS `cargo test --workspace` 失败项为 0”一行从“待验证”改为“通过”，使它与“检查”一节一致。
- 遗留问题：Windows 与 Linux 未验证，要等 CI 或真机。这个测试二进制自己运行，不输出 libtest 的“test … ok”行，`--skip`、`--list` 这类过滤参数对它也不起作用。
