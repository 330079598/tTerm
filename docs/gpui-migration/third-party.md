# 第三方代码登记

本文件登记从 Alacritty、nyaterm、gpui-kit 等仓库复制进 tTerm 的代码。规则见 [README 执行规则第 6 条](README.md#2-执行规则)，要点如下：

- 每个复制进来的文件在文件头注明来源仓库、来源提交、原路径、许可证与修改说明，并在本表登记。
- 不得复制 Zed 中 GPL 许可的 crate（`terminal`、`terminal_view`、`editor`、`workspace`、`ui` 等）的代码。终端部分按净室规则参考 Zed，见[技术选型第 5 节](tech-selection.md#5-终端)。
- 复制进来的代码同样受 AGENTS.md 的大小限制；超限文件在合回 `GPUI` 之前拆分，不加入基线。G1 之前的验证分支可以暂不拆分，见 P0-09。
- tTerm 使用 MIT 许可，复制进来的 Apache-2.0 文件仍属 Apache-2.0：保留原许可声明，在文件头注明修改，发行包中附带 Apache-2.0 全文与第三方声明（P4-25、P5-02）。
- 来源仓库有 `NOTICE` 时一并保留其内容。nyaterm 在固定提交上没有 `NOTICE`。

| 本仓库路径 | 来源仓库 | 来源提交 | 来源路径 | 许可证 | 改动说明 |
| ---------- | -------- | -------- | -------- | ------ | -------- |
| `crates/tterm-terminal-view/src/input/keyboard.rs` | github.com/alacritty/alacritty | `94e7c8874e526b1e67b349d9ba30ddf81669119e` | `alacritty/src/input/keyboard.rs` | Apache-2.0 | 把 winit 的 `KeyEvent` 改写为 GPUI 的 `Keystroke`；把 Alacritty 配置项改成 `EncoderConfig` 参数；补入默认绑定里的应用光标键、F1–F4、Shift+Tab、Backspace、小键盘 Enter，以及 xterm 的应用小键盘序列；Vi、搜索、提示与快捷键动作不移植。协议字节不变。 |
| `crates/tterm-terminal-view/src/input/keyboard/named.rs` | github.com/alacritty/alacritty | `94e7c8874e526b1e67b349d9ba30ddf81669119e` | `alacritty/src/input/keyboard.rs` | Apache-2.0 | 从同一文件拆出的命名键表与 Kitty 码；只改输入类型。 |
| `crates/tterm-terminal-view/src/input/keyboard/sequence.rs` | github.com/alacritty/alacritty | `94e7c8874e526b1e67b349d9ba30ddf81669119e` | `alacritty/src/input/keyboard.rs` | Apache-2.0 | 从同一文件拆出的 `build_sequence` 与修饰键编码；只改输入类型。 |

登记细节：

- Alacritty 在该提交没有 `NOTICE` 文件；仓库中的 `LICENSE-APACHE` 与 `LICENSE-MIT` 是许可文本，复制进来的文件仍按 Apache-2.0 使用，发行包附全文的要求见 P4-25、P5-02。
- 测试文件（`*/tests.rs`）是由本卡按行为新写的，不复制 Alacritty 的测试代码，因此不登记；每个期望值在测试里注明依据。
