# 依赖固定组合

本文件记录 GPUI 版使用的每个外部依赖的来源、固定 rev、版本与许可，由 [P0-06](p0-validation.md#p0-06-固定依赖组合) 填写。[README 执行规则第 5 条](README.md#2-执行规则)要求新依赖使用本表登记的版本，且不得引入其他 GPUI 来源；U-01 每月按本表记录的提交对比上游变化。

下表是 P0-06 必须填写的行。填写前所有值为“待定”，不要用未核实的 rev、版本或标签代替。

| 依赖                                   | 来源仓库（含镜像） | 固定 rev | 版本或标签 | 许可 | 核验日期 | 备注 |
| -------------------------------------- | ------------------ | -------- | ---------- | ---- | -------- | ---- |
| GPUI（`nyakang/zed`）                  | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| gpui-kit（`nyakang/gpui-kit`）         | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| Alacritty（`nyakang/alacritty`）       | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| rustls 后端（统一为 `ring`）           | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| reqwest（`zed-reqwest` 0.12 的 patch） | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| vte                                    | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| portable-pty                           | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| windows / objc2                        | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |
| 工具链（`rust-toolchain.toml`）        | 待定               | 待定     | 待定       | 待定 | 待定     | 待定 |

填写要求：

1. **来源仓库（含镜像）。** 同时写 tTerm 自己的镜像与它对应的上游（如 `nyakang/zed`）；`Cargo.toml` 指向镜像与固定 rev，不直接指向上游分支。
2. **固定 rev。** 写完整的提交哈希。按[技术选型第 3 节](tech-selection.md#3-gpui-来源)选一个 nyaterm 已发布版本对应的组合，不要用 2026-11-01 验证报告中的旧 rev。
3. **版本或标签。** 写对应的 nyaterm 发布版本与 crate 版本。GPUI 没有正式发布，写 nyaterm 固定该 rev 的版本。
4. **许可。** 以固定 rev 上的 `LICENSE`、`COPYING` 或 `NOTICE` 为准；gpui-kit 的 `Cargo.toml` 没有 `license` 字段，按仓库中的 `LICENSE-APACHE` 核对。
5. **核验日期。** 写实际核对许可与三平台编译的日期。
6. **备注。** 记录依赖冲突的处理与未决问题：rustls 统一为 `ring` 后 `aws_lc_rs` 是否仍被间接引入；`reqwest` 用 `[patch]` 还是两个版本共存；`vte` 的解析版本；`windows`、`objc2` 的重复版本中哪些类型需要跨 crate 统一；`portable-pty` 是否仍用 crates.io 的 0.9。
7. **工具链。** 本行引用仓库根 `rust-toolchain.toml` 的 `channel`，改版本时同时改文件与本表。
8. **gpui_platform。** 与 GPUI 同源，P0-06 在同一行或另起一行记录其 rev 与许可。
