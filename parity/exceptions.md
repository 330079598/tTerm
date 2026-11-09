# 界面对比例外

本文件登记[ui-parity.md 第 5.2 节](../docs/gpui-migration/ui-parity.md#52-通过门槛windows100-缩放macosdpr-2)所说的例外：登记过的区域不参加图像比较（遮文字后的 SSIM 与屏幕颜色门槛），改由人工判定。

- 每条以 `## E-NNN 标题` 开头，编号递增，不复用。
- `parity/screens.json` 在屏幕的 `exceptions` 里引用条目编号，写明涉及的元素与 `outset`，即从元素 border box 往外排除多少逻辑像素。元素本身仍然参加比较。对比工具在加载时检查编号是否在本文件中登记过，没登记就报错。
- 第 4 节以外新增的例外要决策人同意，并写明日期。
- 对比报告的每屏结果会列出用到的例外编号；评估时要按条目里写的方法逐一人工核对。

## E-001 模糊阴影的尾部取整

- 日期：2026-11-09
- 决策人：stone
- 涉及：`ui.switch.default` 中的 `ui.switch.checked`、`ui.switch.unchecked`、`ui.switch.focus`、`ui.switch.disabled`。`outset` 为 3，即 `shadow-xs` 的 y 偏移 1px 加模糊 2px。
- 现象：macOS 深色主题下，四个 switch 左右下圆弧外约 1.5 个 CSS 像素处，`shadow-xs`（`0 1px 2px 0 rgb(0 0 0 / 0.05)`）的尾部有 4 个物理像素，参照端是 `(27,29,34)`，GPUI 端是 `(27,28,34)`。G 通道差 1 级，CIEDE2000 为 1.146。浅色主题没有这个现象。
- 原因：GPUI 用解析近似计算带圆角的模糊阴影，WebKit 用另一套算法。5% 透明度的阴影在圆角尾部会出现 1 级的取整差，改几何去不掉；按 WebKit 去调 GPUI 的阴影，到了 Windows 的 Skia 上也不成立。
- 人工判定方法：在报告的并排截图里放大到 400% 以上，看 switch 下沿的阴影。判定标准是阴影的方向、范围和深浅与参照一致。逐像素的 1 级差异不算问题；阴影缺失、偏移或明显偏深偏浅算问题。
- 证据：[P0-24 报告](../docs/gpui-migration/reports/P0-24.md)中“Switch（评估者实现，2026-11-09）”一节。

### 扩展（2026-11-09，决策人 stone）：Tabs active trigger

- 涉及：`ui.tabs.default` 的 `ui.tabs.active`，以及 `ui.tabs.server-monitor` 的 `ui.tabs.server-active`。仅这两个 active 元素；focus trigger 不进入例外。
- `outset` 为 4，即 `shadow-sm` 的 y 偏移 1px 加 CSS 模糊 3px。
- 现象：第 2 轮深色默认屏阴影尾部物理坐标 `(78,74)` 为 `(51,56,65)` → `(51,55,65)`，ΔE00 1.0246；`(245,85)` 为 `(51,56,65)` → `(50,54,64)`，ΔE00 1.1540。第 3 轮使用两层轮廓规则后、应用本例外前，默认屏深色/浅色剩 18/99 个超限像素，最大 ΔE00 1.224873/1.912586；server 屏为 13/12 个，最大 1.224873/1.822615。实际浅色尾部差异高于深色，按复跑数字如实记录；应用本例外后两屏两主题门槛计数均为 0。独立统计与人工核对见 [P0-24 Tabs 报告](../docs/gpui-migration/reports/P0-24-tabs.md)“第 3 轮”。
- 原因与人工判定方法沿用 E-001 原文；核对对象为 active trigger 的 shadow-sm，放大 400% 以上看方向、范围和深浅，阴影缺失、偏移或明显偏深偏浅均为失败。

### 扩展（2026-11-09，决策人 stone）：右键菜单

- 涉及：`ui.context-menu.default`、`ui.context-menu.highlighted` 和 `ui.context-menu.wrapped` 中的 `ui.context-menu`。三屏为同一个容器、同一个 `shadow-lg`。`outset` 为 25，即第一层的 y 偏移 10px 加 CSS 模糊 15px。
- 现象：第 5 轮干净工作区的 macOS 复跑中，按菜单圆角 border box 外 >1 至 50 个物理像素统计，深色 default/highlighted 各有 4632 个像素差 1 级、4 个差 2 级，wrapped 有 2466 个差 1 级、4 个差 2 级；浅色 default/highlighted 各有 20788/2566/40/4/2 个像素分别差 1/2/3/4/5 级，wrapped 有 11306/938/36/6 个分别差 1/2/3/4 级。应用本扩展后，两主题三屏的颜色门槛超限均为 0；400% 核对阴影方向、范围和深浅一致。
- 原因与人工判定方法：沿用本条原文。
- 证据：[右键菜单报告“已登记例外 E-001”](../docs/gpui-migration/reports/P0-24-context-menu.md#已登记例外-e-001)。
