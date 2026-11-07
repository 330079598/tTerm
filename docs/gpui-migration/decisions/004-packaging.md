# 004 打包与更新衔接

- 日期：2026-11-07
- 决策人：stone（实验范围及偏差处理已确认；以下选型建议待评估确认）
- 状态：待确认

## 背景

[P0-26 报告](../reports/P0-26.md)在 Windows 11 x64 上，以完全隔离的
`com.stone.tTerm.gpui-dev` / `tTerm GPUI Dev` 做了 release 实验：Tauri
0.0.1 通过应用内更新安装 cargo-packager 生成的 GPUI 0.0.2 NSIS。
只使用新测试密钥与回环服务器，正式配置、正式数据和发布工作流没有修改。

安装器以 `/P /UPDATE /R /ARGS` 启动并自动启动 GPUI；安装目录、exe 名称、
卸载项与安装位置键得到接管。从原快捷方式启动的进程正常打开了 PTY shell；
直接运行安装后的 exe 时截图显示 PowerShell 提示符；两次启动实际使用的
ConPTY 和 OpenConsole 均在安装目录内。但开始菜单出现重复项，
卸载也留下原位置的快捷方式与安装位置键，因此默认模板尚不能直接用于正式迁移。

## 决定建议

1. Windows 打包继续采用官方 **cargo-packager 0.11.8** 作为 P5 的验证起点。
   CLI 以 `cargo install --locked --version 0.11.8` 安装，固定版本后复核模板；
   本卡未引入新的 workspace 依赖。实验验证了 NSIS 包能被当前 Tauri 更新器下载、
   验签、执行并启动 GPUI，不能据此将默认模板直接投入正式发布。
2. **cargo-packager-updater 0.2.3 不直接兼容当前原始 latest.json。** 独立探针
   实测报 `missing field format`。在各平台条目补 `format: "nsis"` 的仓库外副本中，
   同一个包和 Tauri 生成的同一种 minisign 签名可被下载并验证；篡改包与无效签名
   均被拒绝。若后续采用该候选，建议固定 `=0.2.3` 作为适配验证基线，先经依赖
   选型与 pins 流程，再由 P5-03 评估增加兼容字段或做解析适配。不要把它现在加入
   workspace，也不要将“验签兼容”表述为“清单无需适配”。
3. 延续现有 `version`、`notes`、`pub_date`、`platforms` 与 minisign 签名契约。
   `format` 扩展须同时对现有 Tauri 与 GPUI 更新器做兼容矩阵；本实验未验证
   带新增字段的清单重新驱动 Tauri。Windows 目标键已实测，macOS/Linux 的键、
   包格式与安装链路仍需各自验证。自写更新器是备选，本卡未实现。
4. **P5-02 必须修订快捷方式接管与卸载。** 新模板必须沿用 Tauri 的原快捷方式
   位置，或安装时删除并接管旧位置；卸载时清理根目录项与子文件夹项两个位置，
   并清理本产品安装位置注册表键。不得删除其他产品或真实数据。
   两边快捷方式的 **AppUserModelID 必须实测比较**：Tauri 通过
   `SetLnkAppUserModelId`，cargo-packager 通过 `ApplicationID::Set ${IDENTIFIER}`；
   标识与可执行路径一致仍不足以证明任务栏固定图标不会失效。
5. P5-02/P5-03 还需明确 `/UPDATE`、passive 模式、重启参数与卸载时是否保留
   快捷方式的行为。cargo-packager 0.11.8 模板没有 Tauri 当前模板的 UpdateMode
   逻辑。本次记录安装成功与残留，不在本卡修改模板或重新安装。
6. 系统集成钩子不能原样复制。P5-02 先参数化 identifier、菜单 verb、命令名、
   bin/数据目录和失败提示，保留用户选择与失败不阻断语义；P5-03 验证更新时
   保留/恢复及非更新卸载清理。报告的“NSIS 钩子迁移清单”为逐宏证据。
7. Windows 包必须携带 `conpty.dll`、`OpenConsole.exe`、`conpty-LICENSE.txt`，
   运行证据应确认实际 DLL 与 OpenConsole 路径，而非仅检查资源存在。本卡原型的
   最小改动是在 exe 旁资源完整时跳过源码树预加载；P1-12 仍需正式路径注入。

## 影响与风险

- **macOS、Linux 全部未验证。** macOS 仍缺 arm64/x64 的包构建、`.app.tar.gz`
  原位更新、权限/签名/公证及钥匙串交互的设备与证书条件；Linux 缺原生构建运行、
  AppImage 替换、deb/rpm 安装与依赖基线验证。手动下载是可调查的备选，未验证。
- P5-03 的过渡 Tauri 版、localStorage 迁移与两级更新地址不可省略。本卡的新空数据
  实验不能证明真实用户数据完整、GPUI 到 GPUI 更新或失败回退。
- 私钥仅为本机新测试密钥，不能转用于正式发布。正式密钥与代码签名未读取、未使用；
  签名身份变化、任务栏固定项、快捷方式 AppUserModelID 和安全回退仍需后续实测。
- 实验卸载退出码 0 后残留原快捷方式；按决策人授权核对目标并精确删除，最终快捷方式
  与 before 一致。实验安装位置键和独立实验数据缓存在报告提交时保留，之后已被删除
  （删除者待确认，见报告）；正式条目无变化。
  完整命令、原始日志、快照、截图与哈希见 P0-26 报告及其 assets。
