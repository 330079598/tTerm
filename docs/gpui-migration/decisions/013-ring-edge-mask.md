# 013 focus ring 外沿纳入对比工具的边缘遮罩

- 日期：2026-11-09
- 决策人：stone
- 状态：已决定

## 背景

P0-24 在 macOS 上对比 Button 时，`ui.button.variants` 只剩 focus 按钮的 ring（Tailwind `ring-[3px]`，即无偏移、无模糊的 box-shadow 外扩）不达标：ring 外沿圆弧有深色 24 个、浅色 16 个物理像素的 CIEDE2000 超过 1，最大 2.006。原因是 WebKit 与 GPUI 对圆弧的抗锯齿衰减不同。原有的边缘遮罩只覆盖元素自身的 border box，ring 外扩 3px，不在遮罩里。Windows 通道的 Edge 用 Skia，衰减又不一样，按 WebKit 去调 GPUI 的抗锯齿，换到 Windows 也用不上。

## 决定

1. 对比工具把无偏移、无模糊的外扩阴影（ring）的外轮廓视为边缘：矩形向外扩 spread，圆角按 CSS Backgrounds 3 对 spread 的规则计算，排除轮廓两侧各 1 个物理像素，与元素边缘带的抗锯齿余量相同。
2. ring 内部的像素仍参与屏幕颜色门槛；两端的 spread 记入布局数据，并按样式检查比较。GPUI 端的 spread 必须与绘制同源。
3. 第 5.2 节的门槛数值不变，这条规则不算例外，不在 `parity/exceptions.md` 登记。
4. 不为匹配某个浏览器的抗锯齿曲线去改 GPUI 的绘制。

## 影响

- 之后所有带 focus ring 的组件（Input、Select、Switch、Tabs 等）都按这条规则对比，不必逐个登记例外。
- 有偏移或模糊的阴影不在这条规则内，遇到时另行处理。
- 实现见 P0-24（`a6222e0a`），规则说明在 [parity/README.md](../parity/README.md)。

## 补充（2026-11-09，决策人 stone）：有偏移的 ring

有偏移的 ring（例如 `ring-2 ring-offset-2`）由多层零偏移、零模糊的外扩阴影组成。元素的每一层非 inset、x/y/blur 均为 0、颜色不透明度大于 0 且 spread 为正的阴影，其外轮廓都视为边缘，轮廓两侧各排除 **1 个物理像素**。每层轮廓矩形按该层 spread 外扩，圆角按 CSS Backgrounds 3 §6.1.1 的 spread 规则计算。

原文“有偏移或模糊的阴影不在这条规则内”保持不动：有偏移的 ring 现在由本补充覆盖，此处的偏移指 ring-offset，不是 box-shadow 的 x/y 位移；有模糊的阴影以及 x/y 非零的阴影仍不在范围内。ring 内部及 offset 那一圈的填充像素仍计入颜色门槛，不能把两条轮廓之间的区域整体遮掉。

布局保留最外层 `ringSpread`，新增按升序排列的 `ringSpreads`，记录全部符合条件的层。两端数组必须通过样式比较；GPUI 从绘制用的同一份计算样式记录各 spread。布局 schema 升至 5，缺少该字段或旧版本的缓存拒绝复用。颜色、SSIM 与位置门槛不变。反向探测必须证明整体平移 ring 1px 或改变 offset 填充颜色仍会失败。
