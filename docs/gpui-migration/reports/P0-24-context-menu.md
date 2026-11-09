# P0-24 第二批：右键菜单（macOS）

状态：**需要确认**。本轮未完成组件交付，未达到验收标准；没有把试作或失败的对比屏作为完成品提交。

## 偏差与停止原因

任务要求菜单图标为 **14px**，同时要求使用真实生产组件作为参照、不修改生产 CSS，并达到第 5.2 节门槛。实际生产菜单的图标是 **16×16px**。

- `src/components/ContextMenu.tsx:64` 等处给 Lucide 传入 `size={14}`，输出 SVG 的 width/height 属性。
- `src/components/ui/button.tsx:7` 的 `[&_svg:not([class*='size-'])]:size-4` 为这些 SVG 设置 CSS width/height，覆盖属性值。菜单图标没有 `size-*` class，因此该规则生效。
- 沙箱外 WebKit 真正捕获的布局数据中，五个可见菜单图标均为 16×16 逻辑像素。第一个在 `(551, 438)`，其余在 `(551, 466)`、`(551, 494)`、`(551, 531)`、`(551, 559)`。800×600 逻辑窗口，DPR 2。原始 `icons` 字段摘录见 [icon-size-evidence.json](assets/P0-24-context-menu/icon-size-evidence.json)。

这是明确的 CSS 尺寸覆盖，不是抗锯齿差异。继续按 14px 画 GPUI 将与生产参照不同；改生产 CSS 又超出本轮授权。因此依据本轮硬性规则“任务卡、报告与代码对不上时停下，写进报告的‘偏差’”，停止实现并请求确认尺寸口径。

建议确认：**按生产实际的 16px 还原，还是另行授权把生产菜单图标改成 14px 后再迁移。** 本轮没有自行选择，也没有扩大遮罩、放宽门槛、登记例外或修改生产 CSS。

## 已读代码结论

已通读 `ContextMenu.tsx` 与 `src/types/tab.ts:312` 的 `TabContextMenuAction`。

| 项目类型 | 生产支持情况 |
| --- | --- |
| 普通项 | `label`、`action`，可有 `icon`。调用回调后关闭。 |
| 分隔线 | `separator: true`，不进入键盘导航。`bg-border my-1 h-px`。 |
| 禁用项 | `disabled`，不进入键盘导航，不触发回调。保留原色并叠加 opacity。 |
| 勾选项 | 无 checked 字段或勾选绘制。 |
| 子菜单 | 无 children/submenu 字段或展开逻辑。 |
| 快捷键标签 | `shortcut` 是 KeymapActionId，经 `useShortcutLabel` 读取当前绑定；非固定字符串。`kbd` 使用 muted foreground，`ml-auto shrink-0 pl-3 font-sans text-xs`。 |
| 危险项 | 无 destructive 字段或专用样式；关闭/删除类 action 仍使用 ghost Button。 |

容器由 Card 与 CardContent 组成：popover 背景、Card 前景与边框，6px 圆角、`shadow-lg`，外层 `py-1`，内容 `p-1`，宽度 `min(16rem, 100vw - 20px)` 且 `min-w-52`。行是 ghost Button，`h-auto`、`gap-1.5`、`px-2 py-1.5`、普通字重、允许折行。键盘焦点走 Button 的 focus-visible ring；hover 走 ghost 的 accent 背景（深色 alpha 0.5），两者不能当作同一种实心蓝色高亮。

定位规则在 `fitInViewport`：只有 `x + width > innerWidth` 时向左平移到 `innerWidth - width - 10`；只有 `y + height > innerHeight` 时向上平移到 `innerHeight - height - 10`；最终两轴各取 `max(10, adjusted)`。这是平移，没有围绕指针翻转的分支。菜单在 body 下使用 fixed portal，不受祖先 transform 影响。

键盘初始聚焦第一个可用项；上下键和 Tab 循环移动并跳过禁用项/分隔线，Home/End 到首尾；Enter/空格触发当前项并关闭，Esc 关闭。document mousedown 落在菜单外时关闭。卸载时若焦点退到 body，尝试恢复打开前焦点。

真实标签页调用目前在 `src/components/TabBar/TabItem.tsx:75`，由 TabBar 使用（已从题目列举的大文件拆出）。页面标签分支提供新建、重新打开、分隔线和 `getTabCloseMenuActions` 的四种关闭操作；关闭左/右/其他是否禁用由标签位置决定。后续夹具应直接复用此处调用及生产 helper，不能使用本轮临时试作里手写的 action 集合。

## 试作与验证记录

发现冲突前曾添加临时 GPUI 实体和两端屏幕用于捕获。发现尺寸冲突后，**所有试作源码、注册、生成图标、provider 改动和临时测试调整均已撤下**，仅保留本报告和 JSON 证据。本轮不宣称完成封装方案评估、主题隔离或交互测试。

沙箱外执行的捕获命令：

```sh
UPDATE_REFERENCE=1 CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
```

临时深色对比退出 1：已有七屏通过，新增两屏失败。临时新增屏有布局标记不一致、行高/位置/高亮错误及图标差异，不能作为组件可用的证据，失败不归因于抗锯齿。完整临时结果留在本 worktree 的 `target/parity/report/result.json`；该目录后续重跑会覆盖。图标尺寸摘录已提交为稳定证据。

| 屏幕 | 深色 macOS | 浅色 macOS | Windows |
| --- | --- | --- | --- |
| `ui.context-menu.default`（临时试作，已撤下） | 未通过；屏幕颜色门槛 42093 px > ΔE00 1，最大 73.4463；遮文字 SSIM 0.966835，整屏 0.946805；5/5 图标失败 | 未运行 | 未验证（需 Windows） |
| `ui.context-menu.highlighted`（临时试作，已撤下） | 同上；两屏当时尚未实现独立状态 | 未运行 | 未验证（需 Windows） |

临时验证中，前端全量 Vitest 174 文件、1294 测试通过；局部 Rust 测试 tterm-ui 30、tterm-parity 76 通过。但这不代表最终组件验收通过，因试作已撤下。**未执行完成态的 AGENTS.md 全部检查**，也没有跳过测试、加 allow/expect 或修改限制。

环境：Mac mini M4，macOS Apple Silicon，800×600 逻辑像素，DPR 2；参照提交键 `6a72ac72c468aabf1b839c912844d36e663ff05f-dirty`（临时 fixture/provider 改动）。GPUI 捕获通过仓库的 macOS 开发 bundle 运行，没有启动正式版或 Tauri 开发版，没有读写真实数据目录。沙箱内捕获的 NSPasteboard 失败在沙箱外复跑成功。脚本退出后核对进程，无本轮 Vite、WebKit 或 GPUI 残留。

## 未验证与 [人工] 项

- 需要先确认图标使用生产实际的 16px，或另行授权生产端改为 14px。
- GPUI 组件、两端内部 data-slot 对齐、与绘制同源的样式快照尚未完成。
- `shadow-lg` 换算单元测试、右/下/右下角定位测试、真实 GPUI 键盘/鼠标交互测试尚未交付。
- 同一窗口放两套主题并前后读取全局 Theme 的真实渲染隔离测试尚未交付。
- macOS 深浅主题完整 parity 通过证据与全量最终检查尚未交付。
- Windows 通道：未验证（需 Windows）。
- [人工] 真实 Tauri 窗口与 WebKit 参照的交叉截图未验证。
