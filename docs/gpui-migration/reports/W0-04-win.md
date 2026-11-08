# W0-04-win Windows 对比通道

## 结论

完成。Windows 参照端使用 Playwright 1.54.0 的 Microsoft Edge channel，GPUI 端使用真实窗口的 `Window::render_to_image`，示例屏幕在 Windows 11、100% 缩放下通过对比门槛。

## 改动

- `scripts/parity/capture.mjs` 增加 `--platform windows` / `PARITY_PLATFORM=windows`，使用 `chromium.launch({ channel: "msedge" })`；生产 CSS 和 parity 页面样式保持不变，字体族在比较工具中归一化。
- `crates/tterm-parity` 在 Windows 使用真实窗口、`render_to_image` 和截图后 `remove_window()`；macOS/Linux 保留 `HeadlessAppContext` 路径。Windows 清单使用 100% viewport。
- 更新 `docs/gpui-migration/parity/README.md`、`docs/gpui-migration/ui-parity.md` 与平台清单。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 示例屏幕在 Windows 上通过第 5.2 节的门槛，故意改动 1 像素会失败并指名元素，两次运行结果哈希相同。 | 通过 | `$env:PARITY_PLATFORM='windows'; cargo run -p tterm-parity -- compare`：`PASS`，布局最大偏差 `0.62 px`，整屏 SSIM `0.997678`，遮文字 SSIM `0.999831`，screen color `0 px above 1`。临时把 `BUTTON_PADDING_X` 从 `16.0` 改为 `17.0` 后退出 `1`，输出指名 `example.button`，失败原文保存在 `target/parity/windows-padding-fail.txt`。连续两次结果：`result.json` SHA-256 `79216006C26761B60D11B2BB2CB14F9FB5A2E4C5A2D0BA6CB90C92F0CC84D479`；热力图 SHA-256 `726DFF5B7DD222E8DC1B76A90F1BB18B1269C4C92C5BBCF9339DBD234480AC73`。 |
| Playwright Edge 与真实 Tauri 窗口的交叉核对有截图与比较数字。 | 通过 | 在 `target/w0-04-win/tauri.json` 临时标识 `com.stone.tTerm.gpui-dev`、空 APPDATA 和 WebView2 调试端口 `9458` 下启动 Tauri；CDP 读取 `list_profiles=[]`、`session.tabs=[]`、`active_tab_id=null` 后打开同一 parity URL 并保存 `target/w0-04-win/tauri.png`。Edge 图为 `target/w0-04-win/edge.png`。两图均 `800x600`：平均通道绝对差 `0.0159020833`，最大通道差 `159`，任一通道变化 `94` 像素，通道差 >8 的像素 `94`，全局灰度 SSIM `0.981893215`；原始输出见 `target/w0-04-win/cross-check.txt`。 |
| macOS 通道的代码路径不受影响（cfg 分平台；macOS 记为“推迟（决策 012）”，不要求本机验证）。 | 通过 | `capture.rs` 的 `#[cfg(target_os = "windows")]` / `#[cfg(not(target_os = "windows"))]` 分支保留原无头路径；macOS 按决策 012 推迟。 |
| AGENTS.md 的全部检查通过。 | 通过 | `pnpm lint`、`pnpm format:check`、`pnpm check:tokens`、`pnpm exec tsc --noEmit`、`pnpm test`（174 files / 1294 tests）、`pnpm check:size`（1025 files）通过；`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（tTerm 812 passed / 23 ignored；tterm-gpui 12；tterm-parity 60；其余目标与 doc-tests 通过）通过。 |

## 检查

Windows 运行环境：Windows 11 物理机，Microsoft Edge/WebView2 `157.0.4341.42`，Node `24.19.0`，pnpm `11.15.0`，Rust/Cargo `1.99.1`。未下载 Playwright 浏览器；Edge 参照命令：

```powershell
node scripts/parity/capture.mjs --platform windows --screen example.button-input.default
```

原始输出：`example.button-input.default: 3 element(s), 3 text region(s)`。

GPUI 原生窗口尺寸核对：`bounds=800px × 600px`、`viewport=800px × 600px`、`scale_factor=1`、`render_to_image=800x600`。这是按清单尺寸直接创建的 1 倍客户区图像，没有缩放重采样；系统窗口装饰只影响屏幕上窗口的外框，不进入 `render_to_image` 返回值。此前观察到的 `1536x1095` 是系统桌面上窗口装饰和显示区域的物理窗口外观，不是工具写入的 parity 图像。

GPUI 命令：

```powershell
$env:PARITY_PLATFORM = "windows"
cargo run -p tterm-parity -- compare
```

原始输出中的关键数值：`max bounds delta 0.62 px`、`full 0.997678`、`masked 0.999831`、`0 px above 1`、`1 screen(s): 1 passed, 0 failed`。

## 偏差

- Tauri 交叉核对首次使用 `create: true` 的临时配置时触发了应用 setup 重复创建 `main` WebView；改为 `create: false` 后按现有应用 setup 成功启动。该临时配置只写在 `target/`，未提交。
- Windows 桌面上窗口外框的物理尺寸不等于客户区；本轮记录的 GPUI 渲染证据是客户区 `800x600`、1 倍缩放，未对图像做重采样。

## 遗留问题

- macOS/Linux 按决策 012 推迟，不在本轮验证。
