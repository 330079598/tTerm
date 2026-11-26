# P2-25 持续输出与内存自检准备

## 结论

部分完成。本轮确认现有 Unix 采样器可以按 G2 要求执行 1800 秒进程树采样，并在隔离 smoke 进程上验证 JSON/CSV 输出；没有伪造 30 分钟 GPUI 数字，也没有改变固定负载、采样窗口或门槛。真实 GPUI 图形窗口的 30 分钟长跑、历史锚点观察和无 GPU 验收仍待专用桌面人工执行。

## 现有测量入口

`scripts/perf/collect-unix.py` 已满足本轮准备要求：

- `--pid` 跟踪目标进程及其后代，采样 `ps` 的 PID、PPID、CPU、RSS 和命令名。
- `--seconds 1800` 支持完整 30 分钟窗口；脚本不对该值做缩短或重解释。
- macOS 保留每次 `footprint` 的原始 stdout/stderr，不猜测跨版本的内存单位；Linux 同时记录 CPU ticks 与 `smaps_rollup` 的 PSS。
- 输出 `<output>.json` 保存逐样本成员、elapsed 时间、RSS、CPU 和平台原始字段；输出 `<output>` CSV 保存均值/峰值摘要。
- 目标进程提前退出、后代成员变化或 footprint 权限错误会保留在原始 JSON 中，不能静默当作零。

## 隔离现场步骤

1. 使用独立 GPUI 数据根 `com.stone.tTerm.gpui-dev`，确认没有正式连接、SSH 标签或恢复快照；不要使用正式或 Tauri 开发数据目录。
2. 启动 release GPUI 窗口，打开本地 shell，使用真实终端输出负载；先让回滚区写满，再开始计时。记录窗口尺寸、字体、字号、回滚行数、显示器刷新率、GPU/渲染适配器和系统缩放。
3. 记录目标 GPUI PID，另开终端运行：

   ```sh
   python3 scripts/perf/collect-unix.py \
     --pid <GPUI_PID> \
     --load p2-25-continuous-output \
     --seconds 1800 \
     --output target/perf/p2-25-continuous-output.csv
   ```

4. 在同一 1800 秒窗口记录负载分段进度、首行行号/历史锚点、是否出现闪烁或跳行；采样结束保存 `target/perf/p2-25-continuous-output.json` 和 CSV，不只保留摘要。
5. 结束后确认 GPUI 及子进程退出，核对临时数据根路径后清理；不要删除正式数据目录。

## 成功判定

- 原始 JSON 覆盖完整 1800 秒，进程树成员和 RSS/footprint 记录连续可解释；若进程退出或采样缺口，报告为未通过/未验证，不插值。
- 回滚区写满后历史首行单调推进，滚动锚点不闪烁、不跳行；持续输出结束后能回到最新行。
- 30 分钟内存曲线有界，尾段没有持续增长趋势；RSS/footprint 的单位和工具口径按平台原样报告。
- 该测量不能替代 release 双门、完整终端视觉验收或无 GPU 空闲 CPU < 2% 验收。

## 本轮验证

环境：macOS arm64，Python 3.13。受限沙箱禁止启动 `ps`，因此在沙箱外使用短命 `sleep` 进程做 2 秒 smoke 测试：

```text
python3 scripts/perf/collect-unix.py --pid <sleep-pid> --load p2-25-smoke --seconds 2 --output <tmp>/measure.csv
```

结果：JSON 含 2 个样本；CSV 输出 `rss_mean`、`rss_peak` 与 `cpu_percent_ps_mean`，样本数均为 2。该结果只证明采集器输出格式可读，不代表 GPUI 性能或 G2 通过。

检查：

- 采样器 smoke：通过（沙箱外）。
- 30 分钟 GPUI 长跑：未执行，需要专用图形桌面和人工观察。
- 无 GPU/远程桌面：未验证。
- 窗口睡眠唤醒、显示器热插拔、切换显卡、RDP 断开重连、锁屏恢复：未验证。
- P2-25 完整 G2 验收：未通过；本报告只覆盖测量准备。

## 遗留问题

- 当前采样器不自动产生终端负载，也不自动读取终端首行/锚点；现场必须同时保存负载完成标记和人工/窗口证据。
- macOS 的 `ps %cpu` 是 lifetime average，不能与 Linux 区间 CPU 或 Windows PDH 数字混为同一指标。
- 无 GPU 和显示环境变化属于人工平台验收，不能在无头 smoke 测试中推断通过。
