# 006 W0-01 的 CI 验收暂缓

- 日期：2026-11-05
- 决策人：stone
- 状态：已决定

编号 002 至 005 已分给 P0-06、G1、P0-26 与 G0 的决策记录，所以本记录用 006。

## 背景

W0-01（[报告](../reports/W0-01.md)）把 Cargo workspace 根移到仓库根目录，并改了四个工作流。评估结论：代码没有缺陷，本地全部检查通过（评估者在 `5870f06` 上重跑，前端 1250 个测试、Rust 817 个测试通过）。唯一没有满足的验收条件是三平台 CI，因为迁移工作暂时只在本地的 `GPUI` 分支上进行，不推送。

决策人已在 macOS 上本地运行 `pnpm tauri:dev`，确认开发版正常启动。

## 决定

1. W0-01 记为“通过，CI 未验证（已豁免）”，后续卡可以以它为依赖开始。
2. 豁免有期限：`GPUI` 第一次推送到远程时，必须先手动触发 `check.yml` 并在三平台通过；`GPUI` 合入 `dev` 之前，这一项必须完成。
3. 发布流程（`release-main.yml`、`release-dev-beta.yml` 中的 `target/` 路径、`rustup show`、macOS 编译目标）在 `GPUI` 合入 `dev` 后的第一次正常 beta 发布中验证，由评估者核对各平台安装包与 `latest.json`。

## 风险

在补跑之前，Windows 与 Linux 上的问题不会暴露，例如 Windows PowerShell 下的 `rustup show`、`swatinem/rust-cache` 的缓存路径、`tauri-action` 能否在根目录 `target/` 找到安装包。后续卡如果改动 CI，同样只能在本地验证，问题会累积到第一次推送时一起出现。
