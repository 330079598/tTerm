# P0-20 本地编辑器验证

独立示例只使用当前固定的 gpui-kit 0.9.0 `Editor` / `EditorState`。不启动终端，不连接远程主机，不加载应用配置，不修改主程序。测试数据仅在 `target/com.stone.tTerm.gpui-dev/P0-20` 下生成。没有复制第三方实现。

在仓库根目录运行：

```powershell
cargo test --workspace --example p0_20_remote_editor -- --nocapture
cargo build --workspace --example p0_20_remote_editor
target/debug/examples/p0_20_remote_editor.exe --local-check
```

第一条使用 GPUI 测试上下文，验证编辑、撤销重做、查找替换、中文组合协议、格式转换和大文件视口布局。第二条只运行本地文件读写实验。全套 `cargo test --workspace` 也会运行该示例的六个测试。

50 MiB 数据为 52,428,800 字节、51,200 条 1024 字节的 ASCII 行，运行时写入并在测试成功后删除。测试明确关闭自动折行，检查首部、中部、末尾和返回首部的可见行范围。数字是测试上下文的 CPU 布局时间，不是 GPU 帧时间、内存峰值或触控板流畅度；这个夹具也不能代表一个 50 MiB 的单行或数百万短行。

当前依赖没有 tree-sitter / syntect 高亮特性。七个语言样例全部没有高亮，这是测试记录的限制。不要为使测试变绿而把高亮判为通过。启用语言特性需要先审批新增的传递依赖并固定版本；本例没有这样做。

本地格式实验覆盖 UTF-8、UTF-8 BOM、UTF-16 LE/BE BOM 与 LF/CRLF/CR 的组合，拒绝未知编码、非法 UTF-16 和混合换行。无损解码、编辑器内统一 LF、编码回原格式是候选方案。字节比对能发现同尺寸的外部改写，但检查与写入间仍有竞态，也没有实现原子替换、权限保留或远程条件写入。这里只评估方案，不能直接用作生产保存代码。

以下为 `[人工]` 步骤，不由 agent 发送系统级输入：

```powershell
target/debug/examples/p0_20_remote_editor.exe rust
# 其他样例参数：css html javascript json markdown yaml
target/debug/examples/p0_20_remote_editor.exe large
```

1. Windows 分别使用微软拼音和微信输入法，在文首、行中、末尾输入中文和 emoji。记录候选框是否跟随光标；修改预编辑、取消预编辑、提交、撤销和重做；预编辑时切换窗口再返回。截图/录屏并记录输入法版本。macOS 和 Linux 在对应真实机器上重复，Linux 要列出 X11/Wayland 和输入法组合。
2. 在小文件内用 Ctrl+Z / Ctrl+Y（macOS Cmd+Z / Cmd+Shift+Z），Ctrl+F 和替换面板验证文本与焦点；记录主题、字号和行号栏的后续对齐差异。
3. `large` 窗口滚到中部和末尾再返回，记录启动时间、进程峰值内存、滚动延迟和是否能退出；另补单条超长行与大量短行的夹具。窗口模式会保留生成的文件供检查，不提交它。

如需由 agent 驱动真实窗口，必须使用 `crates/tterm-gpui/AUTOMATION.md` 开发控制接口。当前接口校验终端 `Prototype` 实体，未适配本独立示例，因此本卡没有驱动窗口、截图或绕开该校验。扩展接口应另经评估；六个测试不依赖此接口，也不替代真实输入法验收。
