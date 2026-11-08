# 011 文件拖出原生适配方案

- 日期：2026-11-08
- 决策人：待评估者及决策人确认
- 状态：提议
- 依据：[P0-19 报告](../reports/P0-19.md)、[P0-07 评估](../gpui-patches.md#41-文件拖出p0-19-相关)，固定 gpui-kit 0.9.0 / gpui-pre 0.3.11。

## 决定草稿

优先将现有 tTerm 的平台拖出适配接到 GPUI 原生窗口。当前只证明 Windows 示例编译、Shell 数据对象格式和本地 200 MB 内容管线；真实资源管理器拖放尚待人工，不能据此声称 Windows 已通过，更不能声称三平台可用。本轮不采用 nyakang/zed 补丁。

| 平台          | 依据与可行性                                                                                                                                                                                                                                                          | 当前限制                                                                                                                                               |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Windows       | `src-tauri/src/drag_out/windows.rs` 的 `data_object`、`run_drag`、`start_watcher` 可与 WebView 解耦；gpui-pre-windows 0.3.11 `src/platform.rs:187` 初始化 OLE。独立示例用当前 PID/标题检查的前台 HWND 调用相同 Shell API。                                            | 编译、真实 IDataObject/CF_HDROP 和 200 MB 文件管线测试通过；原生拖放未验证 `[人工]`。示例用预知 TEMP 目录替代 Shell watcher，不能外推任意目录。        |
| macOS         | `src-tauri/src/drag_out/macos.rs` 的 `begin_drag` / `NSFilePromiseProviderDelegate`；gpui-pre-macos 0.3.11 `src/window.rs:2459` 的 `HasWindowHandle` 返回 NSView，`:2277` 已通过 AppKit 实现现有文件拖出。可将 Tauri 的 view 参数换成 GPUI NSView，保留文件承诺路线。 | 源码判断可行，未运行验证；本轮无 macOS 演示实现或编译，需 Mac/Finder 与已固定的 objc2 相关依赖核验。不要把生产 delegate 立即 completion 当成下载完成。 |
| Linux Wayland | gpui-pre-linux 0.3.11 `src/linux/wayland/window.rs:2110` 与 `client.rs:673` 仅处理 `ExternalDragPayload::Files`，通过 `text/uri-list` 传现有路径。                                                                                                                    | 当前方案不支持延迟内容；未运行验证。拖动开始时先下载完再导出会延迟反馈，不能当文件承诺的等价能力。                                                     |
| Linux X11     | gpui-pre-linux 0.3.11 `src/linux/x11/window.rs` 无 `start_external_drag`，tTerm 现有 drag_out 也没有 Linux 后端。                                                                                                                                                     | 当前方案不可用；未运行验证。需要独立评估 XDS/桌面门户/文件管理器矩阵，再决定是否补丁。                                                                 |

这些“不可用”限定于现有适配与固定依赖，不表示 Linux 在技术上无法实现。P0-07 列出的 `42ba28e0b` 只是存在原生 X11 导出实现的参考，不能证明本仓库或任意 Linux 文件管理器已支持延迟下载。

## 后续共享位置与影响

P1 路径注入和平台 trait 完成后，平台 UI 线程入口放 `tterm-platform` 的 native 实现，调用端只传窗口身份、文件元信息和生命周期回调；内容下载仍由 core/SFTP 负责。Windows watcher 与 macOS promise 生命周期从 `src-tauri` 抽出为共享适配时，移动与修改分开提交。GPUI/Tauri 各自提供主线程调度，异步生成、复制、校验留后台，继续使用 `Result<T, String>`。此提议不授权现在移动生产代码、修改 A9 或扩大平台支持声明。

延迟下载以拖动触发为起点，文件内容不预下载。演示先生成到独立 staging，再在落点确认后流式发布，200 MB 不整份入内存。生产方案应优先复用下载进度、冲突、取消与续传机制，尽量避免二次磁盘复制；macOS 应在真正写入完成后兑现 promise。

## 风险与未决项

- 真实 Windows OLE 模态循环、GPUI 事件重入、鼠标捕获、取消后状态和资源管理器刷新尚未运行验证，必须按示例 README 补验。
- Windows 占位文件在落点短暂可见，通知可能迟到；示例不支持网络目录，取消会等待已启动的生成完成。未测峰值 RSS、拖放延迟、磁盘不足和进程中断。
- 若三平台延迟文件导出是必须项，Linux 需要进一步实现或接受功能缺口；本提议没有批准放弃 Linux 功能。后续确需 GPUI 补丁时只用 pins.md 的同版官方本地源码 `[patch.crates-io]` 退路，并独立审查许可和安全。
