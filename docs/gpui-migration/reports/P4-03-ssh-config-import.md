# P4-03：SSH 配置导入（2026-11-26）

## 验收方案

入口为「SSH → 导入 SSH 配置」。以 Tauri 的 `SshConfigImportDialog` 为参照；两端应使用同一套 core 服务，而不是在 GPUI 再写一遍 OpenSSH 解析器。

| 用户操作                   | 验收标准                                                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 打开、选择文件或目录、刷新 | 能预览默认位置或指定文件；加载、空文件、不可读路径有反馈，失败后可重试。                                                                   |
| 检查主机                   | 可区分可导入、已有、带警告和跳过的条目；显示端点、认证、代理、转发及不支持的选项。Include 警告保留文件和行号，跳板机缺用户名的警告不丢失。 |
| 选择、取消、改路径         | 单选与全选只作用于可导入主机；零选择不写入，取消不创建会话。旧的异步响应不得替换新预览。                                                   |
| 确认导入                   | 仅导入已预览且选中的主机，保存到指定分组；可选覆盖、可选导入转发。完成后给出计数并刷新连接列表，不拨号。                                   |
| 与 Tauri 对照              | 两个隔离库使用同一份合成配置，对照预览、跳过和覆盖行为、落盘的主机与转发字段；不比较各库生成的 UUID。                                      |

只使用合成的 SSH 配置与临时数据库。功能、截图与硬件性能分开记录；此卡不决定 G2 或 G3。

## 本次结论与换机复测

本机定向冒烟通过：14 个 GPUI 窗口交互用例、1 个 GPUI 所用服务的隔离库用例、1 个经模拟 IPC 调用真实 Tauri 命令的隔离库用例。后两者读取同一份合成配置，对照预览、选择、重复导入及落盘字段，不是两套桌面客户端的人工验收。

来源可直接输入，也可用「选择 SSH 配置文件」或「选择 SSH 配置目录」打开系统选择器。取消选择保持当前路径与预览；关闭对话框后旧窗口迟到的选择不会修改新对话框；手填路径后需先点「预览」，确认只使用上一次预览成功的来源。另一台机器请用独立测试实例和合成配置复核：只勾一台主机导入，重新打开列表核对分组，再分别关闭、打开覆盖选项重复导入，核对计数和转发。整窗视觉、跨平台桌面、工作区全量 Rust 检查及 release 性能门本轮未验。

定向复验命令（仓库根目录）：

```sh
cargo test -p tterm-gpui --lib saved_connections::import
cargo test -p tterm-gpui --test p4_03_ssh_config_import
cargo test -p tTerm --lib p4_03_tauri_commands_preview
```

## 2026-11-27 来源选择器复核

「选择 SSH 配置文件」和「选择 SSH 配置目录」已经调用 GPUI 平台原生的 `prompt_for_paths`，不是应用内自绘的路径列表：macOS 走 `NSOpenPanel`，Windows 走系统文件选择器，Linux 走 XDG desktop portal。选择期间 `picker_open` 阻止重复打开；取消保持当前路径和预览；关闭或重开导入框后，旧选择器的迟到结果按对话框实例和预览代次丢弃。此前的「系统文件选择器尚未实现」来自选择器接入前的状态记录，不是当前代码的缺口。

本轮复跑：

```text
cargo test -p tterm-gpui --lib saved_connections::import
  15 passed; 0 failed

cargo test -p tterm-gpui --test p4_03_ssh_config_import
  1 passed; 0 failed

cargo fmt --all -- --check
  no output
```

新增用例 `a_superseded_preview_cannot_replace_the_newer_one` 覆盖同一导入框内两次预览的乱序返回：旧代次的预览不得替换新代次的结果。其余用例覆盖文件/目录两种选择器的 `files`、`directories`、`multiple`、`prompt` 标题参数、取消、旧对话框回调和手动预览解锁。`choosing_a_file_prompts_updates_the_source_and_previews` 与 `choosing_a_directory_prompts_updates_the_source_and_previews` 现在还会断言传给系统面板的标题等于当前语言的按钮文案；标题回归不依赖物理窗口。

仍未完成：在 macOS 与 Windows 的物理桌面点击系统窗口，核对标题、默认目录和取消行为；本轮测试只模拟系统对话框的返回值，不把它算作操作过系统窗口。Tauri 版同样没有文件类型过滤，GPUI 与之一致，因此 P4-03 不新增筛选项。整窗视觉、其余平台和 release 性能门不在本卡本轮范围内。

## 2026-11-27 桌面复验方案

### 自动覆盖到哪里

- 窗口测试已经覆盖：文件/目录两种提示的 `files`、`directories`、`multiple` 和标题参数；选择期间重复点击不打开第二个对话；取消保持当前路径和预览；关闭或重开导入框后旧回调不覆盖新对话框；两次预览乱序返回时旧代次不替换新代次。
- 隔离库测试已经覆盖：合成配置的预览、主机勾选、导入落盘、覆盖与转发选项，以及 Tauri 与 GPUI 读取同一份 fixture 的结果。
- `scripts/gpui-automation.py` 只提供 `focus`、`windows`、`state`、`text`、`key`、`screenshot`、`close` 和语言切换，没有鼠标点击或系统面板控制。它的 `screenshot` 只渲染 GPUI 窗口，不包含 macOS 的 `NSOpenPanel` 或 Windows 的文件对话框。因此系统窗口的标题、目录和取消手感只能物理验收，不能用现有脚本代替。

### 前置条件

- 使用专用测试账户或一次性工作树，不复用个人连接库。macOS 用独立用户；Windows 用专用本地账户，因为进程级 `HOME` 不能隔离资源管理器的快速访问和最近文件。
- 使用从未用过的 `TTERM_GPUI_CHECK_INSTANCE` 编号。实例把配置、数据库和日志放在临时根下的 `com.stone.tTerm.gpui-dev.check-<编号>/`，凭据也使用独立服务名。
- 只使用 `crates/tterm-core/tests/fixtures/ssh-config/p4-03/` 的合成配置，把三个占位符替换为临时路径。不要读取或修改个人 `~/.ssh/config`。
- 记录构建提交、操作系统版本、显示缩放、应用语言和系统文件面板的实际起始目录。

### macOS

在仓库根目录执行；`root` 同时作为临时 `TMPDIR` 和隔离 `HOME`，不要把编号改成已用过的值：

```sh
cargo build -p tterm-gpui --bin tterm-gpui || exit 1
root=$(mktemp -d /tmp/p4-03-desktop.XXXXXX) || exit 1
fixture=crates/tterm-core/tests/fixtures/ssh-config/p4-03
mkdir -p "$root/home/.ssh" || exit 1
sed -e "s|__P4_03_INCLUDE__|$root/home/.ssh/included.conf|g" \
    -e "s|__P4_03_KEY__|$root/home/.ssh/id_test|g" \
    -e "s|__P4_03_CERT__|$root/home/.ssh/id_test-cert.pub|g" \
    "$fixture/config" > "$root/home/.ssh/config" || exit 1
cp "$fixture/included.conf" "$root/home/.ssh/included.conf" || exit 1
export TMPDIR="$root" HOME="$root/home"
export TTERM_GPUI_CHECK_INSTANCE=1202
./target/debug/tterm-gpui
```

打开「SSH → 导入 SSH 配置」，逐项执行：

1. 初始预览应读取 `$HOME/.ssh/config`，列出 `focus`、`optional` 和被跳过的 `*.dropped`；`Include` 警告以 `$HOME/.ssh/included.conf` 和行 3 开头；`focus` 的跳板机缺用户名应有警告，不应静默省略。
2. 点「选择 SSH 配置文件」。核对 `NSOpenPanel` 标题等于当前语言的「选择 SSH 配置文件 / Choose SSH config file」，且没有扩展名过滤；记录实际起始目录。按取消，路径和预览保持不变。
3. 再次打开并选择 `$HOME/.ssh/config`。来源框应更新为该绝对路径，预览刷新；选择期间不能打开第二个面板。
4. 点「选择 SSH 配置目录」。核对标题等于「选择 SSH 配置目录 / Choose SSH config folder」，且只能选目录。先取消，确认状态不变；再选择 `$HOME/.ssh`，预览应解析到其中的 `config`。
5. 只勾选 `focus`，导入到新分组 `p4-03-test`，关闭转发选项；确认计数为 1。重新打开连接列表，只新增 `focus`，且没有拨号、没有新标签。
6. 再导入同一文件：关闭覆盖时报告已有/跳过；打开覆盖后更新；分别打开和关闭转发，核对隧道列表按选项增减。取消导入不写库。
7. 关闭导入框并重新打开，确认上一个面板的迟到结果不会改变新来源；这一项同时有窗口测试覆盖，物理复验只需确认没有旧值闪回。

### Windows

在专用测试账户的 PowerShell 中执行：

```powershell
cargo build -p 'tterm-gpui' --bin 'tterm-gpui'
if ($LASTEXITCODE -ne 0) { throw '构建失败。' }
$root = Join-Path $env:TEMP ("p4-03-desktop-" + [guid]::NewGuid())
$fixture = Join-Path (Get-Location) 'crates\tterm-core\tests\fixtures\ssh-config\p4-03'
$ssh = Join-Path $env:USERPROFILE '.ssh'
if (Test-Path (Join-Path $ssh 'config')) { throw '测试账户已有 ~/.ssh/config；请换专用账户，或跳过默认预览只用显式选择。' }
New-Item -ItemType Directory -Path $ssh -Force | Out-Null
$include = Join-Path $ssh 'included.conf'
$config = Get-Content -Raw (Join-Path $fixture 'config')
$config = $config.Replace('__P4_03_INCLUDE__', ($include -replace '\\', '/'))
$config = $config.Replace('__P4_03_KEY__', ((Join-Path $ssh 'id_test') -replace '\\', '/'))
$config = $config.Replace('__P4_03_CERT__', ((Join-Path $ssh 'id_test-cert.pub') -replace '\\', '/'))
[IO.File]::WriteAllText((Join-Path $ssh 'config'), $config, [Text.UTF8Encoding]::new($false))
Copy-Item (Join-Path $fixture 'included.conf') $include
$env:TEMP = $root
$env:TMP = $root
$env:TTERM_GPUI_CHECK_INSTANCE = '1202'
& .\target\debug\tterm-gpui.exe
```

步骤与 macOS 相同，另外核对：

- 系统文件对话框的标题是「选择 SSH 配置文件 / Choose SSH config file」或「选择 SSH 配置目录 / Choose SSH config folder」，不是应用内自绘列表。
- GPUI 与 Tauri 一样不按扩展名过滤；如果以后加过滤，两端要同时改并重新验收。
- 取消后来源和预览不变；选择文件和目录分别只返回一个路径，不创建连接。
- 导入后的主机与转发字段用连接列表和隧道列表核对；若本机装有 `sqlite3`，可用只读查询复核：

```powershell
$db = Join-Path $env:TEMP "com.stone.tTerm.gpui-dev.check-1202\config\tterm.db"
sqlite3 -readonly $db "select name, group_name from profiles order by position;"
sqlite3 -readonly $db "select data from tunnels order by position;"
```

### 证据清单

保存到 `docs/gpui-migration/reports/assets/P4-03-desktop/<platform>/`，文件名带序号：

- 系统面板必须用操作系统截图（macOS `screencapture`、Windows 截图工具），不能用 `scripts/gpui-automation.py` 的 `screenshot`，后者只含 GPUI 窗口。
- 系统文件面板标题与选择状态截图：`01-choose-file.png`、`02-choose-directory.png`。
- 取消前后的来源与预览截图：`03-cancel-keeps-source.png`。
- 导入预览、导入结果和重新打开后的连接列表：`04-preview.png`、`05-imported-list.png`。
- 覆盖与转发四种组合的结果：`06-overwrite-off.png`、`07-overwrite-on-forwards-on.png`。
- 只读数据库查询输出：`db-profiles.txt`、`db-tunnels.txt`（可选）。
- 环境记录：`environment.txt`，写入 `git rev-parse HEAD`、OS 版本、显示缩放、语言、面板实际起始目录和操作人。

### 通过规则

- 标题、文件/目录模式、取消保持状态和“选择后预览更新”必须与 Tauri 一致；任一项不同即不通过。
- 默认起始目录由系统决定，Tauri 也没有传 `defaultPath`；记录实际值即可，只有 GPUI 与 Tauri 行为不同才判失败。
- 只要没有在物理窗口点击并取消/选择过，报告只能写「待物理复验」，不能写「通过」。
