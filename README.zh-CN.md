<p align="center">
  <img src="./app-icon.png" width="112" alt="tTerm 图标">
</p>

<h1 align="center">tTerm</h1>

<p align="center">
  <strong>免费开源的 SSH 终端，内置 SFTP。</strong><br>
  Xshell、SecureCRT、Termius 的轻量替代，支持 macOS、Windows 和 Linux。
</p>

<p align="center">
  <a href="https://github.com/330079598/tTerm/releases/latest"><img src="https://img.shields.io/github/v/release/330079598/tTerm?label=release" alt="最新版本"></a>
  <a href="https://github.com/330079598/tTerm/releases"><img src="https://img.shields.io/github/downloads/330079598/tTerm/total" alt="下载量"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue" alt="支持平台">
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/330079598/tTerm" alt="许可证"></a>
  <a href="https://github.com/330079598/tTerm/stargazers"><img src="https://img.shields.io/github/stars/330079598/tTerm?style=social" alt="GitHub Star"></a>
</p>

<p align="center">
  <a href="https://github.com/330079598/tTerm/releases/latest"><strong>下载</strong></a> ·
  <a href="#安装">安装</a> ·
  <a href="#功能">功能</a> ·
  <a href="#常见问题">常见问题</a> ·
  <a href="./README.md">English</a>
</p>

![tTerm：三个分屏 SSH 会话同时实时广播输入](./docs/screenshot/main-overview.png)

## 为什么选择 tTerm？

- **AI 编程助手在等你，一眼就知道。** 在多个标签里同时跑 Claude Code、Codex、OpenCode 或 pi，每个标签都会显示助手是在工作还是在等你回复；需要你处理或完成时会发通知，SSH 远程同样可用。
- **免费、开源、无需注册。** MIT 许可证，没有账号、没有订阅，也没有付费功能。
- **数据在你自己手里。** tTerm 没有自己的云服务。保存的密码在本地用 AES-256-GCM 加密；多设备同步走你自己的 WebDAV（坚果云、Nextcloud、群晖等），上传前先加密。
- **终端和文件在同一个窗口。** 任意 SSH 会话都能打开 SFTP 面板；传输多通道并行、断点续传，还能限速。
- **和 Xshell、SecureCRT 一样用 `rz` / `sz`。** 本地终端和 SSH 会话都支持 ZMODEM，网络设备、嵌入式板子、只给 Shell 的堡垒机都能传文件。
- **为多服务器而生。** 分屏、多终端广播输入、带变量的命令库、可视化端口转发，以及服务器端无需安装任何东西的监控栏。

## 安装

从 [最新版本](https://github.com/330079598/tTerm/releases/latest) 下载对应系统的安装包：

| 系统                            | 文件                          |
| ------------------------------- | ----------------------------- |
| macOS，Apple 芯片（M1 及更新）  | `tTerm_x.y.z_aarch64.dmg`     |
| macOS，Intel 芯片               | `tTerm_x.y.z_x64.dmg`         |
| Windows                         | `tTerm_x.y.z_x64-setup.exe`   |
| Linux                           | `.AppImage`、`.deb` 或 `.rpm` |

安装后 tTerm 可以在应用内自动更新，提供 **Stable** 和 **Beta Dev** 两个通道。

### macOS：首次打开提示“tTerm 已损坏，无法打开”

文件并没有损坏。tTerm 是开源项目，没有付费的 Apple 开发者账号，安装包没有经过 Apple 公证，而 macOS 会拦截所有从浏览器下载、未经公证的应用。

1. 把 `tTerm.app` 拖进 **应用程序** 文件夹。
2. 打开 **终端**，执行：

   ```bash
   xattr -dr com.apple.quarantine /Applications/tTerm.app
   ```

3. 正常打开 tTerm 即可。

这一步只需要做一次。通过 tTerm 应用内更新安装的新版本不会被拦截；如果你是从浏览器重新下载 `.dmg` 来升级，需要再执行一次上面的命令。

### macOS：每次升级后钥匙串都要求输入密码

默认情况下，tTerm 把解锁已保存密码的密钥放在 macOS 钥匙串中。由于没有 Apple 开发者账号，每个新版本在钥匙串看来都是一个不同的应用，所以 **每次升级后** macOS 都会弹出一次对话框，说 tTerm 想要使用钥匙串中名为 `tterm` 的机密信息。

输入你的 **Mac 登录密码**，然后点击 **始终允许**。在下一次升级之前都不会再问。

- 如果点了 **拒绝**，已保存的密码会保持锁定，可以在 **设置 → 安全** 中重试。
- 如果不想再看到钥匙串提示，可以在 **设置 → 安全 → 解锁方式** 中改为 **主密码**。这样 tTerm 不再往钥匙串里存任何东西，改为向你询问主密码。

## 功能

<details>
<summary><strong>工作区与标签</strong>：多标签、分屏、会话恢复</summary>

- 多标签会话，支持拖拽排序、复制、重命名、关闭其他 / 左侧 / 右侧标签
- 分屏：任意标签可向左、右、上、下分屏，标签可在分组间拖动，分组可最大化
- 标签很多时可搜索和总览；标签宽度支持自适应或固定
- 重启后恢复会话和布局，可选择启动时只连接当前标签
- 关闭有传输任务的标签、或在隧道运行时退出应用前会二次确认

</details>

<details>
<summary><strong>终端</strong>：Shell 集成、自定义快捷键、会话日志</summary>

- 本地 Shell 自动检测，也可手动选择（Windows 下支持 cmd、PowerShell、PowerShell 7、WSL、Git Bash 或自定义程序）
- 终端内搜索，实时高亮匹配结果
- **命令标记**（Shell 集成，OSC 133）：按 `Ctrl/Cmd+↑/↓` 在命令之间跳转，一键复制上一条命令的输出，滚动条上标出成功和失败的命令。Windows 本地 shell 和使用 bash、zsh、fish 的 SSH 会话自动启用（每个连接可单独关闭）；其他服务器可在设置中复制脚本，加到 `~/.bashrc`、`~/.zshrc` 或 fish 的 `config.fish`
- WebGL / Canvas 渲染器可切换，回滚行数可配置（默认 10,000 行）
- Web 链接识别，一键清空终端历史
- 快捷键可完全自定义，带冲突检测；绑定会遮挡常用 Shell 按键（如 `Ctrl+A`、`Ctrl+R`）时会提示
- macOS 下可将 Option 键用作 Meta 键（shell、Emacs、tmux 的 Alt 快捷键）
- 可选会话日志：原始记录和/或纯文本、文件名模板、按大小分卷、关闭后 gzip 压缩

</details>

<details>
<summary><strong>终端广播</strong>：输入一次，多台服务器同时执行</summary>

在多台服务器上同时执行同样的操作（`Cmd/Ctrl+Shift+B`）。页首截图展示的就是在三个分屏窗格中实时广播输入。

- **命令模式**：输入一次命令，发送到选中的多个终端；多行内容发送前会确认
- **实时模式**：主终端的按键和粘贴实时同步到所有目标终端
- 显示每个目标的发送状态，发送前可先重连已断开的目标
- 检测到密码提示时实时广播会自动停止，密码只留在源终端

</details>

<details>
<summary><strong>AI 编程助手</strong>：Claude Code、Codex、OpenCode 和 pi</summary>

同时跑好几个助手？tTerm 会标出哪个正在工作、哪个在等你确认，不用挨个标签点进去看。

- **一键接入**：在「设置 > 通知」中安装。Claude Code 和 Codex 会合并 hooks 到其配置文件，OpenCode 和 pi 安装一个小插件；其他设置保持不变，原文件备份为 `.tterm-bak`，安装前可点「查看写入内容」预览改动
- **标签标记**：助手工作时显示动画，停下来请求权限或输入时换成另一种标记
- **通知提醒**：助手等待超过几秒、完成本轮或因出错停止时发出通知；自动模式下立即通过的请求不会打扰你
- **SSH 远程同样可用**：助手通过终端转义序列（OSC 777）上报状态，把同样的 hooks 复制到远程主机上的配置里即可。在 tmux 中使用需要开启 `set -g allow-passthrough on`
- 可在「设置 > 外观」中选择标记样式（七种工作动画、三种等待标记）和颜色

</details>

<details>
<summary><strong>命令库</strong>：保存常用命令，支持变量</summary>

![command-library](./docs/screenshot/command-library.png)

- 保存命令，支持名称、描述、标签和收藏（`Cmd/Ctrl+Shift+P` 打开）
- 命令可作用于所有连接，或只属于某个已保存的连接
- 自动记录最近执行过的命令
- 把命令插入当前终端（不会自动执行），或把终端中选中的文本保存为新命令
- **命令变量**：在命令里写 `{{host}}` 这样的占位符，插入时会弹出表单询问它的值。支持文本、数字、选项和密文四种类型，可设置默认值和是否必填
- 可为危险命令开启“插入前确认”，插入前先显示完整命令
- 标签管理：新建、重命名、删除

</details>

<details>
<summary><strong>SSH 连接管理</strong>：密钥、Agent、跳板机、MFA、sudo 自动填充</summary>

![saved-connections](./docs/screenshot/saved-connections.png)

- 密码、私钥（支持密码短语）或 SSH Agent 认证，Agent 中加载的硬件密钥（YubiKey、FIDO2）同样可用
- **键盘交互认证与多步登录**：服务器要求动态口令、MFA 验证码或“密钥 + 验证码”这类多步认证时，会在标签内弹出提示逐项询问；已保存的密码会自动回答服务器的密码提问。也可以选择“键盘交互”方式，不保存任何凭据
- 可按连接开启 SSH Agent 转发
- 跳板机链，按顺序逐级连接，与 OpenSSH `ProxyJump` 一致
- HTTP、SOCKS5 代理和 `ProxyCommand`
- 从 `~/.ssh/config` 导入主机，导入前可预览，`LocalForward` / `RemoteForward` / `DynamicForward` 规则可一并导入为隧道
- 连接分组（支持拖拽）、搜索、批量删除
- 主机密钥确认；已信任的主机可在“设置 → 安全”中查看和删除
- 断线自动重连，退避间隔有上限，重试次数可配置
- 每个连接可单独设置心跳间隔和判定断线的无响应次数
- 每个标签独立的连接信息栏，可固定显示
- **sudo 密码自动填充**：`sudo` 或 `doas` 询问密码时，终端外会出现提示条，点击或按 `Cmd/Ctrl+Shift+Enter` 即可填入已保存的密码。可识别中文、英文及其他语言环境的提示，也可添加自定义正则。每个连接可单独保存 sudo 密码，适合密钥或 Agent 登录的场景。密码被拒绝后本会话内会暂停自动填充。

</details>

<details>
<summary><strong>服务器监控</strong>：CPU、内存、网络、磁盘、延迟</summary>

- 每个 SSH 会话下方的精简状态栏：CPU、内存、网络吞吐、磁盘、负载、运行时长、服务器 IP、SSH 延迟，显示哪些指标及顺序均可自定义
- 可展开的详情面板，包含概览、CPU、内存、网络、磁盘分页和近期趋势
- 指标通过现有 SSH 连接从 `/proc` 读取，服务器端无需安装任何东西；仅支持 Linux 主机
- 刷新间隔 1–60 秒可调

</details>

<details>
<summary><strong>SFTP 文件管理</strong>：浏览、在线编辑、改权限、拖拽上传</summary>

![sftp-browser](./docs/screenshot/sftp-browser.png)

- 浏览远程目录，可直接输入路径跳转，列宽可调整，显示所有者/用户组与权限
- 点击列头按名称、修改日期、大小、类型、权限或所有者排序；可隐藏以 `.` 开头的文件
- 按文本、通配符或正则表达式筛选当前目录
- 新建文件夹、重命名、删除；批量删除前可预览，超大目录可改用经确认的远程命令删除
- 修改权限（chmod）：勾选读/写/执行或直接输入八进制，可一次应用到多个选中项
- 上传、下载文件和文件夹，支持从桌面拖拽上传，或在面板中 `Ctrl+V` / `Cmd+V` 粘贴已复制的本地文件
- **远程文件编辑器**：在内置编辑器中打开远程文件，支持语法高亮，保存后直接写回服务器
- 复制远程文件完整路径

</details>

<details>
<summary><strong>文件传输</strong>：多通道并行、断点续传、限速、ZMODEM</summary>

- 每个传输任务使用多个 SFTP 通道并行（1–16，默认 4）
- 中断的传输可从断点继续或重试
- 目标文件已存在时的处理策略：仅在更新时覆盖、全部覆盖、全部跳过、保留两者
- 上传、下载可分别限速，对正在进行的传输同样生效
- 每个任务显示进度、速度和状态，批量/文件夹任务可展开，支持历史记录
- **ZMODEM（`rz` / `sz`）**：无法使用 SFTP 时，直接在终端里用 `rz` / `sz` 传文件，行为与 SecureCRT / Xshell 一致。本地终端和 SSH 会话都能自动识别，也可以从右键菜单手动触发，任务与 SFTP 统一显示。详见 [docs/zmodem.zh-CN.md](./docs/zmodem.zh-CN.md)。

</details>

<details>
<summary><strong>端口转发</strong>：本地、远程和 SOCKS5 隧道</summary>

![port-forwarding](./docs/screenshot/port-forwarding.png)

- 基于已保存的 SSH 连接（包括跳板机）创建本地、远程、动态（SOCKS5）转发
- 指向同一主机的隧道共用一条 SSH 连接
- 实时显示状态、活动/累计连接数和收发流量
- 断线自动重连，隧道断开或恢复时会通知
- 可设置 tTerm 启动时自动开启（需要输入密码的隧道会跳过）
- 显示等价的 OpenSSH 命令；监听地址可被局域网访问时会提示
- 一键复制监听地址或在浏览器中打开

</details>

<details>
<summary><strong>个性化</strong>：近 700 个主题、字体、窗口模糊</summary>

![theme-editor](./docs/screenshot/theme-editor.png)

- 内置主题，以及支持实时预览的自定义主题编辑器
- 近 700 个主题的主题库（与 Ghostty 自带的主题相同）：可搜索，用方向键浏览时整个应用实时预览，可收藏，任一主题都可复制到编辑器中修改
- 可跟随系统明暗，在你选定的浅色主题和深色主题之间自动切换
- 支持导入 Ghostty、Windows Terminal、iTerm2（`.itermcolors`）、Alacritty 和 kitty 的主题，并根据终端色板自动生成相配的界面颜色
- 字体设置支持选择系统字体，可选光标样式
- 界面文字缩放 80%–200%
- 窗口背景模糊（macOS、Windows）：模糊程度和不透明度可调，Windows 可选亚克力或云母材质
- 中英文界面，自动识别系统语言

</details>

<details>
<summary><strong>安全、备份与同步</strong>：本地加密、WebDAV</summary>

- **加密保存密码**：SSH、跳板机和 sudo 密码使用 AES-256-GCM 加密存放在 tTerm 本地数据库中，密钥的解锁方式可选：
  - **系统凭据存储**（macOS 钥匙串、Windows 凭据管理器、Linux Secret Service）
  - **主密码**（Argon2id），可选择启动时弹出解锁
  - **不保存密码**：只在当前会话内使用
- 可设置恢复密码，防止系统凭据存储中的密钥丢失
- **备份与迁移**：按需选择导出的数据（设置、连接与隧道、工作区、已知主机、命令、主题、日志）。密码只会包含在设置了备份密码的加密备份中（Argon2id + AES-256-GCM）。私钥文件不会进入备份，导入后会列出本机缺少的私钥。导入前可预览，支持合并或替换，并自动创建恢复快照。
- 定时本地备份（每天或每周），可设置保留份数；定时备份不包含密码
- **WebDAV 远程备份**：把加密备份上传到坚果云、Nextcloud、群晖、AList 等 WebDAV 服务，可在任意设备上下载恢复
- **多设备同步**：使用同一个 WebDAV 目录的设备会自动合并连接、密码、命令、已知主机、设置和主题的改动；同步文件在上传前加密，同步哪些数据可自行选择

</details>

## 截图

| | |
| --- | --- |
| ![已保存的连接](./docs/screenshot/saved-connections.png) | ![SFTP 文件管理](./docs/screenshot/sftp-browser.png) |
| ![端口转发](./docs/screenshot/port-forwarding.png) | ![命令库](./docs/screenshot/command-library.png) |

## 常见问题

**tTerm 真的免费吗？**
是的。tTerm 使用 MIT 许可证，没有付费版本、无需注册账号，也不收集使用数据。

**我的数据存在哪里？**
存在你自己的电脑上，tTerm 的数据目录：macOS 是 `~/Library/Application Support/com.stone.tTerm`，Windows 是 `%APPDATA%\com.stone.tTerm`，Linux 是 `~/.config/com.stone.tTerm`。除非你自己配置了 WebDAV 备份或同步，数据不会离开本机。

**能导入已有的主机吗？**
可以从 `~/.ssh/config` 导入，包括 `ProxyJump` 和端口转发规则。换电脑时也可以用加密备份文件或 WebDAV 整体迁移。

**为什么 macOS 提示应用已损坏，或者升级后要输入钥匙串密码？**
见 [安装](#安装)。两者都是因为 tTerm 没有付费的 Apple 开发者账号，并不代表应用有问题。

**服务器监控需要在服务器上装东西吗？**
不需要。它通过已有的 SSH 连接读取 `/proc`，支持 Linux 主机。

## 参与贡献

欢迎提交 Issue 和 Pull Request。如何构建 tTerm、代码如何组织、提交前需要通过哪些检查，请看 [CONTRIBUTING.md](./CONTRIBUTING.md)。

如果 tTerm 对你有帮助，欢迎在 GitHub 上点个 ⭐，让更多人看到它。

## 许可证

[MIT](./LICENSE)

主题库来自 [iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes)（MIT，见 `src/assets/themes/LICENSE`），各主题版权归原作者所有，署名见 `src/assets/themes/CREDITS.md`。
