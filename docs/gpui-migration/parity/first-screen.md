# 首屏、连接列表与 Terminal 设置的视觉取证

本页只描述取证工具和夹具，不改产品外观。它补上 `P3-05-first-screen.md`
要求的三类真实界面在深色与浅色下的成对截图：

| 屏幕 | GPUI | Tauri/WebKit 参照 |
| --- | --- | --- |
| 工作区标题栏 + 本地 Terminal | 真实原生窗口 | 生产 `TitleBarTabs`、`TitleBarActions` + 生产 xterm 表面 |
| 有数据的已保存 SSH 列表 | 真实原生窗口，生产 `SavedConnectionsUi` | 生产 `ProfilesPanel` 与 `Dialog` |
| 设置 / Terminal 页 | 真实原生窗口，生产 `SettingsPage` | 生产 `SettingsPanel` |

## 命令

```sh
# 两侧、两主题一次生成
node scripts/parity/first-screen.mjs

# 只生成一侧（写视图时用）
node scripts/parity/first-screen.mjs --skip-gpui
node scripts/parity/first-screen.mjs --skip-reference
```

产物固定在 `target/parity/first-screen/`：

```text
capture.json                 尺寸、2x、locale、命令、SHA-256、限制
README.md                    本次生成的可读说明
reference/<dark|light>/      三张 WebKit PNG 与 layout JSON
gpui/<dark|light>/           三张原生窗口 PNG、gpui.json、应用日志
```

单独复现一侧：

```sh
node scripts/parity/capture.mjs --theme dark \
  --screen workspace.first-screen \
  --screen profiles.list.first-screen \
  --screen settings.terminal.font \
  --out target/parity/first-screen/reference/dark

cargo run --locked -p tterm-gpui --example first_screen_capture \
  --features tterm-gpui/automation -- \
  --theme dark --output target/parity/first-screen/gpui/dark
```

## 固定条件

- 视口 800×600 逻辑像素，2× 像素密度，en-US，classic，100%。
- 工作区与连接列表的窗口客户区是 800×600。设置窗口是 800×636，截取
  标题栏以下的 800×600 页面，和参照端的页面裁剪一致。
- 两端使用同一份 `parity/fixtures/first-screen.profiles.json`：三个合成分组、
  四个合成 SSH 主机。主机都指向 `*.capture.invalid`，没有密码、密钥或真实
  地址。
- Terminal 输出来自 `parity/fixtures/first-screen.terminal.json`。原生端把它写成
  临时脚本的输出；参照端用生产的 xterm 表面写入同一串字节。没有 shell、
  PTY、SSH 或网络连接。
- 选中态：本地标签、列表第一行、Terminal 设置页。两端都由生产代码
  自行选择，不由截图脚本强设。

## 隔离

`first_screen_capture` 在 `target/parity/first-screen/gpui/<theme>/instances/`
下为每个窗口创建新的编号实例（`com.stone.tTerm.gpui-dev.check-<n>`），
并在子进程启动前把 `TMPDIR`/`TMP`/`TEMP` 指向该目录。实例里只有合成配置、
合成数据库和固定输出脚本；截图结束后删除实例。

参照端只启动 Vite 的 parity 模式和 Playwright WebKit。两端都不读取开发者
的配置、数据库、钥匙串、桌面或系统外观，也不建立 SSH 连接。

## 限制

- 原生端是真实窗口的客户区图像，不是桌面截屏；不会包含 macOS 交通灯、
  窗口阴影或原生边框。参照端是 Playwright WebKit，不是 Tauri WKWebView，
  所以窗口级模糊与原生材质也不在比较范围内。
- 已保存连接弹窗和设置页是两套真实实现，布局并不相同；工具不把它们强行
  套进同一套克隆组件。对比时要按功能对齐，不把已知布局差异当作取证失败。
- 只截选中态，不截悬停、键盘焦点移动或弹窗打开动画。
- 两端光标的闪烁相位没有同步；因此不要求 PNG 逐字节可复现，
  只要求布局、颜色和选中态一致。
- 两个 GPUI 窗口使用独立实例，因此设置页截图里的连接数据不会被打开；
  设置页本身不依赖那些连接。
- 工作区参照组合生产的 `TitleBarTabs`、`TitleBarActions` 与 xterm 表面；广播和
  传输工具在这个夹具里没有已填充状态，所以没有挂载它们的标题栏入口。
- 原生启动脚本目前写成 POSIX shell 脚本；Windows 需要等价的命令后才能直接
  使用这个入口。

## 参照夹具的 Terminal 设置

参照端的 `FirstScreenTerminal` 读取 `load_config` 解析后的字体族、字号、行高、字距、光标、回滚和 `terminal_renderer`，加载生产默认的 WebGL（失败回退 Canvas），并使用生产 `.terminal-surface` 的左右 6 px、下 4 px 内边距。两端使用同一份输出字节；
最后一行是提示符，不带末尾换行，且两端都把焦点放在 terminal 上。首屏 terminal 的剩余字体与表头差异见
[首屏复核报告](../reports/P3-05-first-screen.md)。
