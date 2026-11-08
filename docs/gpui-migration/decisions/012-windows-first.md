# 012 先做 Windows，macOS 与 Linux 的验证推后

- 日期：2026-11-08
- 决策人：stone
- 状态：已决定

## 背景

到 2026-11-08，Windows 机器上能做的 P0 卡已经接近做完。剩下的 G1 必需项里，P0-02、P0-24 与 P0-01 的 macOS 基线要在 Mac 上做，P0-03、P0-16 与 P0-17 的 Linux 部分没有环境（[environments.md](../environments.md)）。按原计划，G1 会被这些平台卡住。

## 决定

1. **平台优先级：Windows，然后 macOS，最后 Linux。** Linux 版可以最后发布。
2. **macOS 与 Linux 的验证不再阻塞 G1 和之后的阶段。** 任务卡里只能在 macOS 或 Linux 上完成的验收项记为“推迟（决策 012）”，不记为未通过；代码仍按三平台编写，CI 仍在三平台编译和测试。
3. **macOS 的验证在发布 macOS 版之前补齐，Linux 的验证在发布 Linux 版之前补齐。** 补验证的卡另开，不在这里列出。
4. **界面一致性先在 Windows 上做。** [ui-parity.md](../ui-parity.md) 第 5.2 节的门槛原本以 macOS（DPR 2）为准，现在先以 Windows（100% 缩放）为准：参照端用 Playwright 驱动本机已安装的 Microsoft Edge（Tauri 版在 Windows 上用的 WebView2 与它同属 Chromium 内核，文字同样走 DirectWrite），GPUI 端用真实窗口的 `render_to_image`（Windows 没有无头渲染器，见 P0-10、P0-23 报告）。macOS 的对比在第 3 条的补验证中再做。
5. **外观一致的目标不变**（[决策 001](001-formal-migration.md)）。决策人接受 [ui-parity.md 第 4 节](../ui-parity.md#4-需要替代的部分)的三处替代画法：元素级毛玻璃改为不透明度更高的同色底，圆角裁剪子内容改为内边距，径向与锥形渐变改为描边与分段近似。不给 GPUI 打补丁做原样效果；这三处在 `parity/exceptions.md` 中按第 4 节登记为例外，人工判定。

## 影响

- G1 必需项中，P0-02、P0-03、P0-16 改为推迟；P0-17、P0-18 只做 Windows 部分；P0-01 的 macOS 基线推迟。
- W0-04 的对比工具需要增加 Windows 通道，作为 P0-24 的前置工作；P0-24 改在 Windows 上做。
- 推迟的验证越晚做，发现平台问题时要返工的代码越多。macOS 尤其如此：签名、钥匙串、Metal 渲染与输入法都只在 macOS 上暴露。
