# SSH 连接列表：分组、搜索与最近使用（macOS，2026-11-26）

本批接续[首台 SSH 主机](P4-01-first-ssh-macos.md)，针对已有多个 SSH 连接的使用场景，不代表 P4-01 整卡或 G2/G3 通过。

## 范围与验收方法

- 对照 Tauri「已保存连接」：核对树的层级和组内顺序、未分组、折叠、扁平搜索、无匹配提示与最近使用排序。过滤或折叠后不能再打开隐藏的连接。
- 新建或编辑连接时可以输入分组路径。输入多余的斜杠和空格应归一化；清空可移回未分组；保存后应能重新读出。
- 最近使用的依据是远程 Shell 已经打开。浏览列表、切换排序、保存、发起但未完成的连接以及连接失败，都不是成功使用。另检查列表重开后的顺序与持久化结果。
- 本轮不迁移分组管理页、拖放、多选/批量删除、复制、图标、SSH 配置导入或高级认证。Windows/Linux 的桌面表现和像素一致性不在这次签收范围。

## 对照夹具

Tauri 与 GPUI 各用独立的临时连接库，初始保存顺序均为 `Atlas`、`Build`、`Gateway`、`Spare`。`Build` 属于 `Lab`，`Atlas` 和 `Gateway` 属于 `Operations / Service`，`Spare` 未分组。`Gateway` 指向仅绑定本机回环的临时 OpenSSH 服务；`Spare` 指向没有监听的端口。未读取个人连接库或使用个人 SSH 身份。

在 Tauri 窗口实测，组头顺序为 `Lab → Operations → Service → Ungrouped`，输入 `service audit` 仅剩 `Atlas`；继续输入不匹配的字符显示“No matching connections”，而不是空库引导。

## 原生窗口的实测结果

1. GPUI 中的分组和组内顺序与上述夹具一致。编辑 `Atlas`，输入 `Operations // Sandbox`，保存后再次打开显示 `Operations / Sandbox`；改回 `Operations / Service` 后再做列表对照。
2. 折叠 `Operations` 后，普通列表不再展示 `Atlas` 或 `Gateway`。搜索 `service audit` 仍能跨分组路径和用户名选中 `Atlas`；`ci.fixture` 找到 `Build`，`Gateway` 找到同名连接，`missingfixture` 显示无匹配。重开列表后搜索清空，同窗口折叠状态保留。
3. 通过列表打开 `Spare`（无监听端口），再切到「最近使用」，顺序仍是 `Atlas → Build → Gateway → Spare`。随后打开 `Gateway`，核对指纹与容器实际使用的 ED25519 公钥一致，以临时私钥登录。远端命令返回 `LIB-library-ssh-library`。
4. 重开列表，顺序变为 `Gateway → Atlas → Build → Spare`。独立只读检查 SQLite `app_meta`，`frontend.recent_profiles` 中只有 `gateway` 一条，无失败连接或秘密。见[成功连接后的列表截图](evidence/P4-01-ssh-library/recent.png)。该数据使用与 Tauri 相同的条目格式，但 Tauri 的 WebView 从 localStorage 读取历史；两端历史不会自动互通。

## 自动化复验与结论

长列表回归覆盖 48 个连接、三层分组、Home/End 和方向键滚动，标签导航与折叠不会拨号。最近记录上限为 50 条；回归验证了迟到事件不会挤掉较新的连接、重新登录只更新时间，同一连接不会重复。排序偏好按操作顺序写盘，并保留其他配置字段；本批没有实现跨启动恢复折叠状态。

前端六项、Rust 格式、全目标严格 Clippy、默认并发的全工作区测试及大小限制均通过。本机独立 release 测量：30 个后台会话下前台 p95 为 **0.334209 ms**（上限 16.667 ms）；30 MiB 全部处理并确认用时 **156.219 ms**（时限 500 ms）。这是本机结果，不替代 CI 或整个 G2 的验收。

**结论：本批通过；P4-01 整卡仍在进行。** 下一步优先开发 SSH 配置导入（P4-03），使用同一份合成配置验证两端的主机预览、选择与导入结果。
