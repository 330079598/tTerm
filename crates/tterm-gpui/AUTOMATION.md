# GPUI 开发版自动化

接口必须同时满足 `automation` feature 和 `debug_assertions`。不带 feature 的正式构建，以及带 feature 的 release 构建，均不编译控制模块。

## 从头运行

在仓库根目录先构建，再运行脚本（Windows 需要 Python 3 和 PowerShell，Unix 需要 bash）：

```powershell
cargo build --workspace --bin tterm-gpui --features tterm-gpui/automation
python -u scripts/gpui-automation.py --output target/gpui-automation.png
```

脚本直接启动刚构建的 debug 二进制，显式选择无 profile 的 PowerShell 或 bash，等待原型获得激活与焦点，通过文本输入执行下面的命令，再发送 Enter，直到 `state` 确认真实 OSC 标题变更，然后截图并正常关闭原型。总超时默认 120 秒（可用 `--timeout` 改小）；成功、错误、超时均清理脚本自己启动的进程，并尽力删除令牌文件。

```powershell
$Host.UI.RawUI.WindowTitle = 'tTerm P0-23 OSC title'
```

```bash
printf '\033]0;tTerm P0-23 OSC title\007'
```

原型没有设置页，示例使用程序通过 OSC 0/2 设置的真实窗口标题作为可读回状态。没有自动化专用的标题开关。

## 手动连接

```powershell
target/debug/tterm-gpui.exe
```

直接运行哪个二进制决定了是否包含接口：请先用前述 feature 构建，再运行对应 debug 二进制。程序输出 `127.0.0.1:<port>` 和令牌文件路径。每次启动生成 256-bit 系统随机令牌，默认文件在 `%LOCALAPPDATA%/com.stone.tTerm.gpui-dev`，Unix 在 `XDG_STATE_HOME` 或临时目录的同名子目录，权限从创建起为 `0600`。Windows ACL 继承自当前用户的 `%LOCALAPPDATA%`：当前用户、SYSTEM、管理员可访问。`TTERM_GPUI_AUTOMATION_TOKEN_FILE` 可指定替代路径，调用者必须确保父目录只有当前用户可读。

每个 TCP 连接发一个 UTF-8 请求，然后关闭写方向（`shutdown(SHUT_WR)`），继续读取响应：

```text
token=<64 hex characters>
command=state
```

Python 调用示例（`request` 返回响应正文 bytes，错误响应抛异常）：

脚本文件名含连字符，交互客户端请用 `runpy` 加载：

```python
import runpy
import time
from pathlib import Path
request = runpy.run_path('scripts/gpui-automation.py')['request']
port = 12345  # 替换为当前启动日志中的端口。
token = Path(r'C:/Users/stone/AppData/Local/com.stone.tTerm.gpui-dev/automation-<id>.token').read_text().strip()
deadline = time.monotonic() + 30
print(request(port, token, 'windows', deadline).decode())
request(port, token, 'action=focus', deadline)
request(port, token, 'text=hello 中文', deadline)
request(port, token, 'key=ctrl-a', deadline)
Path('target/window.png').write_bytes(request(port, token, 'screenshot', deadline))
print(request(port, token, 'state', deadline).decode())
request(port, token, 'action=close', deadline)
```

响应头为 `OK <content-type> <byte-length>` 或 `ERR <byte-length>`，紧跟指定长度正文。命令：

| 命令 | 行为 |
| --- | --- |
| `windows` | 原型窗口 ID、焦点、激活状态。 |
| `state` | 最后实际传给窗口的 OSC 标题，格式 `title=...`。 |
| `key=<keystroke>` | GPUI 解析器接受的任意按键，含字母、修饰键及 `secondary-`；最多 64 字节，拒绝控制字符。动态解析入口为 `Keystroke::parse_with_os`。 |
| `text=<UTF-8>` | 逐字符派发到原型窗口输入处理器，最多 4096 字节，保留空格与 Unicode，拒绝控制字符。换行用单独的 `key=enter`。 |
| `action=focus` | 请求激活捕获的原型窗口，随后查询确认是否成功。 |
| `action=close` | 关闭捕获的原型窗口，触发正常清理。 |
| `screenshot` | `Window::render_to_image` 返回客户区 PNG，不含系统标题栏。 |

输入前检查捕获窗口的 GPUI Root 内容实体仍是原型，并检查窗口激活和焦点。不会向系统发送真实按键。不能把 `action=focus` 的成功响应当成已获得激活；Windows 可能拒绝激活，查询确认后再输入。

监听器只绑定 IPv4 回环，单线程串行处理连接；读写超时 5 秒，请求最多 64 KiB，超限或令牌不符直接拒绝，队列容量 1。令牌比较扫描固定长度的全部字节。没有 shell/程序执行 RPC，也没有任意文件路径或网络转发 RPC。持有令牌的当前用户可向已有终端打字，该用户本来就能运行 shell。

## 禁止清单与清理

不得自动触发文件打开/保存、目录选择、系统密钥串、代理配置、权限请求、外部链接确认等真实系统对话框；需要时标为 `[人工]`。不要读写 `com.stone.tTerm` 或 `com.stone.tTerm.dev` 数据。

窗口关闭和正常退出均尽力删除令牌，绑定失败和文件写入失败也清理。强制杀进程、崩溃或断电不能保证执行回调；脚本另有 finally 清理，手动运行者可删除自己留下的过期令牌。令牌每次启动不同，旧文件不授权新实例。
