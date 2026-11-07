# GPUI 迁移技术选型

> 日期：2026-11-05。前提：已决定正式迁移（[决策 001](decisions/001-formal-migration.md)），界面目标为与现有 Tauri 版一致。
> 本文回答“用什么、包怎么改”。界面还原的方法与边界见 [ui-parity.md](ui-parity.md)，验收规则见 [acceptance.md](acceptance.md)。
> 外部事实均在 2026-11-05 查询，第 8 节列出来源。标为“待核实”的项由对应任务卡确认，不能当作已知。

## 1. 结论

| 层         | 选型                                                                                                                                                                                            | 备选与放弃理由                                                                                                                                                   |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 仓库形态   | **同一仓库，重写界面，共享 Rust 核心**                                                                                                                                                          | 新建仓库：迁移期后端要两边维护，见第 2 节                                                                                                                        |
| GPUI 来源  | **官方 Zed 代码的 crates.io 快照**：`gpui-pre =0.3.11`（`zed@f816e1f`），经 `gpui-kit 0.9.0` 使用                                                                                               | nyakang 的分支组合（个人维护，2026-11-05 决定不用，见[决策 007](decisions/007-official-sources-and-terminal-route.md)）；上游 Zed git；gpui-ce，见第 3 节        |
| 组件库     | **gpui-kit 0.9.0**（crates.io 官方发布）：行为层用 `gpui-base`，外观由 tTerm 自己的 `tterm-ui` 决定                                                                                             | adabraka-ui、guise 等规模小；Zed 的 `ui` 是 GPL                                                                                                                  |
| 终端       | **官方 `alacritty_terminal 0.26.0` 为内核**；模型与绘制元素自写，架构参考 Zed 的做法；输入协议、方框字符、链接识别移植自 Alacritty 应用层；关键词高亮等独立模块从 nyaterm 挑选，见第 5 节 | 整体复用 nyaterm 终端 crate：需要给 Alacritty 带 2 个补丁，且代码来自一个 6 个月的个人项目；Zed `terminal` 是 GPL；`gpui-terminal`、`gpui_xterm` 只是 0.1 的雏形 |
| 远程编辑器 | **gpui-kit 的代码编辑器**（tree-sitter、ropey）                                                                                                                                                 | 自研代价过高；Zed `editor` 是 GPL                                                                                                                                |
| 图标       | **与 lucide-react 同版本的 Lucide SVG**，由脚本从 npm 包导出                                                                                                                                    | gpui-kit-assets 自带 Lucide，但版本不一定与现有一致                                                                                                              |
| 国际化     | **自写 `tterm-i18n`，直接读现有 i18next JSON**                                                                                                                                                  | rust-i18n 的插值与复数语法不同，需要转换文案，两个客户端就不再共用一份文件                                                                                       |
| 打包       | **cargo-packager**（app、dmg、NSIS、deb、AppImage）+ **cargo-generate-rpm**                                                                                                                     | 继续用 tauri-bundler：要伪造 Tauri 工程配置；cargo-packager 没有 RPM 输出                                                                                        |
| 更新       | **沿用现有 `latest.json` 与 minisign 公钥**，优先 cargo-packager-updater，不兼容则自写                                                                                                          | gpui-updater 0.4：格式与签名方式待核实                                                                                                                           |
| 界面测试   | GPUI `TestAppContext`（行为）+ `VisualTestAppContext`（macOS 截图）+ 自建对比工具                                                                                                               | 只靠人工截图无法复现，也无法由评估者判定                                                                                                                         |

一句话：**所有依赖都来自官方发布，不需要 fork 任何仓库；终端在官方 Alacritty 之上自己搭，组件用 gpui-kit；真正的工作量在把 8.9 万行界面按原样重画一遍，以及平台层的输入法、无 GPU 和更新衔接。**

## 2. 仓库形态：不新建仓库

可以新起一个项目，但不建议放在新仓库。建议的做法是：**在本仓库里从零写 GPUI 界面，不逐行翻译 React；Rust 业务核心两边共用。**

| 方案                          | 迁移期后端                           | 数据兼容                 | 结论     |
| ----------------------------- | ------------------------------------ | ------------------------ | -------- |
| 新仓库，复制后端              | 两份代码各自演进，修一个 bug 改两遍  | 两份迁移脚本，容易不一致 | 不采用   |
| 新仓库，git 依赖本仓库的 core | 每次改 core 都要跨仓库提交和升级 rev | 可以                     | 不采用   |
| **本仓库，新 crate**          | 一份 core，Tauri 与 GPUI 同时调用    | 同一套迁移与测试         | **采用** |

迁移期间 Tauri 版还要发布 6 个月以上，期间的修复和新功能只写一次是决定性因素。AGENTS.md 的检查、大小限制和提交规则对新代码同样有效，也不需要再配一套。

### 2.1 Workspace 布局（修订 README 约定 A1）

原约定把 workspace 根放在 `src-tauri/`。正式迁移后，最终产品是 GPUI 版，`src-tauri/` 会在 P5-09 删除，所以 **workspace 根改到仓库根目录**：

```text
Cargo.toml                 # [workspace]，成员见下；[workspace.lints] 与 [workspace.dependencies]
rust-toolchain.toml        # 固定工具链，CI 读取它
crates/
  tterm-core/              # 会话、认证、SFTP、隧道、存储、备份、同步（P1 从 src-tauri/src 迁入）
  tterm-platform/          # 路径、凭据、剪贴板、通知、托盘、单实例等能力 trait 与各平台实现
  tterm-terminal/          # 终端模型：封装官方 alacritty_terminal，不依赖 GPUI
  tterm-terminal-view/     # 终端 GPUI Element：绘制、滚动、选区、输入法
  tterm-i18n/              # 读取 src/i18n/locales/*.json（P5 后移到 crates/tterm-i18n/locales/）
  tterm-ui/                # 设计令牌、图标、基础组件，对应 src/components/ui
  tterm-gpui/              # 应用本体：窗口、工作区、各功能视图；开发期二进制名 tterm-gpui，P5 改为 tTerm
  tterm-parity/            # 开发用：界面对齐的截图与布局对比工具，不进发行包
src-tauri/                 # 现有 Tauri 包，作为 workspace 成员保留到 P5-09
src/                       # 现有 React 前端，保留到 P5-09
```

拆这么多 crate 的主要原因是**编译时间**。GPUI 及其依赖全量编译约十分钟（[验证报告](validation-2026-11-05.md) 5.1、5.2 节），界面代码放在 `tterm-gpui` 一个 crate 里，改一行样式就要重编整个应用。终端绘制、基础组件、i18n 各自成 crate 后，改功能视图时它们不用重编。

这一改动带来的连锁修改，并入 W0-01：

- `target/` 移到仓库根。`release-dev-beta.yml` 第 175–176 行引用了 `src-tauri/target`，需要改；Tauri CLI 通过 `cargo metadata` 找 target 目录，本身支持 workspace，但打包流程要在三平台 CI 和一次 dev-beta 发布中实测。
- AGENTS.md 的“所有 cargo 命令从 `src-tauri/` 执行”改为“从仓库根执行，加 `--workspace`”。
- `scripts/check-file-size.mjs` 的扫描范围加上 `crates/`。
- `Cargo.lock` 移到根目录。用 `git mv` 单独提交，确认锁定的版本没有变化。
- 根 `Cargo.toml` 显式写 `resolver = "2"`。虚拟 workspace 默认用解析器 1，会让现有依赖的解析结果与 `src-tauri` 单独构建时不同；新 crate 虽是 2024 edition，解析器由 workspace 统一决定。是否改为 `"3"`（按 MSRV 解析）由 P0-06 决定。[决策 002](decisions/002-dependency-sources.md) 已改为 `"3"`：在固定工具链上两者解析结果相同，改后能挡住以后 `cargo update` 拉进 MSRV 更高的依赖。
- `.cargo/config.toml` 随之移到根目录，macOS 的开发 runner 从此对整个 workspace 生效；它只包装名为 `tTerm` 的二进制，所以 GPUI 版开发期必须用别的名字（见 P0-08）。

## 3. GPUI 来源

2026-11-05 决定：**只用官方来源，不依赖任何个人维护的分支**（[决策 007](decisions/007-official-sources-and-terminal-route.md)）。

GPUI 没有 Zed 官方的定期发布：crates.io 上的 `gpui` 停在 2025-10-22 的 0.2.2（验证报告 4.4 节）。可用的官方来源是 `gpui-pre`：

- `gpui-pre 0.3.11` 是官方 Zed 代码 `zed@f816e1f` 的快照，Apache-2.0，2026-11-02 发布；此前 0.3.9（10-12）、0.3.10（10-26），约一两周一版。
- 快照由 gpui-kit 的维护者发布（crates.io 账号 `huacnlee`，Longbridge），`gpui-kit 0.9.0` 与它成对发布、成对测试。
- tTerm 只依赖 `gpui-kit = "=0.9.0"`，通过它的再导出使用 GPUI，避免两处版本不一致。升级时两者一起升。

| 来源                                | 结论     | 理由                                                                                      |
| ----------------------------------- | -------- | ----------------------------------------------------------------------------------------- |
| **gpui-kit 0.9.0 + `gpui-pre`**     | **采用** | 代码是官方 Zed 的；版本化、可复现；组件库与 GPUI 成对测试                                 |
| 上游 Zed git + 上游 gpui-kit git    | 退路     | `gpui-pre` 停止发布时改用：直接依赖 `zed-industries/zed` 的固定提交，gpui-kit 视情况 fork |
| nyakang 的 zed、gpui-kit、alacritty | 不采用   | 个人维护；补丁大多服务于 nyaterm 自己的功能（嵌入原生子窗口、文件拖出等），见下           |
| gpui-ce（社区分支）                 | 不采用   | gpui-kit 不以它为目标                                                                     |

不采用 nyakang 分支意味着放弃它的补丁（2026-11-05 核对：比官方多 38 个提交、+7,607 行）。逐类判断：

| 补丁                                       | 规模        | tTerm 是否需要                                       |
| ------------------------------------------ | ----------- | ---------------------------------------------------- |
| 在窗口中嵌入原生子窗口                     | 约 1,500 行 | 不需要，服务于 nyaterm 的 RDP、VNC                   |
| 文件拖出到 Finder 或资源管理器             | 约 3,000 行 | 先用 tTerm 自己的 `src-tauri/src/drag_out/`（P0-19） |
| Windows 输入法只更新光标的组合事件         | 一个提交    | 可能需要，由 P0-18 的 Windows 中文输入法测试决定     |
| 图集分配失败不崩溃、关闭对话框恢复所属窗口 | 数百行      | 暂缓；遇到对应问题时再处理                           |

真要给 GPUI 打补丁时，不 fork 整个 Zed：用 Cargo 的 `[patch.crates-io]` 把 `gpui-pre` 换成一份打过补丁的同版本源码。这条退路在 P0-06 实测一次（P0-07 负责逐项评估）。

已知要在 P0-06 统一的依赖冲突（以 gpui-kit 0.9.0 的实际依赖树为准）：

| 依赖            | tTerm                   | 待查                                 | 处理                                                     |
| --------------- | ----------------------- | ------------------------------------ | -------------------------------------------------------- |
| rustls 后端     | `ring`                  | gpui-kit 的依赖是否带入 `aws-lc-rs`  | 同一构建只能选一个，统一为 `ring`；不行就记录原因        |
| reqwest         | 0.13（crates.io）       | gpui-kit 是否依赖其他 reqwest        | 接受两个版本共存时记录体积                               |
| vte             | 0.15                    | `alacritty_terminal 0.26.0` 所需版本 | `cargo tree -d` 确认                                     |
| windows / objc2 | windows 0.62、objc2 0.6 | `gpui-pre` 的版本                    | 列出重复版本；类型要跨 crate 传递的必须统一              |
| 工具链          | 1.99.1，edition 2021    | `gpui-pre` 的 `rust-version`         | 新 crate 用 2024 edition；`rust-toolchain.toml` 固定版本 |

P0-06 的核对结果（详见 [pins.md](pins.md) 与[决策 002](decisions/002-dependency-sources.md)）：没有 `aws-lc-rs`，rustls 只有一个 `ring` 后端；reqwest 两个版本共存，GPUI 用的是改名发布的 `gpui-pre-reqwest 0.12.15`（reqwest 0.12 快照）；`vte` 只有 0.15.0；tTerm 与 `gpui-pre` 共用 `windows 0.62.2` 与 `objc2 0.6.4`；`gpui-pre` 不声明 `rust-version`，树上最高 MSRV 是 1.92，1.99.1 保留。

## 4. 组件库：gpui-kit

[longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)（原 gpui-component，Apache-2.0，约 1.9 万星）是 GPUI 生态里最大的组件库，也是 nyaterm 用的那个。

选它的理由：

- **设计来源与 tTerm 相同。** README 写明“UI design based on shadcn/ui”，分层也照搬 shadcn：`gpui-base` 相当于 Base UI，只管行为与状态；`gpui-component` 是带样式的一层。tTerm 的 `src/components/ui/` 本身就是 shadcn 风格，令牌名（background、foreground、muted、border、ring、radius）可以直接对应。
- **覆盖面够。** 75 个以上的组件，tTerm 需要的都有：按钮、输入框、选择框、组合框、复选框、开关、单选、滑块、标签页、对话框、弹出层、悬停卡片、提示、通知、菜单、虚拟列表、虚拟化表格（固定与可调列宽、排序）、树、侧边栏、可调整面板、滚动条、颜色选择器、快捷键显示、进度、骨架屏、标题栏、窗口边框、Markdown 与 HTML 渲染、图表、代码编辑器、检查器（inspector）。
- **代码编辑器。** README 称可处理 20 万行，带 tree-sitter 高亮和 LSP 诊断、补全、悬停。远程文件编辑器替代 CodeMirror 的首选，由 P0-20 验证。
- **无界面测试。** README 列出 headless UI integration testing。

**怎么用：行为借 gpui-kit，外观归 tTerm。** “与现有界面一致”意味着每个组件的高度、内边距、圆角、字号、焦点样式都要等于现在 Tailwind 里的数值。gpui-kit 的默认外观接近 shadcn，但不会恰好等于 tTerm 的。所以：

1. `tterm-ui` 为 `src/components/ui/` 的每个组件提供一个同名、同变体的 Rust 组件。
2. 交互复杂、样式可控的（输入框、编辑器、虚拟列表、表格、树、菜单、弹出层定位），封装 `gpui-component`，用主题和样式参数调到一致。
3. 样式很难调到一致的，用 `gpui-base` 的行为加自己的绘制。
4. 每个组件取哪一种由 [P0-24](ui-parity.md#p0-24-ui-原语还原验证) 的截图对比决定，不靠目测。

注意：gpui-kit 0.9.0 发布到 crates.io 的清单带 `license = "Apache-2.0"`；验证报告 4.2 节核对的是发布之前的仓库清单，当时没有该字段，仓库里另有 `LICENSE-APACHE`，两者一致（P0-06 核对）。复用前在 `third-party.md` 登记。

其他库只做参考：

| 库                                 | 用途                                            |
| ---------------------------------- | ----------------------------------------------- |
| gpui-devtools                      | 元素拾取与样式检查，开发时对照布局              |
| gpui-agent                         | 让 agent 检查和驱动 GPUI 应用，P0-23 的备选方案 |
| gpui-storybook                     | 组件展示窗口的做法参考（P3-06 的展示窗口）      |
| gpui-hooks、gpui-router、gpui-form | 不采用。tTerm 只有一个路由；状态按 Entity 组织  |

## 5. 终端

2026-11-05 决定（[决策 007](decisions/007-official-sources-and-terminal-route.md)）：**以官方 `alacritty_terminal` 为内核，其余自己搭，按来源分四块：**

| 部分                                                        | 来源                                                                                                                                                                      | 许可                    | 任务卡                            |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | --------------------------------- |
| 终端内核：解析、屏幕、回滚区、选区、搜索的底层              | crates.io 官方 `alacritty_terminal = "=0.26.0"`，不打补丁                                                                                                                 | Apache-2.0              | P0-06、P0-09                      |
| 终端模型封装、绘制元素、滚动、输入法                        | 自写。架构参考 Zed 的 `terminal` 与 `terminal_view`：先由读过 Zed 的 agent 写纯文字设计说明，实现者以说明为主、可以阅读 Zed 源码，但不复制代码（决策 008）                                 | tTerm 自己的代码（MIT） | P0-27、P0-09、P0-10、P0-12、P0-13 |
| 按键与鼠标协议、方框与 Powerline 字符、链接识别             | 从 Alacritty 应用层复制并改写到 GPUI：`input/keyboard.rs`（718 行）、`input/mod.rs`（1,559 行）、`renderer/text/builtin_font.rs`（1,032 行）、`display/hint.rs`（703 行） | Apache-2.0              | P0-28                             |
| 关键词高亮；sixel 与 kitty 图片协议（现有版本没有，只评估） | 从 nyaterm 挑选模块复制，先过质量关卡                                                                                                                                     | Apache-2.0              | P0-29                             |

这样做的理由：

- **全部依赖来自官方。** Zed 自己的 Alacritty 分支只改了启动 PTY 时的信号屏蔽，tTerm 有自己的 PTY 与 SSH 通道，用不到；nyaterm 的 2 个 Alacritty 补丁只服务于它自己的滚动锚定设计。所以官方 0.26.0 就够用。
- **最容易出错的输入协议用久经检验的实现。** Alacritty 的按键编码与鼠标上报 2017 年起被大量用户使用。
- **不整体继承一个年轻项目的设计。** nyaterm 2026-04 创建，6 个月约 3,900 次提交，有 48 个未关闭的 bug（2026-11-05）；它的代码测试充分，但单文件最大 3,488 行，需要大改才符合本仓库规则。只取独立模块，改造量小。
- **代价：** 终端部分比整体复用 nyaterm 多约 4–8 周，主要在绘制元素、滚动锚定与输入法。

**Zed 参考规则**（README 执行规则 6 的细化；2026-11-07 起按[决策 008](decisions/008-zed-terminal-reading.md)，替代原来的净室规则）：

1. P0-27 的设计说明里不得出现代码片段、逐行翻译或 Zed 私有的标识符；可以写 Alacritty 与 GPUI 的公开接口名。评估者已审查通过。
2. 实现终端的 agent 以设计说明与官方文档为主要依据，可以阅读 Zed 的 `terminal`、`terminal_view` 源码，只借鉴思路。
3. 不得复制 Zed 的代码、注释、测试或私有标识符，不得逐个函数翻译或照搬模块结构，不引入 GPL 依赖。
4. 实现卡的报告列出阅读过的 Zed 提交与文件，并声明没有复制。评估者对照这些文件抽查实现。
5. P0-09、P0-10 在原净室规则下完成，报告中的“未阅读 Zed 终端源码”声明保持原样。

回滚区写满后的锚定是已知难点：nyaterm 为此给 Alacritty 加了滚动计数，官方版本没有。P0-12 以现有 xterm.js 的行为为目标，评估不改内核的做法（例如比对视口内容、或向 Alacritty 官方提交补丁），不得自行 fork。

其他候选及放弃原因：

| 候选                            | 情况                                                         | 结论                                                      |
| ------------------------------- | ------------------------------------------------------------ | --------------------------------------------------------- |
| 整体复用 nyaterm 两个终端 crate | Apache-2.0；需要 Alacritty 带 2 个补丁                       | 不采用，只挑模块（P0-29）                                 |
| Zed `terminal`、`terminal_view` | GPL-3.0-or-later，且深度依赖 Zed 的 workspace、editor、ui    | 可以阅读、参考架构，不复制代码（决策 008）                |
| `gpui-terminal` 0.1             | 基于 alacritty_terminal，提供 `TerminalView`，接受任意读写流 | 太早期，没有输入法、滚动模型、搜索                        |
| `gpui_xterm` 0.1.1              | 基于 alacritty_terminal 0.25.1 与 crates.io 的 gpui 0.2.2    | 绑在旧 GPUI 上                                            |
| libghostty-vt                   | Ghostty 的 VT 内核，有 Rust 绑定，不是线程安全的             | 没有 GPUI 绘制层，要配 Zig 工具链；作为日后替换内核的备选 |

## 6. 依赖迁移表

### 6.1 前端依赖

| 现有（npm）                                                 | GPUI 版                                                                                     | 一致性                            |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------- |
| react、react-dom、@tanstack/react-router                    | GPUI 的 Entity 与 `Render`；只有一个路由，不需要路由库                                      | —                                 |
| tailwindcss、clsx、class-variance-authority、tailwind-merge | GPUI 的样式方法（命名与 Tailwind 相同，如 `px_2`、`rounded_md`）；变体写成 Rust 枚举        | 数值可逐一对上，见 ui-parity 2 节 |
| `src/styles/themes.css` 的 CSS 变量                         | `tterm-ui` 的设计令牌，由脚本从同一份源文件生成（UP-03）                                    | 数值完全一致                      |
| lucide-react                                                | 同版本 Lucide SVG，构建时导出到 `tterm-ui/assets/icons`                                     | 同一份矢量                        |
| vscode-material-icons、`src/assets/brand-logos`             | 导出用到的 SVG                                                                              | 同一份矢量                        |
| @radix-ui/react-toast                                       | gpui-component `notification` 封装为 `tterm-ui::Toast`                                      | 样式调到一致                      |
| @dnd-kit/react                                              | GPUI 的 `on_drag`、`on_drop` 加自写排序逻辑（标签、分组、窗格）                             | 拖动预览要单独还原                |
| @xterm/xterm 及 webgl、canvas 插件                          | `tterm-terminal-view`                                                                       | 接近，见 ui-parity 3 节           |
| @xterm/addon-fit                                            | 按元素尺寸与字体度量计算行列                                                                | 行列数必须一致                    |
| @xterm/addon-search                                         | 终端模型的搜索                                                                              | —                                 |
| @xterm/addon-serialize                                      | 新的快照格式，另外**必须能读旧的 SerializeAddon 快照**（P2）                                | —                                 |
| @xterm/addon-unicode11                                      | unicode-width 与内核的宽字符处理                                                            | 逐项核对差异                      |
| @xterm/addon-web-links                                      | 内核的链接识别                                                                              | —                                 |
| @codemirror/*、@lezer/highlight                             | gpui-kit 代码编辑器 + tree-sitter 语法（css、html、javascript、json、markdown、rust、yaml） | 接近，主题色对齐                  |
| i18next、react-i18next                                      | `tterm-i18n`                                                                                | 文案完全一致                      |
| react-markdown、remark-gfm、remark-emoji                    | gpui-component Markdown + 预处理 emoji 短码                                                 | 接近                              |
| @tauri-apps/api（invoke、event、Channel）                   | 直接调用 `tterm-core` 的服务，订阅类型化事件                                                | —                                 |

### 6.2 Tauri 与插件

| 现有                                  | GPUI 版                                                                                                  | 备注                                                    |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| tauri（窗口、事件、托盘）             | GPUI 窗口；托盘用 `tray-icon` + `muda`（Tauri 团队维护，不依赖 Tauri）                                   |                                                         |
| `macos-private-api`、`window_blur.rs` | GPUI 的 `WindowBackgroundAppearance`（不透明、透明、模糊）                                               | Windows 的 Mica 与 Acrylic 在固定 rev 上待核实（P0-25） |
| tauri-plugin-frame                    | GPUI 的标题栏选项（macOS 透明标题栏与红绿灯位置）；Windows 用 gpui-kit 的 `title_bar` 与 `window_border` |                                                         |
| tauri-plugin-dialog                   | 文件选择用 GPUI 自带的 `prompt_for_paths` / `prompt_for_new_path`；消息框用自己的对话框，保持现有外观    |                                                         |
| tauri-plugin-opener                   | GPUI 的 `open_url`、`reveal_path`                                                                        |                                                         |
| tauri-plugin-clipboard-manager        | GPUI 剪贴板                                                                                              | 图片剪贴板需要时再用 arboard                            |
| tauri-plugin-os                       | 标准库与已有的 `sys-locale`                                                                              |                                                         |
| tauri-plugin-process                  | GPUI 的 `restart` 与 `quit`                                                                              |                                                         |
| tauri-plugin-single-instance          | 自写：Unix 套接字或 Windows 命名管道转发参数                                                             | 必须保留 `tterm` 命令与文件夹右键菜单的参数协议         |
| tauri-plugin-updater                  | 见第 7 节                                                                                                |                                                         |
| tauri bundler 与 NSIS 钩子            | cargo-packager + 自定义 NSIS 模板；RPM 用 cargo-generate-rpm                                             | 安装目录、注册表键、产品名不变，才能原地升级            |
| devtools                              | gpui-kit `inspector` 特性、gpui-devtools                                                                 |                                                         |
| `webview2-com`、`ime_focus.rs`        | 删除                                                                                                     | 只为修 WebView2 的输入法焦点                            |

### 6.3 不动的后端依赖

russh、russh-sftp、ring、rusqlite、keyring、portable-pty、serialport、tokio、aes-gcm、argon2、zip、reqwest、rustls、quick-xml、`windows`、objc2 系列（凭据、通知、拖出、Touch ID）都继续用，版本不随迁移升级（README 执行规则 5）。

## 7. 打包与更新

迁移最容易出事故的一步，是**让已经装了 Tauri 版的用户自动升级到 GPUI 版**。约束：

- 应用标识 `com.stone.tTerm`、产品名 `tTerm`、可执行文件名、安装目录、数据目录、钥匙串服务名都不变。开发期二进制叫 `tterm-gpui`、标识为 `com.stone.tTerm.gpui-dev`，P5-02 打正式包时才改成 `tTerm` 与正式标识；此时 `src-tauri` 的 `tTerm` 二进制要么已删除，要么改名，避免同名冲突。
- 最后一个 Tauri 版用现有更新器、现有 `latest.json` 和现有 minisign 公钥，安装第一个 GPUI 版。这要求 GPUI 版的安装包在格式上满足 Tauri 更新器：macOS 是 `.app.tar.gz`，Windows 是能接受 Tauri 安装参数（`passive` 模式）的 NSIS 安装包，Linux 是 AppImage。
- 之后 GPUI 版自己的更新器继续读同一份清单、用同一个公钥验证。

选型：

1. **cargo-packager** 产出 `.app`、`.dmg`、NSIS、`.deb`、AppImage。它不出 RPM，RPM 用 cargo-generate-rpm，沿用 [RPM beta 版本号](../../src-tauri/tauri.linux.conf.json) 中 epoch 1 与 `beta.N` release 的规则。
2. **cargo-packager-updater** 作为更新器首选。它与 Tauri 更新器同出一系，但签名格式与清单字段是否与现有 `latest.json` 兼容**待核实**；不兼容就用 `minisign-verify` 自写下载、验签和安装。
3. 这两点在 [P0-26](#p0-26-打包与更新衔接可行性) 先做最小验证，不等到 P5。

### P0-26 打包与更新衔接可行性

- 依赖：P0-08
- 规模：M
- 并行：P0-09 之后的各卡
- 参考：`src-tauri/tauri.conf.json` 的 `plugins.updater`、`src-tauri/src/updater.rs`、`.github/workflows/release-main.yml`、`src-tauri/windows/` 下的 NSIS 钩子、[P5-03](p5-release.md#p5-03-更新器)
- 工作：
  1. 用 cargo-packager 给 P0-08 的最小程序打出三平台安装包，标识、产品名与安装目录与现有 Tauri 版相同，**使用 `.gpui-dev` 标识的副本做实验**，不覆盖本机正式版。
  2. 用现有的签名私钥（测试用一对新密钥）签名，构造一份 `latest.json`，让一个本地构建的 Tauri 版通过自带更新器把自己替换成这个 GPUI 程序。三平台各做一次。
  3. 核对 cargo-packager-updater 能否读同一份清单、验同一种签名；不能时写清缺什么。
  4. 列出现有 NSIS 钩子（系统集成的注册与恢复选项）要搬到新模板里的部分。
- 产出：`reports/P0-26.md`、演示脚本、`decisions/004-packaging.md`。
- 验收：三平台都有“Tauri 版自动升级到 GPUI 程序后能启动”的录屏或日志；不可行的平台写明原因和替代路径（例如该平台改为手动下载）。
- 评估重点：Windows 安装目录与卸载项是否被正确接管；macOS 替换 `.app` 后钥匙串是否反复询问（签名身份变化）。
- 不做：正式发布流程改造（P5-02、P5-03）。

## 8. 来源

查询日期 2026-11-05：

- [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)：README、组件目录 `crates/component/src`；crates.io 上 [gpui-kit](https://crates.io/crates/gpui-kit) 与 [gpui-component](https://crates.io/crates/gpui-component) 0.9.0 的依赖（`gpui-pre =0.3.11`、`rust-i18n`、`ropey`、`tree-sitter`）。
- [crates.io gpui](https://crates.io/crates/gpui)：最新 0.2.2，2025-10-22。
- [gpui-pre](https://docs.rs/crate/gpui-pre/0.3.10)：Zed 提交的快照。
- [nyakang/nyaterm Cargo.toml](https://github.com/nyakang/nyaterm/blob/main/Cargo.toml)：固定的 zed、gpui-kit、alacritty rev；v2.0.2 为 `zed@a755e55`、`gpui-kit@9143df9`、`alacritty@86ff089`。
- 分支与官方的差异（2026-11-05）：[nyakang/zed](https://github.com/zed-industries/zed/compare/main...a755e55acb90229c687e64e6a84877fb70d21619)、[nyakang/gpui-kit](https://github.com/longbridge/gpui-kit/compare/main...9143df9774d0190162053defc9b1faac3f8ab907)、[nyakang/alacritty](https://github.com/alacritty/alacritty/compare/master...86ff089983b93700c065ae3ae57273addf561fe7)、[Zed 的 Alacritty 分支](https://github.com/alacritty/alacritty/compare/master...4c129667ce56611becdc82de6e28218c80e2e88f)。
- [crates.io alacritty_terminal](https://crates.io/crates/alacritty_terminal)：最新 0.26.0，2026-04-06。
- [crates.io gpui-pre](https://crates.io/crates/gpui-pre)：0.3.11 为 `zed@f816e1f` 的快照，发布者 `huacnlee`。
- [Alacritty](https://github.com/alacritty/alacritty)：应用层 `input/`、`renderer/text/builtin_font.rs`、`display/hint.rs`，Apache-2.0。
- [zed-industries/awesome-gpui](https://github.com/zed-industries/awesome-gpui)：组件库与工具列表。
- [GPUI Community Edition](https://gpui-ce.github.io)。
- [gpui-terminal](https://docs.rs/gpui-terminal)、[gpui_xterm](https://docs.rs/crate/gpui_xterm/latest)、[libghostty-vt](https://docs.rs/libghostty-vt)。
- [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)。
- [Zed 截图测试提交 #45259](https://git.secluded.site/zed/commit/8c0b088366545e5050711066a4be666266f28110)：`VisualTestAppContext`、`Window::render_to_image`。
- [Zed 次像素文字渲染提交 #45423](https://git.secluded.site/zed/commit/47d2694dcceae02ff9ffd5d7d174389dde6a7bf3)。
- 同类 GPUI 应用：[Navop](https://deepwiki.com/feigeCode/navop)（自定义许可，不可复用代码）、[ShellMaster3](https://github.com/iwoov/ShellMaster3)。
