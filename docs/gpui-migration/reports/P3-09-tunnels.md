# P3-09：端口转发只读页

这张卡解决两个问题：从工作区能找到 Tauri 同名页面；查看端口转发时，后台的窗格和会话不被替换。它不是端口转发管理器的完整迁移。

## 复现 Tauri 参照

在仓库根目录运行（Windows，需要安装 Edge）：

```powershell
node scripts/parity/capture.mjs --platform windows --screen tunnels.empty --screen tunnels.list.running --out target/parity/tunnel-page
```

参照直接渲染 `src/components/TunnelsPanel`，使用模拟 IPC，不启动 Tauri 后端。两张 PNG 及各自的 `*.layout.json` 都在输出目录里。数据取自 `parity/fixtures/tunnels.*.json`：一组没有规则，另一组有一条运行中的本地转发、一条已停止的动态转发；域名和流量都是虚构的。两张图都只包含页面内容，不含标题栏。

Windows、800 × 600、深色 classic、100% 时，Tauri 页头高 73.8 CSS px。列表两侧各留 24 px；首卡起点为 `(24, 97.8)`，宽 752 px；第二张卡与首卡垂直相隔 16 px。空状态区宽 448 px，水平居中。这些是 Tauri 的测量值，不是两端一致性的评分。

| 800 × 600 页面截图 | Tauri 生产组件 | GPUI 生产视图 |
| --- | --- | --- |
| 空状态 | [Tauri 空页](assets/P3-09-tunnels/tunnels.empty.png) | [GPUI 空页](assets/P3-09-tunnels/gpui-tunnels.empty.png) |
| 运行中与已停止 | [Tauri 列表](assets/P3-09-tunnels/tunnels.list.running.png) | [GPUI 列表](assets/P3-09-tunnels/gpui-tunnels.list.running.png) |

复现 GPUI 截图：`cargo test --workspace renders_parity_fixtures -- --ignored --nocapture`。该命令借用系统图形环境绘制离屏窗口，不读取正式工作区。图中两端均为单列、三段路由；GPUI 的卡片略高，第二张卡的起点比 Tauri 低约 8 px。运行连线在 GPUI 是静态实线；Tauri 使用虚线。常规工作区测试另用两个真实本地 PTY 验证了关闭页面后焦点和 35:65 分屏比例不变；截图本身不验证这些行为。

## 在 GPUI 手动复核

1. 从根目录执行 `$env:TTERM_GPUI_CHECK_INSTANCE='309'`，再执行 `cargo run --bin tterm-gpui`。此调试实例使用隔离目录，首次进入通常是空列表，无需复制正式配置或密钥。
2. 准备两个本地窗格，调整为非均分比例，在其中一个运行持续输出的命令。点标题栏设置旁的「端口转发」，再点一次。应只有一个端口转发页，没有新开的 shell。
3. 检查页头、空状态或规则卡片。关闭页面后，原会话应继续输出，比例不变，键盘输入应落回进入前的窗格。

## 仍然不同的地方

- Tauri 有新建、启停、编辑、删除等操作；这里故意不画不能使用的按钮。没有规则时看到空页是预期行为，不代表没有连接到 SSH。
- 规则与累计流量是持久化数据，运行状态来自各自进程的 `TunnelManager`。不能用另一个客户端中正在运行的隧道来验证 GPUI 的「运行中」计数。
- 此卡不覆盖整个标题栏、其他工具页、托盘或通知入口，也不把组件截图当作实机桌面验收。
