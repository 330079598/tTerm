# P0-15-wheel 修正原型的滚轮方向与每格事件数

## 结论

完成待评估。按决策人选定的方案 A 修复共同入口的方向与三条路径的计数，回归测试及 AGENTS.md 全部检查通过。先前沙箱权限阻塞已通过正常提权重跑解决。Windows 真实输入按最新指令交给评估者，结果为“待评估者执行”；macOS、Linux 仅源码确认，未运行验证。

环境：调查于 2026-11-07，本轮续作于 2026-11-08；Windows 11，PowerShell 7；worktree `D:\code\rust\tTerm-P0-15-wheel`，分支 `migrate/P0-15-wheel-fix`，起点 `3c6665e17d5ce0241ed6ae8b3eae4e33d735d0a2`。未启动任何客户端，未读写 `com.stone.tTerm`、`com.stone.tTerm.dev` 或开发数据目录。后续原型运行必须使用 `com.stone.tTerm.gpui-dev`。

## 改动

- 已有调查提交：`60aabebf :memo: Document wheel semantics and required compatibility decision`。
- `52868b8 :bug: Preserve GPUI positive-up wheel direction across terminal routes`：只修正累积器符号并更新旧方向断言。
- `d6cea10 :bug: Match xterm wheel counts and fifty-pixel history notches`：实现每事件最多一次及历史像素换算，更新对应单元测试。
- `2cc32b3 :white_check_mark: Cover wheel routing counts and fractional trackpad input`：新增视图事件回归测试，覆盖三条路径、双向 Lines/Pixels、小数累积和换向清零。
- 本报告及证据单独提交；可用 `git log -1 --format='%h %s' -- docs/gpui-migration/reports/P0-15-wheel.md` 查询其哈希。
- `view/scroll.rs` 是唯一垂直符号入口，保留 GPUI 正值向上。历史 Lines 使用有文档注释的 `HISTORY_PIXELS_PER_LINE = 50.0 / 3.0` 换算像素；应用 Lines 非零事件立即转为一个步进；Pixels 继续按行高累积、换向清零。
- `view/wheel.rs` 复用现有 `wheel_action` 判断当前路由，再给累积器选择历史或应用计数策略；不增加各路由的方向补偿。移除应用上报后的余数重置，让触控板跨事件保留余数。
- `input/wheel.rs` 只将垂直应用步数限制为一次；横向计数及 Shift 的路由条件沿用原实现。补充 `view/wheel/tests.rs` 的视图事件测试，更新旧符号和计数断言。
- 原始测试与先前环境失败输出保存于 `assets/P0-15-wheel/*.txt`。没有修复后真实输入截图，旧截图仅作调查证据。
- 未修改 `status.md`、依赖、数据目录、横向滚轮、滚动边界、Shift 路由或其他 P0-15 遗留问题。

### GPUI 的符号与平台来源

本机 registry 根目录为 `C:\Users\stone\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f`。以下路径相对此目录，行号来自已安装的 crates.io 固定版本。

`gpui-kit-0.9.0/src/lib.rs:100` 将 `gpui` 原样再导出，没有重新定义滚轮事件。`gpui-pre-0.3.11/src/interactive.rs:575` 定义 `ScrollDelta::{Pixels, Lines}`，文档只说明单位，未直接规定正负方向；`:635` 的 `pixel_delta` 对 Lines 乘行高，对 Pixels 原样返回，不取负。符号约定需要结合各平台生产事件的代码确认。

平台统一采用内容位移的符号：垂直正值对应向上进入历史，负值对应向下返回新内容；这与 DOM `WheelEvent.deltaY` 的符号相反。macOS 自然滚动等系统设置已反映在平台给出的事件里，不应再按物理手势额外取反。

| 平台 | 固定版本源码位置 | 如何填充事件 | 验证范围 |
| --- | --- | --- | --- |
| Windows 普通滚轮 | `gpui-pre-windows-0.3.11/src/events.rs:552–599`；`src/system_settings.rs:72–85` | `WM_MOUSEWHEEL` 的有符号高字 / `WHEEL_DELTA` × `SPI_GETWHEELSCROLLLINES`，填入 `Lines.y`。Windows 正滚轮值表示远离用户；默认每格 3 行时结果为 +3。Shift 会转成横向，此卡不改。 | 源码确认，本轮未运行验证 |
| Windows 精确触控板 | `gpui-pre-windows-0.3.11/src/direct_manipulation.rs:319–334` | `dy = y_offset - last_y`，填入 `Pixels.y`；保留 Direct Manipulation 内容平移方向。 | 源码确认，本轮未运行验证 |
| macOS | `gpui-pre-macos-0.3.11/src/events.rs:260–289` | 读取 `NSEvent.scrollingDeltaY()`，精确设备填 `Pixels`，其他设备填 `Lines`，两种均不取负，保留系统滚动设置后的方向。 | 源码确认，未运行验证 |
| Linux X11 | `gpui-pre-linux-0.3.11/src/linux/x11/client.rs:1292–1300, 2523, 2554–2593`；`src/linux/platform.rs:49` | 离散 Up 为 `+SCROLL_LINES`，Down 为负，常量为 3；XI2 用 `(old_scroll - new_scroll) × multiplier`，其中 multiplier 为 `SCROLL_LINES / increment`。两条路径填 `Lines`。 | 源码确认，未运行验证 |
| Linux Wayland | `gpui-pre-linux-0.3.11/src/linux/wayland/client.rs:1125, 2625–2740`；`src/linux/platform.rs:49` | 垂直 modifier 初始为 -1。连续 Axis 的 value × 3 × modifier 填 `Pixels`；AxisDiscrete 的 discrete × modifier × 3、AxisValue120 的 value120 / 120 × modifier × 3 填 `Lines`。Frame 优先送出连续事件，否则离散事件。Wayland 原生向下为正，经 modifier 后向上为正。 | 源码确认，未运行验证 |

复核命令（在 PowerShell 中运行）：

```powershell
$registryRoot = 'C:\Users\stone\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f'
rg -n 'pub enum ScrollDelta|pub fn pixel_delta' "$registryRoot\gpui-pre-0.3.11\src\interactive.rs"
rg -n 'wheel_distance|wheel_scroll_lines|ScrollDelta::Lines' "$registryRoot\gpui-pre-windows-0.3.11\src\events.rs"
rg -n 'scrollingDeltaY|ScrollDelta::Pixels|ScrollDelta::Lines' "$registryRoot\gpui-pre-macos-0.3.11\src\events.rs"
rg -n 'ScrollDirection::Up|ScrollDirection::Down|old_scroll - new_scroll' "$registryRoot\gpui-pre-linux-0.3.11\src\linux\x11\client.rs"
rg -n 'vertical_modifier: -1|wheel_percent|axis_modifier \* SCROLL_LINES' "$registryRoot\gpui-pre-linux-0.3.11\src\linux\wayland\client.rs"
```

本次关键原始输出摘录：

```text
interactive.rs:575:pub enum ScrollDelta {
interactive.rs:635:    pub fn pixel_delta(&self, line_height: Pixels) -> Point<Pixels> {
events.rs:578:            (wparam.signed_hiword() as f32 / WHEEL_DELTA as f32) * wheel_scroll_amount as f32;
events.rs:271:                    native_event.scrollingDeltaY() as f32,
client.rs:1293:                                ScrollDirection::Up => Point::new(0.0, SCROLL_LINES),
client.rs:1294:                                ScrollDirection::Down => Point::new(0.0, -SCROLL_LINES),
client.rs:2562:            .map(|old_scroll| (old_scroll - new_scroll) * axis.multiplier);
client.rs:1125:            vertical_modifier: -1.0,
client.rs:2700:                let wheel_percent = value120 as f32 / 120.0;
```

### 当前根因与 Tauri 目标数字

修复前（`60aabebf`）的 `crates/tterm-terminal-view/src/view/scroll.rs:24–25` 对两种 delta 都取负。因此 Windows 向外一格得到 `Lines.y = +3` 后被转成 -3，历史滚动、备用屏幕方向键、鼠标上报全部反向。唯一需要修正符号的入口是这个累积器；无需在三条路由分别补偿。

一格三次上报不是 Lines 转像素又除行高引入的倍数：这两步相互抵消。Windows GPUI 已乘系统行数；`input/wheel.rs:83–87, 120–122` 再按 `abs(lines)` 生成按钮，`view/wheel.rs:43–50` 逐个发送，所以在系统值为 3 时恰好产生三次。备用屏幕的 `repeat_arrow` 同样按行数重复。修复前 view 没有额外再乘 3。本轮依决策人指示修复这些行为。

Tauri 目标从仓库锁定的 `@xterm/xterm` **6.0.0** 源码确认，读取位置为主 checkout 的 `D:\code\rust\tTerm\node_modules\@xterm\xterm\src`；本轮在本 worktree 的 `node_modules/@xterm/xterm/src` 复核同一版本。`src/components/TerminalTab` 未覆写 `scrollSensitivity`；xterm `common/services/OptionsService.ts:37,39` 的默认值为 1 和 0（平滑时长）。

| 路径 | xterm.js 6.0.0 的源码行为及数字 | 证据 |
| --- | --- | --- |
| 鼠标上报 | `consumeWheelEvent` 非零后，单次调用 `triggerMouseEvent`；一个有效滚轮事件仅发送一个上报，不按返回行数循环。向外为 64、向内为 65。 | `browser/CoreBrowserTerminal.ts:641–674`；`common/services/CoreMouseService.ts:241–269`；P0-15 已有真实输入截图 `assets/P0-15/manual/tauri-mouse-sgr-drag.png` |
| 备用屏幕 | 同样只用消费结果判断是否非零，然后只发一条方向序列。DECCKM 开时向外为 `ESC O A`，向内为 `ESC O B`；DECCKM 关时 xterm 发 CSI。 | `browser/CoreBrowserTerminal.ts:814–840`；本卡不顺带修改 SS3/CSI 模式策略 |
| 本地历史 | 常规 Windows Chromium/WebView 滚轮 `wheelDeltaY = ±120` 在 StandardWheelEvent 归一化为 ±1，滚动器乘 50，即每格 50 CSS px，再按单元格高度用 `Math.round` 计算显示行。不是固定每格 3 行，也不能把两格 5 行简单当成所有字体下每格 2.5 行。 | `vs/base/browser/mouseEvent.ts:155–160`；`vs/base/browser/ui/scrollbar/scrollableElement.ts:23,407,446`；`browser/Viewport.ts:128,184` |

P0-15 既有截图已逐张查看：GPUI start 首行 160，朝用户一格后 157，再向外两格回到 160；Tauri start 首行 163，朝用户一格不动，再向外两格为 158。与其报告一致。鼠标图中 GPUI 每方向三条，Tauri 每方向一条。它们是修复前的历史证据，本轮未重跑，不挪作修复后的截图。

触控板还有另一处计数差异：xterm 的 `consumeWheelEvent` 对像素 delta 除单元格 CSS 高，`abs(deltaY) < 50` 时再乘 0.3，保留小数后供路由判断；本卡现有累积器直接除行高并在换向时清余数。任务要求保留小数累积和换向清零，因此不能未经决策照搬整套 xterm 输入处理。

### 与 Alacritty 的冲突及上一轮备选方案

读取官方固定提交 `94e7c8874e526b1e67b349d9ba30ddf81669119e` 的 [alacritty/src/input/mod.rs](https://github.com/alacritty/alacritty/blob/94e7c8874e526b1e67b349d9ba30ddf81669119e/alacritty/src/input/mod.rs#L725) 与 [alacritty/src/config/scrolling.rs](https://github.com/alacritty/alacritty/blob/94e7c8874e526b1e67b349d9ba30ddf81669119e/alacritty/src/config/scrolling.rs#L17)。用 `Invoke-WebRequest` 读取 raw.githubusercontent.com 上这两个固定版本文件，按行筛选的原始输出如下：

```text
725:     pub fn mouse_wheel_input(&mut self, delta: MouseScrollDelta, phase: TouchPhase) {
760:     fn scroll_terminal(&mut self, new_scroll_x_px: f64, new_scroll_y_px: f64, multiplier: f64) {
769:         let multiplier = if self.ctx.mouse_mode() { 1. } else { multiplier };
772:         self.ctx.mouse_mut().accumulated_scroll.y += new_scroll_y_px * multiplier;
774:         let lines = (self.ctx.mouse().accumulated_scroll.y / height).abs() as usize;
787:             for _ in 0..lines {
808:             for _ in 0..lines {
822:             let lines = if is_scroll_up { lines as i32 } else { -(lines as i32) };
```

`scrolling.rs:19` 默认 multiplier 为 3。对于一格输入 `LineDelta(0, 1)`，Alacritty 的鼠标模式不乘倍数，得到一个上报；备用屏幕和本地历史乘 3，分别得到三条 SS3 方向序列和三行滚动。这里的 3 来自 Alacritty 配置，与本原型 GPUI 已乘系统行数的 3 是不同层。

上一轮因此触发停止条件，提交 `60aabebf` 列出下表。2026-11-08 决策人已选 A 并认可与 Alacritty 的差异；本轮严格按补充参数实现。下表保留为决策背景，不再等待选择：

| 方案 | 具体行为 | 好处 | 代价 / 需要明确的事项 |
| --- | --- | --- | --- |
| A：按本卡目标对齐 Tauri | 修正共同符号入口；有效离散输入在鼠标/备用屏幕路径各发一次；本地历史按 xterm 的像素目标计算。保留本卡要求的触控板累积和换向清零。 | 符合本卡验收及用户在现有 Tauri 版中的习惯。 | 明确偏离 Alacritty 的备用屏幕倍数和历史行数；需要认可这项例外，并明确采用 xterm 的 50 CSS px 历史目标。 |
| B：保持 Alacritty 滚动计数 | 先正确归一化 GPUI 的系统行数，再采用 Alacritty 的鼠标倍率 1、其他路径默认倍率 3。 | 沿用已有路由的上游计数语义。 | 备用屏幕每格三条与本卡“每格事件数与 Tauri 版相同”冲突，必须先修改任务卡；历史也不等同 Tauri。 |
| C：协议次数对齐 Tauri，历史沿用 GPUI 系统行数 | 鼠标和备用屏幕各一次；本地历史保留系统每格行数，只统一修正方向。 | 改动较小，保留系统历史滚动速度。 | 既偏离 Alacritty 的备用屏幕行为，也不能满足历史速度与 Tauri 对照一致，必须明确缩小本卡的对齐范围。 |

无论采用哪种方案，都不能对所有 Lines 直接除以常量 3：Windows 系统行数可变，macOS 的 Lines 也不承诺每格为 3。GPUI 事件只保留归一化后的行数，不携带原始 notch 数。对于快速滚动合并和高分辨率滚轮，本轮已明确：每个有效 Lines 事件最多发一次，不推断原始 notch 数。

未直接阅读 Zed 仓库的任何提交或 `terminal`、`terminal_view` 文件。本次阅读的 GPUI crates.io 源码标注快照为 `zed@f816e1f`，文件如上表。没有复制 Zed 的代码、注释、测试或私有标识符；本次也未向生产代码复制任何上游实现。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 报告写明 GPUI 0.3.11 的 ScrollDelta 符号约定及三个平台的源码位置。 | 通过（源码调查） | 上述平台表、复核命令与输出；本轮重新阅读 GPUI 定义及 Windows/macOS/X11/Wayland 生产事件的位置。macOS、Linux 均为“源码确认，未运行验证”。 |
| 向远离用户的方向滚一格：历史区向上进入历史；鼠标上报为一个 wheel-up（SGR 按钮 64）；备用屏幕发送 ESC O A。朝用户方向相反。每格事件数与 Tauri 版相同。 | 自动化通过；[人工] 待评估者执行 | `cargo test --workspace wheel`：13 passed，0 failed；`assets/P0-15-wheel/wheel-tests.txt`。下表列出三条路径的测试名及断言。目标数字来自上文 xterm.js 6.0.0 源码；真实设备未重跑。 |
| 方向与每格事件数有自动化测试，测试名列在报告中；把修正撤销后这些测试会失败（报告附失败输出，临时改动不提交）。 | 通过 | `cargo test --workspace` 包含下表测试；恢复旧符号后 5 个视图测试失败，恢复旧计数后 7 个测试失败，退出码均 101；完整输出见 `reverted-direction.txt`、`reverted-count.txt`。临时改动已用 try/finally 恢复，随后 Rust 全套检查通过。 |
| Windows 上的真实滚轮结果有截图，或标为 [人工] 并写好步骤；macOS、Linux 为“源码确认，未运行验证”。 | [人工] 待评估者执行 | 按最新指令，真实滚轮验证由评估者执行。没有发送真实输入，没有生成新截图；保留步骤见“遗留问题”。macOS、Linux 为“源码确认，未运行验证”。 |

自动化测试均为 GPUI headless 事件/纯逻辑验证，不能代替 Windows 窗口及 ConPTY 的真实输入验证。命令从本 worktree 根执行。

| 测试名（位于 `tterm-terminal-view`） | 固定的行为 |
| --- | --- |
| `view::wheel::tests::line_notches_report_one_sgr_button_in_each_direction` | Lines.y = +3 / -3，恰好一个 `ESC[<64;1;1M` / `ESC[<65;1;1M`。 |
| `view::wheel::tests::line_notches_send_one_alternate_arrow_in_each_direction` | Lines.y = +3 / -3，恰好一个 `ESC O A` / `ESC O B`。 |
| `view::wheel::tests::line_notches_scroll_history_by_fifty_pixels` | 每格 50 px 按实际行高转换，连续两格保留余数；反向后减少历史偏移，无协议字节。 |
| `view::scroll::tests::line_notches_scroll_fifty_pixels_and_keep_the_remainder` | Lines.y = ±3，行高 20 px：第一格 ±2 行、余数 ±10 px；第二格 ±3 行、余数 0，两格共 ±5 行。 |
| `view::wheel::tests::pixel_events_scroll_history_in_both_directions` | Pixels 正值进入历史，负值返回新内容；3 个行高上移三行，反向一个行高下移一行，无协议字节。 |
| `view::wheel::tests::pixel_application_events_accumulate_reverse_and_emit_at_most_once` | 鼠标和备用屏幕均覆盖：小数累积满一行才发送；已发送后余数保留；3.5 行的大事件只发一次；换向清零，负值分别发送 65 / ESC O B。 |
| `view::scroll::tests::pixel_fractions_accumulate` | 三次 6.75 px、行高 20 px，输出 0/0/1，余数 0.25 px。 |
| `view::scroll::tests::reversing_drops_the_remainder` | +8.5 后 -8.5 px，不产生整行，余数仅为负方向；reset 清空。 |
| `view::scroll::tests::negative_remainders_round_toward_zero` | -30 px、行高 20，得到 -1 行、余数 -10 px。 |

撤销验证可复现方式（临时修改，不保留在工作树）：

1. 只恢复旧方向：在 `scroll.rs` 的共同像素换算之后、有限值检查之前临时加入 `let pixels = -pixels;`；运行 `cargo test --workspace view::wheel::tests`，恢复文件。
2. 只恢复旧计数：历史 Lines 的 `HISTORY_PIXELS_PER_LINE` 临时改为 `cell_height`，跳过应用 Lines 的单步分支，将 `input/wheel.rs` 中两处 `wheel.lines.signum()` 临时改回 `wheel.lines`；运行 `cargo test --workspace wheel`，恢复两个文件。
3. 恢复后执行下述完整 Rust 检查，全部退出 0。以下是失败原始输出摘录，完整输出在同目录 txt 中：

```text
# cargo test --workspace view::wheel::tests（仅恢复旧方向）
---- view::wheel::tests::line_notches_send_one_alternate_arrow_in_each_direction stdout ----
assertion `left == right` failed
  left: [27, 79, 66]
 right: [27, 79, 65]
---- view::wheel::tests::line_notches_report_one_sgr_button_in_each_direction stdout ----
assertion `left == right` failed
  left: [27, 91, 60, 54, 53, 59, 49, 59, 49, 77]
 right: [27, 91, 60, 54, 52, 59, 49, 59, 49, 77]
test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 129 filtered out; finished in 0.10s
exit=101

# cargo test --workspace wheel（仅恢复旧计数）
---- view::wheel::tests::line_notches_report_one_sgr_button_in_each_direction stdout ----
assertion `left == right` failed
  left: [27, 91, 60, 54, 52, 59, 49, 59, 49, 77, 27, 91, 60, 54, 52, 59, 49, 59, 49, 77, 27, 91, 60, 54, 52, 59, 49, 59, 49, 77]
 right: [27, 91, 60, 54, 52, 59, 49, 59, 49, 77]
---- view::wheel::tests::line_notches_send_one_alternate_arrow_in_each_direction stdout ----
assertion `left == right` failed
  left: [27, 79, 65, 27, 79, 65, 27, 79, 65]
 right: [27, 79, 65]
test result: FAILED. 6 passed; 7 failed; 0 ignored; 0 measured; 121 filtered out; finished in 0.05s
exit=101
```

## 检查

AGENTS.md 要求的全部检查已在 2026-11-08 重跑通过。PowerShell 从本 worktree 根按以下命令顺序执行，每步非零即停止；原始输出写入下表文件。

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

前端检查在沙箱内再次复现 `pnpm test` 的 `spawn EPERM` 后，按本轮授权申请提权，完整链在沙箱外正常通过；没有修改测试配置、检查脚本、依赖或基线。Git 暂存和提交也走正常提权流程，没有临时 index、plumbing、`--no-verify` 或权限绕过。

| 检查 | 实际结果 | 原始输出路径（相对 `assets/P0-15-wheel/`） |
| --- | --- | --- |
| `pnpm lint` | 退出 0；`$ eslint src` | `frontend-checks.txt` |
| `pnpm format:check` | 退出 0；`All matched files use Prettier code style!` | `frontend-checks.txt` |
| `pnpm check:tokens` | 退出 0；`$ node scripts/tokens/generate.mjs --check` | `frontend-checks.txt` |
| `pnpm typecheck` | 退出 0；`$ tsc --noEmit` | `frontend-checks.txt` |
| `pnpm test` | 退出 0；`Test Files 174 passed (174)`；`Tests 1294 passed (1294)` | `frontend-tests.txt` |
| `pnpm check:size` | 退出 0；`file-size: 1015 files within their limits` | `size-check.txt` |
| `cargo fmt --all --check` | 退出 0，无输出 | `rust-fmt.txt` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 退出 0；`Finished dev profile ... in 1.40s` | `clippy.txt` |
| `cargo test --workspace` | 退出 0；合计 1092 passed，23 ignored，0 failed；terminal-view 134 passed | `rust-tests.txt` |
| `git diff --check` | 退出 0，无输出 | 本轮执行记录 |
先前环境错误原始输出摘录（已解决，不是当前检查结果；历史文件 `frontend-test-blocked.txt`、`size-check-blocked.txt`、`git-blocked.txt` 保留供核对）：

```text
$ vitest run src
failed to load config from D:\code\rust\tTerm-P0-15-wheel\vite.config.ts
[plugin externalize-deps]
Error: spawn EPERM
    at optimizeSafeRealPathSync (.../vite/dist/node/chunks/node.js:2458:2)

$ node scripts/check-file-size.mjs
Error: spawnSync git EPERM
    at sourceFiles (.../scripts/check-file-size.mjs:26:18)
Node.js v24.19.0

fatal: Unable to create 'D:/code/rust/tTerm/.git/worktrees/tTerm-P0-15-wheel/index.lock': Permission denied
```

Rust 初次构建被 `scripts/prepare-conpty.ps1` 的 NuGet 下载 TLS 错误阻断。只读检查主 checkout 中现有固定缓存，验证 SHA-256 为 `02b07b349af66d801159bdf9e440d4a1ce78bb951f37fc8609731665afdae7ee`，与脚本一致后，将该 zip 复制到本 worktree 的 `src-tauri/resources/conpty/`（构建缓存，不提交）。随后 Rust 构建与检查成功；未写主 checkout 文件、未改分支、未升级依赖。

## 偏差

- 上一轮要求停止的 Alacritty/xterm 取舍已经由本轮明确指令解决，不再有待选方案。
- 先前前端 test/check:size 的 EPERM 已通过本轮获批的正常提权重跑解决，完整检查通过；未静音检查，未更改基线或测试配置。
- 先前 Git `index.lock` 权限问题已通过本轮获批的正常 git add / git commit 提权解决，方向、计数和视图测试分别提交。未修改其他 checkout、未用临时 index 或 git plumbing。
- Windows 真实滚轮按最新指令交给评估者，结果为“待评估者执行”。保留 [人工] 步骤，没有生成修复后截图。本轮没有启动客户端，也没有通过 UI 输入 shell 命令。
- 文件路径与卡相符。没有修改 `status.md`，没有扩大到横向滚轮、边界、Shift 路由或其他遗留问题。

## 遗留问题

已知差异已获决策人认可：

- Pixels 的应用输入不采用 xterm 小于 50 px 时的 0.3 倍率，保留直接按行高累积与换向清零；留 P2-04 / P2-06 在 macOS 上复核。
- Windows 系统每格不是 3 行时，历史距离按 `系统行数 × 50/3 px` 比例变化；xterm 的 50 CSS px 不读取系统行数。
- 历史整数行在本原型用向零截断并保留余数；xterm Viewport 按 `Math.round` 映射可见行。因此某一格的可见行偏移可能不同，像素目标及连续两格累计按本轮明确参数执行。没有顺带改边界策略。
- 与 Alacritty 的备用屏幕倍数、历史策略不同，按决策人选择的方案 A 执行。

[人工] Windows 验证步骤（结果：待评估者执行）：

1. 在本 worktree 根运行 `cargo build --workspace`，启动这里生成的 `target/debug/tterm-gpui.exe`。独立数据目录只可使用 `com.stone.tTerm.gpui-dev`；当前原型不接入持久化服务，不得用正式/Tauri 开发版配置启动。记录字体、单元格行高、缩放、Windows 每格滚动行数。每次真实输入前，核对进程完整路径、窗口标题 `tTerm GPUI Dev`，并确认前台窗口就是此进程的原型 HWND；不是就停止。每格只发送一次 ±120 原生滚轮量，不把 API 的像素滚动量冒充一格。
2. 用原 P0-15 命令启动：`target/debug/tterm-gpui.exe --exec wsl.exe -d Ubuntu -- bash -c "seq 1 200; sleep 120"`。保存 `history-start.png`；向远离用户方向一格，保存 `history-away.png`，应进入历史；向朝用户方向一格，保存 `history-toward.png`，应返回新内容。记录首行与实际偏移，再向外连续两格核对累计 100 px 的行数；余数/边界截断不能作为速度改变的证据。
3. 启动 `target/debug/tterm-gpui.exe --exec wsl.exe -d Ubuntu -- python3 /mnt/d/code/rust/tTerm-P0-15-wheel/scripts/terminal-probe.py --mouse 1002,1006`。指针停同一格，分别向外/向内一格，每次输入前重新确认前台 HWND；核对恰好一条 `<64;…M` / `<65;…M`，保存 RX 原始字节及 `mouse-away.png` / `mouse-toward.png`。
4. 启动 `target/debug/tterm-gpui.exe --exec wsl.exe -d Ubuntu -- bash -c "printf '\033[?1049h\033[?1007h'; python3 /mnt/d/code/rust/tTerm-P0-15-wheel/scripts/terminal-probe.py --application-cursor; printf '\033[?1049l'"`。向外/向内各一格，每次重新确认前台 HWND；核对分别一条 `1b 4f 41` / `1b 4f 42`，保存 `alternate-away.png` / `alternate-toward.png`。Ctrl+C 结束探针，后续 printf 恢复主屏。
5. 所有新截图及 RX 日志写入 `docs/gpui-migration/reports/assets/P0-15-wheel/`，补上实际结果和环境参数。该目录现有 txt 为自动化证据，不是实际输入记录。macOS、Linux 保持“源码确认，未运行验证”。

自动化检查及正常提交已完成；真实 Windows 输入与截图待评估者执行。P0-12 的边界、小数滚动限制和 P0-13/P0-28 其他按键、鼠标、输入法问题仍不属于此卡。未合并到 GPUI，未开始下一张卡。