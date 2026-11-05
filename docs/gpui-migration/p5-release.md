# P5 平台能力、打包、更新与发布

目标：GPUI 版可以安装、更新、回退，现有用户可以从 Tauri 版平滑升级，数据完整，三平台交互验收通过后再替换正式版本。

前提：P5-01 至 P5-03 只依赖 P1-08，可以在 G3 之后提前进行，P4-25 的更新界面需要 P5-03 的接口；P5-04 起的各卡需要 P4-27 的功能矩阵中没有“未开始”。

## 现状

- 打包目标：Windows NSIS，macOS `app` 与 `dmg`，Linux AppImage、deb、rpm（`src-tauri/tauri.conf.json`）。
- 更新：Tauri updater，清单为 `https://330079598.github.io/tTerm/update/stable/latest.json`，minisign 签名，公钥在 `tauri.conf.json` 中。Windows 使用 `passive` 安装模式。
- 更新渠道：正式版跟随 stable，带预发布标记的版本跟随 beta-dev（`src-tauri/src/config/update_channel.rs`）。
- 发布流程：`.github/workflows/release-main.yml` 与 `release-dev-beta.yml`，使用 `TAURI_SIGNING_PRIVATE_KEY` 签名更新包。工作流中没有 Apple 代码签名或公证的配置。
- 应用标识：`com.stone.tTerm`。

## P5-01 原生平台能力实现

- 依赖：P1-08
- 规模：L
- 参考：P1-08 的 trait；nyaterm `crates/nyaterm-app/src/single_instance.rs`、`url_protocols.rs`、`crash_report.rs`，nyaterm-desktop 使用的 `tray-icon`
- 工作：为 `tterm-platform` 的每个 trait 写原生实现：打开链接与文件；文件对话框；剪贴板，包括文件列表（对应 `src-tauri/src/clipboard_files.rs`）；系统通知（复用 `src-tauri/src/notification/` 中已有的平台代码）；托盘与托盘菜单；单实例与第二个实例传参；macOS 应用菜单；进程重启。
- 验收：每个能力在三平台手工验证，附结果表；单实例在第二次启动时把参数（例如启动目录）交给已运行的实例。
- 评估重点：原生实现与 Tauri 插件的行为差异，例如对话框的默认目录、通知点击的路由。

## P5-02 打包

- 依赖：P5-01
- 规模：L
- 参考：`src-tauri/tauri.conf.json` 及各平台配置、`src-tauri/icons/`（图标只能用 `pnpm icons:update` 重新生成）
- 工作：
  1. 选择打包工具（例如 cargo-packager），写决策记录。
  2. 产出与现有目标相同的包：NSIS、`app` 与 `dmg`、AppImage、deb、rpm。应用标识、安装路径、开始菜单与桌面项、文件关联、URL 协议与现有安装一致。
  3. 安装与卸载时的系统集成：右键菜单、Finder 服务、`PATH`（对应 `src-tauri/src/system_integration/`），以及现有 NSIS 安装钩子 `src-tauri/windows/hooks.nsh` 所做的处理。
  4. 必须随包分发的内容：
     - Windows：`conpty.dll`、`OpenConsole.exe` 与 `conpty-LICENSE.txt`（现由 `tauri.windows.conf.json` 的 `resources` 放在可执行文件旁）。
     - macOS：`Info.plist` 中的 `NSLocalNetworkUsageDescription`，缺少时系统会拦截到局域网主机的 SSH 与 SFTP 连接。
     - 全平台：第三方许可声明（见 P4-25 补充）。
  5. 构建目标与现有发布一致：macOS 同时构建 `aarch64-apple-darwin` 与 `x86_64-apple-darwin`；Linux 在 `ubuntu-24.04` 上构建，glibc 最低版本与现在相同，记录 GPUI 新增的系统库依赖并写进 deb 与 rpm 的依赖声明。
  6. Windows 代码签名与 macOS 签名、公证 `[人工：证书与账号]`。现有流程没有 Apple 签名，按 README 第 9 节的决定执行。
- 验收：在干净的系统上安装、启动、卸载；覆盖安装在现有 Tauri 版之上时，开始菜单、快捷方式与系统集成不重复、不残留。

## P5-03 更新器

- 依赖：P5-02
- 规模：L
- 参考：`src-tauri/src/updater.rs`、`src/lib/updater.ts`、`src/components/AppUpdateManager.tsx`、`config/update_channel.rs`
- 工作：
  1. 原生版实现更新器：读取同一格式的 `latest.json`，用同一个 minisign 公钥验证签名，下载、安装、重启。支持现有的更新渠道。
  2. **从 Tauri 版升级到 GPUI 版的路径。** 最后一个 Tauri 版本通过现有更新器安装 GPUI 版的安装包。逐平台确认：
     - Windows：NSIS 安装包覆盖安装后，旧的 WebView 版文件被清理。
     - macOS：`.app` 被原位替换，路径与权限不变。
     - Linux：AppImage 替换；deb 与 rpm 通过包管理器或下载提示。
  3. **两级升级，保证每个用户都先运行过渡版。** WebView 的 localStorage 只有 Tauri 版读得到，P1-10 的迁移必须在用户机器上由 Tauri 版执行一次。做法：
     - 发布一个“过渡版”：最后一个 Tauri 版本，包含 P1-10 的迁移，启动时把 localStorage 中的数据写入后端，并记录迁移完成。
     - 现有的更新地址（`stable/latest.json` 与 beta 渠道清单）从此永远指向过渡版，不再指向 GPUI 版。
     - 过渡版的更新器改读一个新地址的清单，GPUI 版只发布在新地址上。
     - 过渡版在迁移完成之前不提示更新到 GPUI 版。
     - GPUI 版启动时检查迁移记录；没有记录（例如用户手动下载安装、跳过了过渡版）时，提示用户先安装过渡版或接受这部分设置恢复默认，不静默丢弃。
  4. 发布工作流增加 GPUI 版的构建与签名任务，与 Tauri 版的产物分开存放，直到正式切换。
- 验收：
  - 在三平台上，从当前正式的 Tauri 版本经应用内更新升级到 GPUI 测试版，数据完整，再从 GPUI 版更新到更新的 GPUI 版。附完整录屏。
  - 从 P1-10 之前的 Tauri 版本出发（在 localStorage 中预先放入最近命令、快速连接、SFTP 列宽、界面风格等数据），经应用内更新必须先升到过渡版、再升到 GPUI 版，不能直接跳到 GPUI 版；到 GPUI 版后这些数据全部还在。三平台各一次，附录屏与升级前后的数据对照。
  - 跳过过渡版直接安装 GPUI 版时，出现第 3 步所述的提示，数据不被静默丢弃。
- 评估重点：更新失败时的回退；签名密钥不进入日志与产物。

## P5-04 数据兼容与回退演练

- 依赖：P5-03
- 规模：M
- 参考：迁移计划 6 节 P5 与 7.6 节；`src-tauri/src/backup/`、`db/migrations.rs`、`ssh/secret_store/`、`ssh/key_vault/`
- 工作：
  1. GPUI 版首次启动前自动创建可恢复的快照（使用现有备份模块）。
  2. 用真实的旧数据演练：主密码、系统钥匙串中的凭据、SSH 密钥、已知主机、主题、工作区、保存的终端内容、日志、同步。
  3. 回退：GPUI 版运行后再安装回 Tauri 版，确认数据仍可用。若 GPUI 版做过数据库迁移，验证用快照恢复的步骤。
  4. 两个客户端不同时写同一个数据库：GPUI 版启动时检测并提示正在运行的 Tauri 版。
- 验收：每类数据的演练结果表；回退步骤写进用户文档。
- 评估重点：钥匙串服务名与访问权限，特别是 macOS 上应用签名变化导致钥匙串反复询问的问题。

## P5-05 无障碍基线

- 依赖：P4-27
- 规模：M
- 参考：上游 GPUI 的 accesskit 支持；P0-07 关于分支是否包含它的结论
- 工作：确认屏幕阅读器（VoiceOver、Narrator、Orca）能读出主要界面：标签、按钮、设置项、对话框；终端内容的可访问性记录现状。
- 验收：三平台的结果表；与 Tauri 版的差距列出。

## P5-06 CI 与质量

- 依赖：P5-02
- 规模：M
- 工作：
  1. `check.yml` 中 GPUI 相关的测试在三平台运行，包括 P2-24 的终端回归套件。现有的 `sudo-mode.yml` 与 `real_shell_integration` 测试在代码移入 core 后继续运行，路径与命令随之更新。
  2. 性能冒烟：在 CI 中运行 P0-10 的绘制基准与吞吐负载，结果显著退化时失败或告警。
  3. 崩溃日志：panic 与崩溃时写入日志目录，“关于”页提供打开日志目录的入口。
- 验收：CI 在三平台通过；人为引入性能退化时冒烟检测能发现。

## P5-07 三平台交互验收 `[人工]`

- 依赖：P5-01 至 P5-06
- 规模：L
- 工作：按迁移计划第 9 节的场景表，在 W0-03 的全部环境中逐项验收，包括 Linux 输入法矩阵与无 GPU 环境。
- 产出：验收记录。
- 验收：迁移计划第 9 节的门槛全部满足，或每个未满足项都有人签字确认的处理方式。

## P5-08 灰度发布

- 依赖：P5-07
- 规模：M
- 工作：
  1. 先在 beta-dev 渠道发布 GPUI 版，正式渠道保持 Tauri 版。
  2. 发布说明写明已知差异、回退方法、反馈方式。
  3. 收集一段时间的反馈，阻塞性问题清零后进入 G4。
- 验收：至少一个完整的 beta 周期；问题列表与处理状态。

## G4 正式切换 `[人工]`

决定在正式渠道发布 GPUI 版。决定写入 `decisions/` 并附 P5-07 与 P5-08 的结论。

## P5-09 Tauri 版退役

- 依赖：G4 之后至少一个正式版本周期
- 规模：M
- 工作：
  1. 把 Tauri 版最后的源码归档到分支或标签。
  2. 删除 `src/` 前端、Tauri 适配层、相关的 npm 依赖与脚本，更新 `check.yml`、`AGENTS.md`、README 与 `scripts/check-file-size.mjs`。
  3. 在 `third-party.md` 中保留所有来源登记。
- 验收：删除后的仓库检查全部通过；文档中不再有指向已删除文件的链接。
- 不做：在 G4 之前删除任何 Tauri 版代码。

## P5-10 迁移收尾

- 依赖：P5-09
- 规模：S
- 工作：用 P0-01 的负载做最终性能对比；回顾实际工期与 P0-22 的估算；整理遗留问题清单。
- 产出：`reports/P5-10.md`。
