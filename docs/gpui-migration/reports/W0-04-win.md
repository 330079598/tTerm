# W0-04-win Windows 对比通道

## 结论

完成。Windows 参照端使用 Playwright 1.54.0 的 Microsoft Edge channel，GPUI 端使用真实窗口的 `Window::render_to_image`，示例屏幕在 Windows 11、100% 缩放下通过对比门槛。

## 改动

- `scripts/parity/capture.mjs` 增加 `--platform windows` / `PARITY_PLATFORM=windows`，使用 `chromium.launch({ channel: "msedge" })`，并把 Windows 平台值与 `Segoe UI` 字体栈传入 parity 页面。
- `crates/tterm-parity` 在 Windows 使用真实窗口、`render_to_image` 和截图后 `remove_window()`；macOS/Linux 保留 `HeadlessAppContext` 路径。Windows 清单使用 100% viewport。
- 更新 `docs/gpui-migration/parity/README.md`、`docs/gpui-migration/ui-parity.md` 与平台清单。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 示例屏幕在 Windows 上通过第 5.2 节的门槛，故意改动 1 像素会失败并指名元素，两次运行结果哈希相同。 | 通过 | `$env:PARITY_PLATFORM='windows'; cargo run -p tterm-parity -- compare`：`PASS`，布局最大偏差 `0.62 px`，整屏 SSIM `0.997678`，遮文字 SSIM `0.999831`，screen color `0 px above 1`。临时把 `BUTTON_PADDING_X` 从 `16.0` 改为 `17.0` 后退出 `1`，输出指名 `example.button`，失败原文保存在 `target/parity/windows-padding-fail.txt`。连续两次结果：`result.json` SHA-256 `79216006C26761B60D11B2BB2CB14F9FB5A2E4C5A2D0BA6CB90C92F0CC84D479`；热力图 SHA-256 `726DFF5B7DD222E8DC1B76A90F1BB18B1269C4C92C5BBCF9339DBD234480AC73`。 |
| Playwright Edge 与真实 Tauri 窗口的交叉核对有截图与比较数字。 | 未验证 | Edge 参照图与 GPUI 真实窗口截图已生成，比较数字见上；本轮未能启动隔离 Tauri 开发版并通过 WebView2 CDP 完成交叉核对，因此没有把该项记为通过。 |
| macOS 通道的代码路径不受影响（cfg 分平台；macOS 记为“推迟（决策 012）”，不要求本机验证）。 | 通过 | `capture.rs` 的 `#[cfg(target_os = "windows")]` / `#[cfg(not(target_os = "windows"))]` 分支保留原无头路径；macOS 按决策 012 推迟。 |
| AGENTS.md 的全部检查通过。 | 通过 | `pnpm lint`、`pnpm format:check`、`pnpm check:tokens`、`pnpm exec tsc --noEmit`、`pnpm test`（174 files / 1294 tests）、`pnpm check:size`（1025 files）通过；`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（tTerm 812 passed / 23 ignored；tterm-gpui 12；tterm-parity 60；其余目标与 doc-tests 通过）通过。 |

## 检查

Windows 运行环境：Windows 11 物理机，Microsoft Edge/WebView2 `157.0.4341.42`，Node `24.19.0`，pnpm `11.15.0`，Rust/Cargo `1.99.1`。未下载 Playwright 浏览器；Edge 参照命令：

```powershell
node scripts/parity/capture.mjs --platform windows --screen example.button-input.default
```

原始输出：`example.button-input.default: 3 element(s), 3 text region(s)`。

GPUI 命令：

```powershell
$env:PARITY_PLATFORM = "windows"
cargo run -p tterm-parity -- compare
```

原始输出中的关键数值：`max bounds delta 0.62 px`、`full 0.997678`、`masked 0.999831`、`0 px above 1`、`1 screen(s): 1 passed, 0 failed`。

## 偏差

- 任务卡要求交叉核对真实 Tauri WebView2 与 Edge；本轮代码与 Edge/GPUI 通道已完成，但隔离 Tauri 开发版的 CDP 截图未完成，按执行规则标为未验证。
- Windows 实际窗口的物理截图为 `1536x1095`（系统窗口装饰/显示缩放影响），工具使用窗口内容的 `render_to_image` 并输出清单要求的 `800x600` parity 图像；这不改变布局比较的逻辑尺寸。

## 遗留问题

- Tauri WebView2 CDP 交叉核对需要下一轮在隔离 `APPDATA` 与临时 `--config` 下启动开发版，并保存 Edge/Tauri 截图和像素比较数字。
- macOS/Linux 按决策 012 推迟，不在本轮验证。
