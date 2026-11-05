# tterm-gpui

P0-08 的最小 GPUI 程序。它只证明固定版本的 `gpui-kit` 能在三平台编译、打开一个窗口、正常退出，并能用无界面的窗口测试断言交互；终端从 P0-09 开始，本 crate 目前没有任何业务功能。

## 结构

| 文件                 | 职责                                                                        |
| -------------------- | --------------------------------------------------------------------------- |
| `src/main.rs`        | 入口，转到 `app::run`。                                                     |
| `src/app.rs`         | 窗口内容、`Increment` 与 `Quit` 动作、`cmd-q` 绑定、关闭最后窗口时退出。    |
| `src/app/tests.rs`   | 无界面测试：打开窗口、派发动作、点击按钮，断言可见计数变化。                |
| `Cargo.toml`         | 依赖只写 `gpui-kit.workspace = true`，开发依赖另开 `test-support`。         |

## 依赖与工具链

- 工具链由仓库根的 `rust-toolchain.toml` 固定为 `1.99.1`。
- `gpui-kit` 由仓库根的 `[workspace.dependencies]` 固定为 `=0.9.0`，本 crate 不另写版本号。GPUI 与平台层都经它的再导出使用，不直接依赖任何 `gpui-pre-*` crate。
- 不启用 `gpui-kit` 的 `gpui-fast` 特性：它是非官方的 GPUI 变体，决策 007 只允许官方来源。`test-support` 只在开发依赖里启用，用来跑无界面测试。
- 不引入 git 依赖。`Cargo.lock` 里 `source = "git+` 的行数为 0。

## 构建

```sh
cargo build -p tterm-gpui
```

Linux 需要 GPUI 的系统依赖。CI 的 apt 行（`.github/workflows/check.yml` 的 `Install dependencies`）安装了：

```text
libfontconfig-dev libfreetype-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev
libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
libvulkan-dev libgl1-mesa-dev libegl1-mesa-dev libdbus-1-dev
```

X11 与 xkbcommon、Wayland 供窗口与键盘使用；fontconfig 与 freetype 供字体度量；Vulkan 与 GL/EGL 是 wgpu 的后端。缺少这些包时，`xkbcommon-sys`、`wayland-sys`、`yeslogic-fontconfig-sys`、`freetype-sys` 之类的 `-sys` crate 会在构建脚本里失败。本卡没有 Linux 机器，CI 也没有推送，因此这条清单按 GPUI 的构建依赖整理，未在 Linux 上逐条复现，见报告“偏差”。

Windows 不需要额外的系统依赖；macOS 只需要 Xcode 命令行工具。

## 运行

```sh
cargo run -p tterm-gpui
```

macOS 上 `.cargo/config.toml` 的 runner 会调用 `scripts/macos-dev-bundle.sh`。对 `tterm-gpui`，脚本：

- 生成独立的应用包 `target/debug/bundle/dev-gpui/tTerm GPUI Dev.app`，不共用 Tauri 开发版的 `bundle/dev/tTerm Dev.app`；
- 使用应用标识 `com.stone.tTerm.gpui-dev` 与显示名 `tTerm GPUI Dev`，不读写 `com.stone.tTerm.dev` 的数据（执行规则第 7 条）；
- 不读 `TAURI_CONFIG`，该变量只属于 `tauri dev`；
- 以 `src-tauri/Info.plist` 为底生成 `Info.plist`，因此保留 `NSLocalNetworkUsageDescription`；
- 使用 `src-tauri/icons/icon.icns` 作为图标。

关闭窗口或按 Cmd+Q 都会退出。Windows 与 Linux 直接运行 `target/debug/tterm-gpui`，没有应用包。

## 测试

```sh
cargo test -p tterm-gpui
```

测试用 `gpui-kit/test-support` 的无界面平台：不需要显示服务器，打开窗口后派发 `Increment` 动作、点击按钮，断言可见的 `点击次数 / Clicks` 文本从 0 变成 2。macOS 本机已通过；Linux 与 Windows 由 CI 的三平台任务运行（CI 未推送，按决策 006 记为未验证）。
