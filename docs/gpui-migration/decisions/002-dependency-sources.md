# 002 P0-06 的依赖来源与固定组合

- 日期：2026-11-05
- 决策人：stone
- 状态：已决定（P0-06 执行者按任务卡与[决策 007](007-official-sources-and-terminal-route.md)落定，随 [P0-06 报告](../reports/P0-06.md)一起评估）

## 背景

[决策 007](007-official-sources-and-terminal-route.md) 定了「只用官方来源」，没有定到具体版本，也没有定与构建有关的几条规则。[技术选型](../tech-selection.md)第 2.1 节把 `resolver` 是否改为 `"3"`、工具链取哪个版本留给本卡（[P0-06](../p0-validation.md#p0-06-固定依赖组合)的第 4、5 步）。本记录把这些结果固定下来，后续卡直接引用，不再各自决定。

## 决定

1. **版本与来源。**
   - `gpui-kit = "=0.9.0"`（crates.io），经它的再导出使用 GPUI（`gpui-pre 0.3.11`，Zed `zed@f816e1f` 的快照）与 gpui_platform。
   - `alacritty_terminal = "=0.26.0"`（crates.io，Alacritty v0.17.0 `94e7c8874e526b1e67b349d9ba30ddf81669119e`），不打补丁。
   - 两条都写进仓库根 `Cargo.toml` 的 `[workspace.dependencies]`，成员用 `gpui-kit.workspace = true`、`alacritty_terminal.workspace = true` 引用，不另写版本号。
   - 不直接依赖任何 `gpui-pre-*` crate。只有当 gpui-kit 的再导出确实拿不到某个接口时才直接依赖同版本的 `gpui-pre` 系列，并在这张卡的报告里写明缺什么。
   - 不引入 git 依赖、个人分支或镜像；`Cargo.lock` 里 git 来源的包数为 0。
2. **工具链保持 `1.99.1`。** 合并 GPUI 后依赖树里声明 `rust-version` 最高的是 `oo7 0.6.0` 的 1.92（只被 Linux 的 ashpd 门户用到），其次是 tauri 的 1.90；`gpui-pre` 自己不声明。1.99.1 满足全部要求，`rust-toolchain.toml` 不改。
3. **workspace `resolver` 改为 `"3"`。** 在固定工具链下删掉锁文件重新解析两次，`"2"` 与 `"3"` 得到的 1125 个包完全一致，说明今天没有行为变化；而 resolver 3 的 MSRV 感知选择可以避免以后 `cargo update` 拉进需要比 1.99.1 更新编译器的版本。
4. **开发构建沿用依赖 `opt-level = 2`。** `[profile.dev.package."*"]` 已经覆盖 GPUI，不单独调低它的优化级别：同一程序在 opt-level 0 下首次构建 48 秒、在仓库现有 profile 下 135 秒，差 87 秒，当时预计 GPUI 的文字排版与动画在 opt-level 0 下会明显卡顿。**P0-08 实测推翻了这个预期**（[P0-08 报告](../reports/P0-08.md#开发构建优化级别实测)）：`tterm-gpui` 首次构建 144 秒对 73 秒；它那个文字很少的窗口，每帧 CPU 时间从 0.06 毫秒涨到 0.45 毫秒（约 8 倍），仍远低于帧预算，真实窗口的帧间隔两者相同。终端视图的绘制量大得多，8 倍的差距那时可能变得重要，所以暂时维持 opt-level 2，由 P0-10 的绘制基准与 P0-21 按同一口径重测后再定是否给 GPUI 依赖单独降级。增量构建两者都在 1.5 秒左右。
5. **打补丁退路是 `[patch.crates-io]` 指向同版本源码目录。** 不 fork Zed、不改锁文件的来源；步骤与实际输出见 [P0-06 报告](../reports/P0-06.md#打补丁退路的实测步骤)。本次没有提交任何补丁。

## 影响

- P0-08 起新建的 crate 直接引用这两条 workspace 依赖，不再重复写版本。
- 第一次把 GPUI 真正加进 workspace 时（P0-08），Cargo 会重解析整棵树：15 个现有包被换成兼容的新版本（`icu_*` 2.3.0→2.4.0、`toml` 0.8.2→0.8.23、`yoke`/`zerovec`/`tinystr` 等补丁版本），并新增 421 个包。这不改变 tTerm 的源码，但会改变 Tauri 版的构建输入，必须在 P0-08 的同一个提交里复核并写进报告。本卡没有把这些变化带进 `GPUI`，锁文件一行未变。
- GPUI 与 tTerm 各有一个 HTTP 客户端：tTerm 用 `reqwest 0.13.5`，`gpui-kit-assets` 用改名发布的 `gpui-pre-reqwest 0.12.15`（reqwest 0.12 的快照）。两个实现会同时链进二进制，体积与 TLS 连接池都翻倍，记入 P0-26 之前的遗留问题，本卡不处理。

## 风险

- `gpui-pre` 不声明 `rust-version`，MSRV 只能从依赖树推断；一旦某个依赖抬高 MSRV，resolver 3 会悄悄挑旧版本而不是报错，要靠 CI 的固定工具链和 U-01 的比对发现。
- GPUI 的发布节奏由 gpui-kit 的维护者决定（[决策 007](007-official-sources-and-terminal-route.md) 已记录退路：直接依赖官方 Zed 的固定提交）。
- 两个 HTTP 客户端与两套 macOS/Windows 绑定（accesskit 自带的 objc2 0.5、windows 0.58）会让 release 体积继续增大；G1 的重新估算（P0-22）应以本卡测得的 2.1 MiB 空程序为起点。
