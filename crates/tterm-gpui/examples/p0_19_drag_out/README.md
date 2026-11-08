# P0-19 文件拖出验证

这是独立 Windows 示例，不启动主程序、终端或 SSH，不加载配置或数据库。临时内容只在 `%TEMP%/com.stone.tTerm.gpui-dev/P0-19/drag-<pid>-<sequence>` 下生成。退出正常路径清理本次 staging；强杀或断电可能留下目录。macOS/Linux 仅有可行性评估，本例在它们上面不实现原生拖出。

在仓库根执行：

```powershell
cargo build --workspace --example p0_19_drag_out
cargo test --workspace --example p0_19_drag_out -- --nocapture
target/debug/examples/p0_19_drag_out.exe --local-check
```

`--local-check` 不打开窗口，不证明原生拖放。它在创建 staging 后先检查内容不存在，再显式触发生成、复制、逐字节验证及同名冲突保护。测试还验证末尾损坏检测、清理、非法目标拒绝、占位文件被修改时不删除，以及真实 Shell `IDataObject` 提供 `CF_HDROP`。

文件是十进制 200 MB（200,000,000 字节），不是 200 MiB。生成循环使用 64 KiB 缓冲，每块字节为 `index % 251`，实际写入并 sync，非稀疏文件；目标复制和完整验证也使用固定内存。运行时间是本机磁盘实验，不是网络下载速度、峰值内存或拖放响应延迟。staging 加目标最多占约 400 MB。

## Windows 实拖 `[人工]`

开发接口 `crates/tterm-gpui/AUTOMATION.md` 绑定主程序的 `terminal_demo::window::Prototype`，其 Root/实体校验不接受本例；本卡不改主程序或绕过校验。没有使用系统级自动输入，也没有窗口截图或录屏。下面由评估者真人执行；若之后适配开发接口，自动操作每次输入前必须确认前台为本例，且目标只能是自己新建的 TEMP 空目录窗口。

```powershell
$probeTarget = Join-Path $env:TEMP ('tTerm-P0-19-target-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $probeTarget
explorer.exe $probeTarget
target/debug/examples/p0_19_drag_out.exe --target $probeTarget
```

1. 将新开的空目录资源管理器窗口和示例并排放置。核对示例显示的完整目标路径与资源管理器地址栏相同。不要拖到桌面、已有目录或其他窗口。
2. 开始前确认提示 `No content generated before drag`，TEMP 中没有本次内容文件。按住蓝色文件区域移动超过 4 px，拖入上述空白文件区后松开。生成只在超过阈值时启动，与系统拖拽同时进行；不等待生成完成才开始原生拖拽。
3. 保存 stdout 与录屏。预期包含 `native_drag_started ... placeholder_bytes=0`、`generate_started bytes=200000000 buffer=65536`、`native_drag_ended accepted=true`，最终 `Published and verified bytes=200000000 ...`。后台执行器调度使日志打印顺序可能不同。目标最终只有 `generated-200MB.bin`，占位文件被删除，内容完整验证后才报告成功。
4. 记录 `(Get-Item (Join-Path $probeTarget 'generated-200MB.bin')).Length`，应为 `200000000`。观察生成期间与复制期间窗口、资源管理器是否响应，是否出现闪现占位文件、错误图标或需要手动刷新。它们尚未运行验证。
5. 重新新建 TEMP 空目录再启动，拖动后用 Escape 取消。应无目标内容，生成的 staging 在生成结束后丢弃。取消目前不打断正在进行的 200 MB 生成；记录这段延迟。再验证落回本窗口、释放在无效区域、重复拖动；不能误发布到上一次目标。
6. 测完关闭示例与本次资源管理器窗口，仅删除自己创建的目标目录。不要清理其他 TEMP 内容。

## 方案边界

Windows 复用 `src-tauri/src/drag_out/windows.rs` 的 `SHParseDisplayName` → `SHCreateShellItemArrayFromIDLists` → `BHID_DataObject` → `SHDoDragDrop`。owner HWND 来自 `GetForegroundWindow`，发起前同时校验 GPUI active 状态、当前 PID 与固定标题，不需要新增 raw-window-handle 依赖或 GPUI 补丁。GPUI 自身已在主线程 `OleInitialize`。

与生产版不同，演示只轮询预先指定目标中的唯一空占位文件，最多等 10 秒，然后生成内容复制到目标。它不证明全局 Shell 变更通知、网络目录或任意落点可用。错误目标可能遗留一个空占位文件；请严格遵守目标限制。复制不是原子发布，失败有尽力清理，进程崩溃仍可能留下部分文件。未实现冲突弹窗、下载取消/续传和失败重试。

macOS 可复用 `NSFilePromiseProvider`，由 GPUI 的 AppKit NSView 发起 `beginDraggingSessionWithItems`；生产实现的 delegate 提前报告完成，后续正式共享适配应保留 completion，等异步下载成功再完成 promise。Linux 的 X11 缺少 external drag 实现；固定版 Wayland 只发 `text/uri-list`，不能承诺延迟文件。应先研究 XDS/门户与目标文件管理器矩阵；若必须补丁，按 pins.md 的本地同版源码 patch 退路，禁止个人 fork/git 依赖。源码位置和结论见决策 011 与报告，均未运行验证。
