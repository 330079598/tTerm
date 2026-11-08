# GPUI 开发版自动化

这个入口只用于本地开发构建。它必须同时满足 `--features automation` 和
`debug_assertions`；普通 `cargo build --release` 不会编译监听器、令牌、动作或截图代码。

在仓库根目录执行：

```powershell
python scripts/gpui-automation.py --output target/gpui-automation.png
```

脚本启动 `cargo run -p tterm-gpui --features automation`，读取令牌文件，依次读取状态、切换标题状态、再次读取状态并保存 PNG。GPUI 原型目前只有终端视图，没有设置页，因此标题状态是任务卡要求的可切换且可读回状态的替代。

程序只绑定 `127.0.0.1`，每次启动生成新的 256-bit 系统随机令牌。令牌文件位于
`LOCALAPPDATA\com.stone.tTerm.gpui-dev`（或 `XDG_STATE_HOME`）下，Unix 权限为 `0600`；也可用 `TTERM_GPUI_AUTOMATION_TOKEN_FILE` 指定路径。请求格式为：

```text
token=<token>
command=state
```

支持 `windows`、`state`、`action=toggle-title`、受限的 `key=<...>` 和 `screenshot`。截图通过 `Window::render_to_image` 返回 PNG；按键只发给已激活的原型窗口。接口没有执行任意命令、任意文件读写、路径参数或网络转发能力，错误令牌会被拒绝。

## 禁止清单

自动化不得触发文件打开/保存、目录选择、系统密钥串、系统代理配置、权限请求或外部链接确认等真实系统对话框。需要这些流程时改为人工步骤并在报告中标为 `[人工]`。不要向 `com.stone.tTerm` 或 `com.stone.tTerm.dev` 写入数据。
