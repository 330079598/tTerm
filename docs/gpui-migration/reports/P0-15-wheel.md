# P0-15-wheel 修正原型的滚轮方向与每格事件数

## 结论

部分完成，停在任务卡工作第 3 项要求的决策点：已确认方向取反与一格三次上报的原因，也确认 xterm.js 与 Alacritty 的备用屏幕计数不同；尚未修改行为、测试或生成修复后的截图。不能作为修复完成或进入 P2 的依据。

环境：2026-11-07，Windows 11，PowerShell；worktree `D:\code\rust\tTerm-P0-15-wheel`，分支 `migrate/P0-15-wheel-fix`，起点 `3c6665e17d5ce0241ed6ae8b3eae4e33d735d0a2`。未启动任何客户端，未读写 `com.stone.tTerm`、`com.stone.tTerm.dev` 或开发数据目录。后续原型运行必须使用 `com.stone.tTerm.gpui-dev`。

## 改动

- 本次仅新增这份调查及决策报告；报告提交可用 `git log -1 --format='%h %s' -- docs/gpui-migration/reports/P0-15-wheel.md` 查询。
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

当前 `crates/tterm-terminal-view/src/view/scroll.rs:24–25` 对两种 delta 都取负。因此 Windows 向外一格得到 `Lines.y = +3` 后被转成 -3，历史滚动、备用屏幕方向键、鼠标上报全部反向。唯一需要修正符号的入口是这个累积器；无需在三条路由分别补偿。

一格三次上报不是 Lines 转像素又除行高引入的倍数：这两步相互抵消。Windows GPUI 已乘系统行数；`input/wheel.rs:83–87, 120–122` 再按 `abs(lines)` 生成按钮，`view/wheel.rs:43–50` 逐个发送，所以在系统值为 3 时恰好产生三次。备用屏幕的 `repeat_arrow` 同样按行数重复。当前 view 没有额外再乘 3。

Tauri 目标从仓库锁定的 `@xterm/xterm` **6.0.0** 源码确认，读取位置为主 checkout 的 `D:\code\rust\tTerm\node_modules\@xterm\xterm\src`（本卡 worktree 尚未安装 node_modules）。`src/components/TerminalTab` 未覆写 `scrollSensitivity`；xterm `common/services/OptionsService.ts:37,39` 的默认值为 1 和 0（平滑时长）。

| 路径 | xterm.js 6.0.0 的源码行为及数字 | 证据 |
| --- | --- | --- |
| 鼠标上报 | `consumeWheelEvent` 非零后，单次调用 `triggerMouseEvent`；一个有效滚轮事件仅发送一个上报，不按返回行数循环。向外为 64、向内为 65。 | `browser/CoreBrowserTerminal.ts:641–674`；`common/services/CoreMouseService.ts:241–269`；P0-15 已有真实输入截图 `assets/P0-15/manual/tauri-mouse-sgr-drag.png` |
| 备用屏幕 | 同样只用消费结果判断是否非零，然后只发一条方向序列。DECCKM 开时向外为 `ESC O A`，向内为 `ESC O B`；DECCKM 关时 xterm 发 CSI。 | `browser/CoreBrowserTerminal.ts:814–840`；本卡不顺带修改 SS3/CSI 模式策略 |
| 本地历史 | 常规 Windows Chromium/WebView 滚轮 `wheelDeltaY = ±120` 在 StandardWheelEvent 归一化为 ±1，滚动器乘 50，即每格 50 CSS px，再按单元格高度用 `Math.round` 计算显示行。不是固定每格 3 行，也不能把两格 5 行简单当成所有字体下每格 2.5 行。 | `vs/base/browser/mouseEvent.ts:155–160`；`vs/base/browser/ui/scrollbar/scrollableElement.ts:23,407,446`；`browser/Viewport.ts:128,184` |

P0-15 既有截图已逐张查看：GPUI start 首行 160，朝用户一格后 157，再向外两格回到 160；Tauri start 首行 163，朝用户一格不动，再向外两格为 158。与其报告一致。鼠标图中 GPUI 每方向三条，Tauri 每方向一条。它们是修复前的历史证据，本轮未重跑，不挪作修复后的截图。

触控板还有另一处计数差异：xterm 的 `consumeWheelEvent` 对像素 delta 除单元格 CSS 高，`abs(deltaY) < 50` 时再乘 0.3，保留小数后供路由判断；本卡现有累积器直接除行高并在换向时清余数。任务要求保留小数累积和换向清零，因此不能未经决策照搬整套 xterm 输入处理。

### 与 Alacritty 的冲突及待选方案

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

这与 xterm 的备用屏幕每个有效事件一条、历史按 50 CSS px 的策略不一致，触发任务卡的明确停止条件。以下仅列方案，尚未选择或实现：

| 方案 | 具体行为 | 好处 | 代价 / 需要明确的事项 |
| --- | --- | --- | --- |
| A：按本卡目标对齐 Tauri | 修正共同符号入口；有效离散输入在鼠标/备用屏幕路径各发一次；本地历史按 xterm 的像素目标计算。保留本卡要求的触控板累积和换向清零。 | 符合本卡验收及用户在现有 Tauri 版中的习惯。 | 明确偏离 Alacritty 的备用屏幕倍数和历史行数；需要认可这项例外，并明确采用 xterm 的 50 CSS px 历史目标。 |
| B：保持 Alacritty 滚动计数 | 先正确归一化 GPUI 的系统行数，再采用 Alacritty 的鼠标倍率 1、其他路径默认倍率 3。 | 沿用已有路由的上游计数语义。 | 备用屏幕每格三条与本卡“每格事件数与 Tauri 版相同”冲突，必须先修改任务卡；历史也不等同 Tauri。 |
| C：协议次数对齐 Tauri，历史沿用 GPUI 系统行数 | 鼠标和备用屏幕各一次；本地历史保留系统每格行数，只统一修正方向。 | 改动较小，保留系统历史滚动速度。 | 既偏离 Alacritty 的备用屏幕行为，也不能满足历史速度与 Tauri 对照一致，必须明确缩小本卡的对齐范围。 |

无论采用哪种方案，都不能对所有 Lines 直接除以常量 3：Windows 系统行数可变，macOS 的 Lines 也不承诺每格为 3。GPUI 事件只保留归一化后的行数，不携带原始 notch 数，快速滚动合并和高分辨率滚轮也需要在既定范围内明确如何计数。

未直接阅读 Zed 仓库的任何提交或 `terminal`、`terminal_view` 文件。本次阅读的 GPUI crates.io 源码标注快照为 `zed@f816e1f`，文件如上表。没有复制 Zed 的代码、注释、测试或私有标识符；本次也未向生产代码复制任何上游实现。

## 验收

| 验收条件（照抄任务卡） | 结果 | 证据 |
| --- | --- | --- |
| 报告写明 GPUI 0.3.11 的 ScrollDelta 符号约定及三个平台的源码位置。 | 通过（源码调查） | 上述平台表、复核命令与原始输出。macOS、Linux 均为“源码确认，未运行验证”。 |
| 向远离用户的方向滚一格：历史区向上进入历史；鼠标上报为一个 wheel-up（SGR 按钮 64）；备用屏幕发送 ESC O A。朝用户方向相反。每格事件数与 Tauri 版相同。 | 未通过 | 尚未修改现有反向和计数行为；在工作第 3 项停止。 |
| 方向与每格事件数有自动化测试，测试名列在报告中；把修正撤销后这些测试会失败（报告附失败输出，临时改动不提交）。 | 未通过 | 尚未新增测试或实施临时回退验证；现有 `view::scroll::tests` 仍按旧符号断言。 |
| Windows 上的真实滚轮结果有截图，或标为 [人工] 并写好步骤；macOS、Linux 为“源码确认，未运行验证”。 | [人工]，未验证 | 下述重跑步骤；决策前未生成 `assets/P0-15-wheel/` 的修复截图。 |

## 检查

AGENTS.md 规定的完整检查如下，**本轮未运行，不声明通过**。原因是工作第 3 项明确要求遇到 Alacritty 行为取舍时停下，本轮只提交调查报告；实现完成后仍必须全部执行并补入实际输出。

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

本轮文档检查：`git diff --check`，退出码 0，无输出。

## 偏差

- 任务卡工作第 3 项原文：“如果对齐 Tauri 版与 Alacritty 的行为冲突、需要取舍，停下，在报告中列出可选方案和利弊，不要自行决定。”备用屏幕计数和本地历史距离已发现上述冲突，故未继续工作第 2、4、5 项，也未完成修复提交。需要决策人选定滚动策略后在同一分支继续。
- 文件路径与任务卡相符，未发现需要扩大实现范围的路径偏差。

## 遗留问题

待决策：从 A/B/C 中确定策略，尤其是本地历史采用 xterm 的 50 CSS px 还是系统行数。A 最符合原卡的 Tauri 对齐目标，但需要明确认可与 Alacritty 的差异。本报告不替决策人选择。

决策后的 `[人工]` Windows 验证步骤（本轮未执行）：

1. 从仓库根运行 `cargo build --workspace`，启动本分支生成的 `tterm-gpui.exe`，确认应用标识及数据目录为 `com.stone.tTerm.gpui-dev`。记录字体、行高、缩放、Windows 每格滚动行数。每次真实输入前核实进程路径、窗口标题及前台窗口确实属于此原型，否则停止。
2. 用原 P0-15 命令 `--exec wsl.exe -d Ubuntu -- bash -c "seq 1 200; sleep 120"`。截图底部初态，向外一格后截图、记录首行及位移，向内一格后截图、记录恢复情况。不要把顶部/底部钳制后的距离作为每格速度证据。
3. 用 `--exec wsl.exe -d Ubuntu -- python3 /mnt/d/code/rust/tTerm-P0-15-wheel/scripts/terminal-probe.py --mouse 1002,1006`。指针停在同一格，向外一格、向内一格，分别核对恰好一个 `<64;…M` 和一个 `<65;…M`，保存 RX 原始字节及截图。
4. 在 WSL shell 中运行 `printf '\033[?1049h\033[?1007h'; python3 /mnt/d/code/rust/tTerm-P0-15-wheel/scripts/terminal-probe.py --application-cursor`。向外/向内各一格，截图并核对接收次数和 `1b 4f 41` / `1b 4f 42`。Ctrl+C 退出探针，再执行 `printf '\033[?1049l'` 恢复主屏。
5. 保存本次新证据到 `docs/gpui-migration/reports/assets/P0-15-wheel/`，使用 `history-start.png`、`history-away.png`、`history-toward.png`、`mouse-away.png`、`mouse-toward.png`、`alternate-away.png`、`alternate-toward.png`，并补录实际观测。macOS、Linux 保持“源码确认，未运行验证”，不标为运行通过。

P0-12 的边界、小数滚动限制和 P0-13/P0-28 其他按键、鼠标、输入法问题仍不属于此卡；没有开始下一张卡或合并到 GPUI。
