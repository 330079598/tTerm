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

### 扩展（2026-11-09，决策人 stone）

- 涉及：`ui.dialog.confirm` 的 `ui.dialog.confirm.content`、`ui.dialog.destructive` 的 `ui.dialog.destructive.content`、`ui.dialog.prompt` 的 `ui.dialog.prompt.content`。
- `outset` 为 25，即 `shadow-lg` 第一层的 y 偏移 10px 加 CSS 模糊 15px；内容框本身仍参加比较。
- 现象：macOS 深色主题下，确认/破坏性确认每屏有 10,030 个框外物理像素的 RGB 通道最大差 1 级，Prompt 为 10,862 个；每屏另有 24 个差 2 级，最大 CIEDE2000 为 1.49。浅色主题每屏只有 2 个像素超过颜色门槛，均位于内容框下圆角外侧且在 outset 25 内。原因与人工判定方法沿用 E-001 原文。
- 证据：[P0-24 Dialog 报告“第 3 轮：E-001 扩展与干净工作区复跑”](../docs/gpui-migration/reports/P0-24-dialog.md#第-3-轮e-001-扩展与干净工作区复跑)。
