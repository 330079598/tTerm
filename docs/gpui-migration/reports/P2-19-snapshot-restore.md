# P2-19 普通启动恢复旧快照

2026-11-26，分支 `migrate/P2-19-snapshot-restore`。只读恢复的生产路径已经在 `43bf05bd` 落地；本轮核对后没有改生产代码，只把普通启动的自动回归扩到多标签、副作用控制串、损坏/截断/超限/缺失文件、绕过路径和保存后文件不变，并记录结果。未修改 `status.md`，未合并，未推送。

## 行为与实现（核对结论）

- 入口在 `window.rs`：`session_workspace::startup(runtime, command.uses_saved_workspace(), app_config.restore_terminal_content)`。只有普通启动（`CommandSpec::Shell`，没有 `--exec`/`--ssh`）且设置开启时才读取；`restore_terminal_content` 的默认值是 `true`。
- `legacy::read_saved` 只读当前数据根的 `<config_dir>/scrollback/<tab-id>.gz`，按工作区里每个标签的 id 逐个读取；不搜索其他客户端的数据目录。单文件压缩与解压各限 32 MiB，跨标签保留的解压文本共限 64 MiB。
- `restore_workspace` 用标签 id 取对应快照，所以文件顺序与标签打开顺序无关；内容为空白时不产生恢复分隔行。分隔文本走 i18n，按保存时间生成。
- `Session::spawn` 在新 PTY 和 parser 存在之前调用 `restore_legacy_history`，临时解析器只保留字符、颜色和旧序列化器使用的绘制操作。快照里的 OSC 0/52/9/133、设备查询、备用屏、鼠标、括号粘贴、隐藏光标和 OSC 8 超链接都不会生效：不写剪贴板、不改标题、不发送查询应答、不进入模式。
- 恢复的历史在分隔行之前，新会话输出在分隔行之后；历史不会写入 PTY，也不会重复回放。
- GPUI 保存工作区布局时不会删除、改写或新增 `scrollback/*.gz`；不属于当前工作区的旧文件原样保留。
- 结论：普通启动恢复旧快照的生产路径已满足本卡要求，不需要实现改动。

## 测试

`cargo test -p tterm-gpui --lib legacy_snapshot -- --nocapture`：2 passed，0 failed。

- `legacy_snapshot_startup`：在隔离数据根里写入真实 SQLite 工作区与 gzip 快照，普通启动后打开 7 个标签。三条快照正确归属到各自的标签，红色文本保留调色板颜色，历史先于分隔行；损坏、截断、超限、缺失的标签仍能启动且没有历史；恶意快照只画出可见文本。随后向新 PTY 写入命令，结果落在分隔行之后，历史只出现一次。保存布局后，目录内文件集合与内容哈希都不变。
- `legacy_snapshot_bypasses_explicit_launch_and_disabled_setting`：`--exec` 与 `--ssh` 不恢复也不读取；关闭设置时保留普通启动但跳过快照；开启设置且目录存在时才读取；目录删除后不报错、不创建文件、不清理无关文件。
- 两个用例都在独立子进程和独立数据根里运行，避免与全局 config/database 单例竞争，也不会打开用户数据。

## 检查记录

- `cargo fmt --all --check` 通过。
- `cargo clippy -p tterm-gpui --all-targets -- -D warnings` 通过。

## 未验证项

- 没有做两个桌面客户端共用同一数据根的人工互操作：Tauri 版写快照后由 GPUI 普通启动恢复，以及 GPUI 保存布局后回到 Tauri 版打开。自动测试只覆盖 GPUI 一侧。
- 没有在 Windows 与 Linux 实机上运行该回归；本轮只在 macOS arm64 上执行。
- 没有决定原生写入格式（P2-19 第 2 项）；GPUI 版仍只读旧快照，不写新快照。
- 本轮按本卡范围只跑了相关过滤测试与目标 crate 的 clippy，没有重跑 `pnpm` 全套和 `cargo test --workspace`；前端代码未改动。
