# GPUI 补丁差距评估

> 日期：2026-11-05。对应任务卡 [P0-07](p0-validation.md#p0-07-gpui-补丁差距评估)，执行报告见 [reports/P0-07.md](reports/P0-07.md)。
> 本文只做评估，没有打任何补丁、没有改依赖。真要打补丁时的落地方式见第 6 节。

## 1. 结论摘要

| 项                                     | 来源提交                                           | 改动规模               | 官方是否已有                                      | 结论                                              |
| -------------------------------------- | -------------------------------------------------- | ---------------------- | ------------------------------------------------- | ------------------------------------------------- |
| Windows 输入法只更新光标的组合事件     | `4cdc74a06a`                                       | 1 个代码文件，+139/−38 | 没有，代码自快照以来未变                          | 需要时按 P0-06 的退路打补丁，由 P0-18 判定        |
| 图集分配失败处理                       | `eba94834a6`、`db83fea0cb`（只有 wgpu 部分）       | 1 个代码文件，+102/−29 | 部分：DirectX 与 Metal 已返回错误，只有 wgpu 会崩 | 需要时按 P0-06 的退路打补丁，由 P0-17、P0-18 判定 |
| 关闭对话框时恢复所属窗口               | `027804e15a`、`9c715bdbd7`                         | 4 个代码文件，+381/−26 | 没有                                              | 不需要，条件变化后重新评估                        |
| 隐藏光标                               | `d24cecec8a`                                       | 8 个文件，+72/−1       | 没有；但输入时隐藏指针由 GPUI 自带功能覆盖        | 不需要                                            |
| macOS 光标状态                         | `3f2557eeb9`                                       | 1 个文件，+6/−4        | 不适用，问题由上一项引入                          | 不需要                                            |
| 文件拖出（P0-19）                      | 6 个提交                                           | 约 +3,124/−111         | 部分：只有已存在的文件，且只有 macOS 与 Wayland   | 需要时打补丁，先验证 P0-19 复用 tTerm 自己的方案  |
| 原生子窗口与浮层（nyaterm 的 RDP/VNC） | 10 个提交                                          | 约 +1,810/−92          | 官方一行 `bc6fc11740` 解决相邻问题                | 不需要                                            |
| 内部拖拽的安全与取消                   | `a02177258`、`099133e58`、`33980b598`、`6f2aeb588` | 约 +849/−19            | 没有                                              | 需要时按 P0-06 的退路打补丁，由 P3 的拖动卡判定   |
| 可变图集与动态纹理（远程桌面帧缓冲）   | `77282a5c2f`                                       | 7 个文件，+540/−7      | 没有                                              | 不需要                                            |

「需要时打补丁」的项都附了触发它的测试场景，见 3.1、3.2、4.1、4.3。

## 2. 对比基线

| 项                 | 值                                                                                                                    | 核对方式                                                                  |
| ------------------ | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| 个人分支           | `github.com/nyakang/zed`，分支 `nyaterm`，顶端 `6cda7bc3f0`（2026-11-05 13:12 +0800）                                 | `git log -1 origin/nyaterm`                                               |
| 个人分支的官方基线 | `f4d40a2ccb`（2026-11-02 16:48 UTC，`git_ui: Drag files from the git panel to the terminal (#66423)`），即 merge-base | `git merge-base` 官方 `main` 与 `origin/nyaterm`                          |
| 个人分支多出的提交 | 40 个（无合并提交），45 个文件，+7,734/−295                                                                           | `git rev-list --count f4d40a2ccb..origin/nyaterm`、`git diff --shortstat` |
| 官方仓库           | `github.com/zed-industries/zed`，`main` 顶端 `dcf3f98c9a`（2026-11-05 10:31 UTC）                                     | `git log -1 main`                                                         |
| 快照点             | `zed@f816e1f`（2026-11-01 22:52 UTC），`gpui-pre 0.3.11` 的代码来源                                                   | `git log -1 zed@f816e1f`                                                  |
| 快照之后           | 51 个提交，其中 8 个改到 `crates/gpui*`                                                                               | `git rev-list --count zed@f816e1f..main`                                  |

个人分支改到的 45 个文件里，`crates/gpui` 14 个、`crates/gpui_windows` 12 个、`crates/gpui_linux` 7 个、`crates/gpui_macos` 3 个、`crates/gpui_apple` 1 个、`crates/gpui_wgpu` 1 个、`crates/gpui_web` 1 个；其余是 `crates/scheduler` 的两个测试文件、`Cargo.toml`、`NYATERM.md`、`README.md` 与 `.github/workflows/nyaterm.yml`。**没有任何提交改到 `crates/terminal`、`crates/terminal_view`、`crates/editor`、`crates/workspace`、`crates/ui` 等 GPL crate**，因此本卡没有触发「该提交涉及终端 crate，未阅读」的记录。

核对清单：

```sh
git clone --filter=blob:none --no-checkout https://github.com/zed-industries/zed.git zed-official
git clone --filter=blob:none --no-checkout https://github.com/nyakang/zed.git zed-nyakang
# 两个仓库都只用来读提交与文件，不放进 tTerm
```

净室范围：本卡只打开 `crates/gpui`、`crates/gpui_*` 下的文件，以及个人分支自己的 `NYATERM.md`（分支作者写的说明，不是 Zed 的 GPL 代码）。`crates/scheduler` 的改动没有阅读，只从提交信息判断去向（见 4.5）。

## 3. 五项必评

### 3.1 Windows 输入法只更新光标的组合事件

- **来源提交**：`4cdc74a06a`（2026-10-29），`fix(gpui_windows): forward IME caret-only composition updates`，改 `crates/gpui_windows/src/events.rs` +139/−38、`NYATERM.md` +15。
- **作用**：Windows 的 `WM_IME_COMPOSITION` 有时只带 `GCS_CURSORPOS`（预编辑内移动光标），不带 `GCS_COMPSTR`（文本变化）。0.3.11 的 `handle_ime_composition_inner` 只在 `GCS_COMPSTR` 置位时才把预编辑文本交给 `replace_and_mark_text_in_range`；只有 `GCS_CURSORPOS` 时落到最后一行 `None`，交给 `DefWindowProc`，预编辑的选区不会更新。补丁把它拆成 `dispatch_ime_composition`：凡与组合相关的标志都读取当前组合串，只发一次「文本 + 选区」更新；保留原有的结果串顺序、`should_use_ime_cursor_position` 的光标兼容策略，以及标志为 0 时的取消行为。
- **tTerm 场景**：终端视图自己维护预编辑文本与光标（P0-13 的输入法部分），候选框位置也由它决定（P0-18）。Windows 中文输入法在预编辑里移动光标、翻页或改选后，预编辑光标与候选框可能停在旧位置，与 Tauri 版（xterm.js 走浏览器 composition 事件）表现不同。
- **官方等价**：没有。`crates/gpui_windows/src/events.rs` 在 `zed@f816e1f` 与官方 `main`（`dcf3f98c9a`）上逐字节相同，区间内没有提交改过这个文件；`crates.io` 上 `gpui-pre-windows 0.3.11` 的同一文件也与快照逐字节相同。
- **改动规模**：1 个代码文件 +139/−38（其中约 90 行是 `ime_composition_tests` 单元测试），另加 15 行分支文档。
- **结论**：需要时按 P0-06 的退路打补丁。只 patch `gpui-pre-windows` 一个包。
- **触发场景**：P0-18 的 Windows 用例「用微软拼音或第三方中文输入法，在预编辑中用左右方向键移动光标、翻页改选」出现预编辑光标或候选框停在旧位置；或 P0-13 验收时预编辑光标位置与 Tauri 版不一致。

### 3.2 图集分配失败处理

- **来源提交**：`eba94834a6`（2026-10-17，`crates/gpui_wgpu/src/wgpu_atlas.rs` +73/−12）、`db83fea0cb`（2026-10-17，`wgpu_atlas.rs` +29/−17 可移植；`crates/gpui/src/window.rs` +65/−5 属于 nyaterm 自己的动态纹理 API，见 4.4）。
- **作用**：wgpu 后端在 0.3.11 里由 `push_texture` 直接 `create_texture`，并把 `&mut WgpuAtlasTexture` 交回上层。分配失败（显存不足、超出 `max_texture_dimension_2d` 等）走 wgpu 的未捕获错误路径，直接 panic。补丁在 `create_texture` 前后压入 `OutOfMemory`、`Validation`、`Internal` 三个 error scope，任一失败就返回 `None`，走 GPUI 原有的错误路径（跳过这一块绘制）；`push_texture` 改为返回 `Option`，上传失败时移除已分配的 tile，并去掉两处 `expect("texture must exist")`。
- **tTerm 场景**：Linux 用 wgpu/Vulkan（P0-17 的 llvmpipe 与远程桌面，P0-18 的设备与图集重建）。终端要画大量 CJK 与 emoji，图集扩容是常规路径；扩容失败在 0.3.11 上等于进程退出。
- **官方等价**：部分。0.3.11 的 DirectX 与 Metal 后端已经能把分配失败变成错误（`directx_atlas.rs` 的 `CreateTexture2D(...).ok()?`，`metal_atlas.rs` 的 `.context("allocating atlas texture")?`），只有 wgpu 会 panic；官方在快照之后没有任何提交触及 `crates/gpui_wgpu`。
- **改动规模**：需要移植的是 1 个文件、+102/−29（两个提交的 wgpu 部分）。`gpui-pre-wgpu 0.3.11` 的该文件与快照相差 2 行（wgpu 30 适配的注释与 `default_queue`），打补丁时要手工核对该处上下文。
- **结论**：需要时按 P0-06 的退路打补丁。只 patch `gpui-pre-wgpu` 一个包。
- **触发场景**：P0-17 或 P0-18 的 Linux 场景里出现 wgpu 的 `Validation` / `OutOfMemory` 未捕获错误并崩溃；或 P0-15 在显存受限的机器上做大量 emoji、CJK 输出时崩溃。

### 3.3 关闭对话框时恢复所属窗口

- **来源提交**：`027804e15a`（2026-10-13，`crates/gpui_windows/src/window.rs` +63/−10）、`9c715bdbd7`（2026-10-18，新增 `modal_owner.rs` +277，`events.rs` +16/−13，`window.rs` +24/−3，`gpui_windows.rs` +1）。另有 `6183922dc6`（+139/−30）只在同时打了 4.2 的原生浮层补丁时才需要。
- **作用**：`WindowKind::Dialog` 创建时用 `GetActiveWindow` 找到所属窗口并 `EnableWindow(false)` 做成模态。0.3.11 只在 `WM_DESTROY` 里恢复（窗口已经开始销毁之后），而且无条件 `SetForegroundWindow`。两个问题：(a) 创建过程中任一可失败步骤提前返回，所属窗口再也不会被恢复，主窗口永久点不动、也没有窗口可关；(b) 对话框从后台关闭时会把前台抢走。补丁用 `DisabledOwnerGuard` 在创建失败时恢复，用 `ModalOwner` 在隐藏前、`WM_CLOSE` 被接受时一次性恢复，`WM_DESTROY` 只做兜底，并在 `EnableWindow` 之后重新确认前台再决定是否激活。
- **tTerm 场景**：只有 tTerm 用 `WindowKind::Dialog` 打开原生模态窗口时才成立。当前计划里所有对话框都在窗口内（gpui-kit 的 dialog；`gpui-kit 0.9.0` 的源码里没有 `WindowKind` 的用法），所以不触发。
- **官方等价**：没有。`handle_destroy_msg` 在 `zed@f816e1f` 与 `main` 上逐字节相同，官方也没有 `modal_owner.rs`。
- **改动规模**：4 个代码文件，+381/−26。
- **结论**：不需要。
- **重新评估条件**：P3-06 或 P4 若改用 `WindowKind::Dialog` 做独立窗口（设置窗口、启动解锁窗口），必须至少移植 `027804e15a`，否则一次创建失败就会让主窗口永久不可用；再按体验需要补充 `9c715bdbd7`。

### 3.4 隐藏光标

- **来源提交**：`d24cecec8a`（2026-10-13），8 个文件 +72/−1。
- **作用**：给 `CursorStyle` 增加 `Hidden` 变体（对应 CSS 的 `cursor: none`），是元素级、按 hitbox 生效的隐藏。五个后端各自实现：Windows 返回空 `HCURSOR`，X11 复用常驻的隐藏光标，Wayland 调用 `set_cursor(serial, None, ...)`，macOS 用缓存的全透明 `NSCursor`（不用全局 hide 接口，因为那会被鼠标移动撤销），web 映射成 `none`。
- **tTerm 场景**：现有前端没有任何 `cursor: none`（`src/` 里只用到 pointer、not-allowed、grab、grabbing、default、text、move）。「打字时隐藏指针」由 GPUI 自带的 `CursorHideMode`（`App::set_cursor_hide_mode` 加 `hide_cursor_until_mouse_moves`）覆盖，不需要这个变体。
- **官方等价**：没有；`main` 的 `CursorStyle` 里没有 `Hidden`。
- **改动规模**：8 个文件（`gpui/src/platform.rs`、`gpui_linux` 4 个、`gpui_macos/src/window.rs`、`gpui_web/src/platform.rs`、`gpui_windows/src/util.rs`），+72/−1。
- **结论**：不需要。
- **重新评估条件**：出现「在某个元素上藏起系统指针」的需求（例如全屏播放、嵌入的原生子窗口），再取；取时必须连带 3.5。

### 3.5 macOS 光标状态

- **来源提交**：`3f2557eeb9`（2026-11-03），`crates/gpui_macos/src/window.rs` +6/−4。
- **作用**：`resetCursorRects` 里，打了 3.4 的版本会先 `let state = window_state.lock()`，然后访问 `state.cursor_style` 与 `state.hidden_cursor`，锁一直持有到随后的 AppKit 调用之后；AppKit 可能在同一个调用栈里回调 GPUI，形成死锁。补丁先把两个值拷出来、释放锁，再调 AppKit。
- **官方等价**：不适用。0.3.11 与 `main` 的写法是 `let cursor_style = window_state.lock().cursor_style;`，锁在语句结束就释放，本来就没有跨 AppKit 调用持锁的问题；这个 bug 是 3.4 引入的。
- **改动规模**：1 个文件 +6/−4。
- **结论**：不需要。只要不取 3.4 就没有这个 bug；取 3.4 时必须一起取。

## 4. 其他提交

### 4.1 文件拖出（P0-19 相关）

- **相关提交**（6 个，约 +3,124/−111；另 4 个内部拖拽修复见 4.3）：
  - `0d48832b2`（+893/−7）：`ExternalDragPayload` 增加 `VirtualFiles`、`VirtualFileTree`、`PromisedFiles` 三个变体，新增 `virtual_file.rs`（436 行）与 `file_promise.rs`（72 行），`Window::supports_file_promise_drag` / `supports_virtual_file_drag`，`AtlasState::tile` 与 `HeadlessAtlas::update` 的测试支撑，以及指针手势的 `set_internal_drag_active`。
  - `f12c1c296`（+241/−5）：macOS 的 `NSFilePromiseProvider` 实现（`gpui_macos/src/file_promise.rs`，205 行）。
  - `8f38a6704`（+1,599/−16）：Windows 的 OLE 导出，`external_drag.rs` 968 行加 519 行测试，`IDataObject`/`IDropSource` 与「文件组描述符 + 流」。
  - `42ba28e0b`（+382/−72）：X11 的 `file_drag.rs`（292 行）与窗口接线，Wayland 一侧改为共用实现。
  - `256c1acfe`（+6/−8）与 `d6bec869f`（+3/−3）：适配上游平台变化与导出顺序的格式调整。
- **官方现状**：0.3.11 已有 `ExternalDragPayload::Files(FileDragPaths)` 与 `PlatformWindow::start_external_drag`，但只有 macOS 与 Wayland 实现，Windows 与 X11 没有；而且只能拖已经存在于磁盘上的文件，不能延迟生成内容。官方 `main` 到 2026-11-05 仍然如此（`gpui_windows` 与 `gpui_linux/x11` 的 `start_external_drag` 计数为 0）。
- **tTerm 现状**：`src-tauri/src/drag_out/` 只找出「拖到了哪个目录」（macOS 的 NSView、Windows 的原生实现），随后前端走既有的下载管线，带冲突提示、进度与续传；只构建 Windows 与 macOS，Linux 没有实现。
- **结论**：需要时打补丁，但先按 P0-19 验证复用现有 `drag_out`。
- **触发场景**（任一成立才打）：
  1. P0-19 的最小演示无法从 GPUI 窗口取得发起原生拖拽所需的原生窗口句柄或视图；
  2. Linux 必须支持拖到文件管理器，而官方 Wayland 的「只能拖已存在的文件」不够用；
  3. 需要在拖放时由 GPUI 直接把内容交给系统（延迟生成、按需流式写入），而不是像现在这样先拿到落点目录、再由前端走下载管线。
- **注意**：这是全部补丁里最大的一块，约 4,000 行，含 Windows OLE 与 X11 DnD 的原生代码；P0-19 的最小演示通过前不要打。

### 4.2 原生子窗口与浮层

- **相关提交**（10 个，约 +1,810/−92）：`44e8efbc9`、`2beca523f`、`90a643395`、`ee2c63d67`、`4daf8693bb`、`96581d000`、`0e47930a4`、`4117b6490`、`f038f381d`、`6183922dc`。为 nyaterm 在 GPUI 窗口里嵌入 RDP/VNC 的 ActiveX 子窗口服务：`WindowOptions::native_child_surfaces`（默认关闭）、区域化浮层窗口、DirectComposition 目标层级在上下层之间切换、原生子窗口的指针与光标归属。
- **官方相邻修复**：`bc6fc11740` 一行把 DirectComposition target 从 `topmost = true` 改为 `false`，让子 HWND 显示在 GPUI 内容之上。
- **tTerm**：不嵌入原生子窗口 → 不需要。

### 4.3 内部拖拽的安全与取消

- **`a02177258`**（+206/−13）：Windows 上内部拖拽按指针下的 HWND 读 `GWLP_USERDATA` 当 `Weak<WindowsWindowInner>`，对其他进程的窗口也会这么读（Electron 应用常占用该槽位），悬停到别的应用窗口上可能解引用外来地址。改为只认本进程注册过的窗口。属内存安全类修复。
- **`099133e58`**（+510/−6）：新增 `PointerCancelEvent` 与 `PointerCancelReason`，捕获丢失或 `WM_CANCELMODE` 时取消拖拽，不合成 mouse-up、不接受 drop。
- **`33980b598`**（+44）：排队中的取消之后到达的 mouse-up 不得被当成投放。
- **`6f2aeb588`**（+89）：Windows 上跨应用窗口的内部拖拽路由。
- **tTerm 场景**：P3 的标签与分屏拖动（现有 `usePaneDrag.ts`、`PaneHeader.tsx` 的对应物）。在 Windows 上把标签拖到别的应用窗口上悬停是常见操作。
- **结论**：需要时按 P0-06 的退路打补丁；`a02177258` 优先级最高。
- **触发场景**：P3-10、P3-12 的拖动用例在 Windows 上把指针悬停到 VS Code、浏览器等窗口时崩溃；或按 Esc、窗口失去捕获后仍然发生重排或误投放。

### 4.4 可变图集与动态纹理

- **`77282a5c2f`**（+540/−7）：`DynamicTexture`、`DynamicTextureId`、`AtlasKey::DynamicTexture`，`PlatformAtlas::update` 与各后端的 stride 上传，为 nyaterm 的远程桌面帧缓冲服务（每帧只更新脏区）。
- **tTerm**：不在对齐范围内（P0-29 只对图片协议给结论）→ 不需要。

### 4.5 文档、CI 与测试

- 文档与 CI：`7d5572c0c`、`f2f74268f`、`7ae7b9058`、`0c59dc545`、`791524aad`、`a755e55ac`、`6cda7bc3f`、`0afc306b6`、`30b6944e7`；测试与格式化：`0aefb4a41`、`d9164a075`。对 tTerm 没有直接影响。
- `b3dd73017e` 改 `crates/scheduler` 的测试调度器锁顺序——不在 GPUI 目录，按净室规则没有阅读内容，只从提交信息判断与 tTerm 无关。

## 5. 官方 `zed@f816e1f` 之后的提交（升级依据）

快照之后 51 个提交，只有 8 个改到 `crates/gpui*`：

| 提交         | 类别             | 改动                 | 内容                                                                                      | tTerm 是否需要                                   |
| ------------ | ---------------- | -------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------ |
| `decd2399b1` | 渲染             | +277/−38             | 复用的缓存视图与 deferred 绘制丢失实体订阅，实体通知后不重绘                              | 需要：终端 canvas 与延迟绘制会踩到，升级时优先取 |
| `c7bea34383` | 渲染 / 输入      | +104/−15             | 拖拽开始时指针样式短暂回退到旧样式                                                        | 需要：P3 拖动                                    |
| `ce6c5413f1` | 渲染             | +190/−9（gpui 部分） | `anchored` 弹层按空间选择展开方向                                                         | 需要：下拉与右键菜单                             |
| `0294cfb58e` | 渲染 / 文本      | +234/−59             | Windows DirectWrite 的制表符以前按零宽处理，制表位与光标索引丢失                          | 可能影响：P0-20 的代码编辑器显示 tab             |
| `d01f125837` | 输入（非输入法） | +15/−6               | macOS 原生菜单里 backspace 与 delete 的键等价字符搞反，菜单字形镜像                       | 低：只影响 macOS 菜单快捷键字形                  |
| `bc6fc11740` | 渲染             | +1/−1                | DirectComposition target 由 `topmost = true` 改 `false`，子 HWND 才能显示在 GPUI 内容之上 | 暂不需要；以后嵌 WebView 或原生子窗口时必须      |
| `635026fabc` | 渲染             | +85/−100             | `Image::to_image_data` 支持动图 WebP                                                      | 不需要                                           |
| `26abd1c959` | 渲染（基准设施） | +2,310/−304          | 扩展 gpui_platform 基准与两版本对比脚本                                                   | 不适用；P0-21 可参考其基准写法                   |

按任务卡的四类归并：

- **输入法**：0 个。区间内没有任何提交触及 `crates/gpui_windows/src/events.rs` 或 GPUI 的输入法接口。
- **Wayland**：0 个。区间内没有任何提交触及 `crates/gpui_linux`。
- **无障碍**：0 个。区间内没有提交改动 accesskit 或 GPUI 的可访问性代码。
- **渲染**：6 个（`decd2399b1`、`c7bea34383`、`ce6c5413f1`、`0294cfb58e`、`bc6fc11740`、`635026fabc`），外加基准设施 `26abd1c959`；`d01f125837` 归输入类。

版本现状（2026-11-05 19:0x +08:00 查询 crates.io API）：

| 包         | 最新版本 | 发布时间（UTC） | 比固定版本更新？ |
| ---------- | -------- | --------------- | ---------------- |
| `gpui-pre` | 0.3.11   | 2026-11-02      | 没有             |
| `gpui-kit` | 0.9.0    | 2026-11-02      | 没有             |

也就是说，**今天没有可以通过升级拿到上述任何修复的新版本**；要拿只能等 `gpui-pre` 与 `gpui-kit` 成对发布新版本（`pins.md` 要求两者成对升级），或者按第 6 节打补丁。版本查询命令：`curl -s -H 'User-Agent: ...' https://crates.io/api/v1/crates/gpui-pre`，`max_version` 分别返回 `0.3.11` 与 `0.9.0`。

## 6. 打补丁的落地方式

P0-06 已经实测过 `[patch.crates-io]` 的退路（见 [P0-06 报告](reports/P0-06.md#打补丁退路的实测步骤)）。本节只补两件本卡核对出来的事实。

**一、补丁落在哪个包上。** `gpui-pre` 是一个包族，每个平台 crate 单独发布，因此每一项都要 patch 对应的包：

| 项                   | 需要 patch 的包                                                                                          |
| -------------------- | -------------------------------------------------------------------------------------------------------- |
| 3.1 Windows 输入法   | `gpui-pre-windows`（`src/events.rs`）                                                                    |
| 3.2 图集分配失败     | `gpui-pre-wgpu`（`src/wgpu_atlas.rs`）                                                                   |
| 3.3 模态窗口所属窗口 | `gpui-pre-windows`（`src/window.rs`、`src/events.rs`、`src/gpui_windows.rs`，新增 `src/modal_owner.rs`） |
| 3.4 隐藏光标         | `gpui-pre`（`src/platform.rs`）与 `gpui-pre-linux`、`gpui-pre-macos`、`gpui-pre-windows`、`gpui-pre-web` |
| 3.5 macOS 光标状态   | `gpui-pre-macos`（`src/window.rs`）                                                                      |
| 4.1 文件拖出         | `gpui-pre`、`gpui-pre-windows`、`gpui-pre-macos`、`gpui-pre-linux`                                       |

**二、已发布源码与快照的差异。** 把 `gpui-pre`、`gpui-pre-windows`、`gpui-pre-macos`、`gpui-pre-linux`、`gpui-pre-apple` 的 0.3.11 源码与 `zed@f816e1f` 的同名文件逐字节对比：3.1、3.3、3.4、3.5 涉及的文件完全相同，补丁可原样套用；只有 `gpui-pre-wgpu/src/wgpu_atlas.rs` 差 2 行（wgpu 30 适配的注释与 `default_queue`），3.2 的 hunk 要手工调整一处上下文。

落地步骤（与 P0-06 相同）：把 `$CARGO_HOME/registry/src/*/gpui-pre-*` 复制到仓库外，按上表打 hunk，在根 `Cargo.toml` 用 `[patch.crates-io]` 逐包指向该目录，`cargo tree` 确认命中的是路径来源，跑测试后在 `pins.md` 第 8 条「打补丁记录」写明补丁文件、原因与对应的 P0-07 条目。补丁源码放在哪、如何登记许可与修改，由真正打补丁的那张卡按 [README 执行规则第 6 条](README.md#2-执行规则) 与 `pins.md` 决定。

## 7. 未确认

- 本卡没有在真机上验证任何一项行为：没有 Windows 与 Linux 设备，也没有运行 GPUI 程序。所有结论来自源码比对与已发布包的逐文件比对。
- 未查 GitHub 的 issue 与 PR 讨论，只按提交与文件判断「官方没有等价实现」；官方可能已在讨论但尚未合入。
- 未实际把补丁套到 `gpui-pre 0.3.11` 上编译（只做了逐文件内容比对），4.1 的 Windows OLE 与 X11 部分尤其未经编译验证。
- `crates/scheduler` 的改动没有阅读内容。
- 未确认 tTerm 以后是否会引入 `WindowKind::Dialog`；3.3 的结论以此为条件。
