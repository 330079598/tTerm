# 依赖固定组合

本文件记录 GPUI 版使用的每个外部依赖的来源、固定 rev、版本与许可，由 [P0-06](p0-validation.md#p0-06-固定依赖组合) 填写。[README 执行规则第 5 条](README.md#2-执行规则)要求新依赖使用本表登记的版本，且不得引入其他 GPUI 来源；U-01 每月按本表记录的提交对比上游变化。

下表是 P0-06 必须填写的行（2026-11-05 按[决策 007](decisions/007-official-sources-and-terminal-route.md)修订：只用官方来源，不使用任何 git 依赖或个人分支）。填写前所有值为“待定”，不要用未核实的版本代替。

| 依赖                                 | 来源                           | 固定版本 | 对应上游提交 | 许可 | 核验日期 | 备注 |
| ------------------------------------ | ------------------------------ | -------- | ------------ | ---- | -------- | ---- |
| gpui-kit                             | crates.io                      | 待定     | 待定         | 待定 | 待定     | 待定 |
| GPUI（`gpui-pre`，经 gpui-kit 带入） | crates.io                      | 待定     | 待定         | 待定 | 待定     | 待定 |
| alacritty_terminal                   | crates.io                      | 待定     | 待定         | 待定 | 待定     | 待定 |
| Alacritty 应用层（P0-28 复制的源码） | github.com/alacritty/alacritty | 不适用   | 待定         | 待定 | 待定     | 待定 |
| nyaterm 模块（P0-29 复制的源码）     | github.com/nyakang/nyaterm     | 不适用   | 待定         | 待定 | 待定     | 待定 |
| rustls 后端（统一为 `ring`）         | 待定                           | 待定     | 待定         | 待定 | 待定     | 待定 |
| reqwest                              | 待定                           | 待定     | 待定         | 待定 | 待定     | 待定 |
| vte                                  | 待定                           | 待定     | 待定         | 待定 | 待定     | 待定 |
| windows / objc2                      | 待定                           | 待定     | 待定         | 待定 | 待定     | 待定 |
| 工具链（`rust-toolchain.toml`）      | 待定                           | 待定     | 待定         | 待定 | 待定     | 待定 |

填写要求：

1. **来源。** crates.io 的依赖写 crates.io；复制进来的源码写官方仓库地址。不得出现 git 依赖、镜像或个人分支。
2. **固定版本。** crates.io 依赖在 `Cargo.toml` 中用 `=` 精确固定，起点为 gpui-kit `0.9.0`、`gpui-pre 0.3.11`、`alacritty_terminal 0.26.0`。gpui-kit 与 `gpui-pre` 成对升级。
3. **对应上游提交。** `gpui-pre` 写它快照的 Zed 提交（0.3.11 为 `zed@f816e1f`）；复制的源码写来源提交的完整哈希，与 `third-party.md` 一致。
4. **许可。** 以固定版本上的 `LICENSE`、`COPYING` 或 `NOTICE` 为准；gpui-kit 的 `Cargo.toml` 没有 `license` 字段，按仓库中的 `LICENSE-APACHE` 核对。
5. **核验日期。** 写实际核对许可与三平台编译的日期。
6. **备注。** 记录依赖冲突的处理与未决问题：rustls 统一为 `ring` 后 `aws-lc-rs` 是否仍被间接引入；`reqwest` 是否两个版本共存；`vte` 的解析版本；`windows`、`objc2` 的重复版本中哪些类型需要跨 crate 统一。
7. **工具链。** 本行引用仓库根 `rust-toolchain.toml` 的 `channel`，改版本时同时改文件与本表。
8. **打补丁记录。** 若按 P0-06 的退路给 `gpui-pre` 打了补丁，在备注中写明补丁文件、原因与对应的 P0-07 条目。
