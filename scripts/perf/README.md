# P0-01 性能基线

脚本只驱动本地终端，不连接 SSH，不修改应用代码或依赖。Windows 上即使负载由 WSL 执行，显示路径仍为 Windows ConPTY → Tauri IPC → xterm.js → WebView2；不能标成 Linux 原生结果。

## 固定条件与负载

用同一 release 二进制、同一显示器和电源设置、相同字体与字号、窗口客户区、回滚行数连续跑两轮。每次只保留一个前台窗口；关闭 DevTools 面板，通过 CDP 保存 trace。记录系统缩放、屏幕刷新率、GPU、渲染器、透明度和背景任务。空闲窗口的光标闪烁保持配置不变。

`workload.sh LOAD RESULT_DIRECTORY` 在待测终端内运行，需要 bash、python3、coreutils、vim、tmux，禁止为了本卡从未授权来源安装工具。

- `cat`：生成 50,000,000 字节 ASCII（十进制 50 MB，500,000 行，每行 100 字节）后执行真正的 `cat`。首次预生成文件，采集时不计生成。重复使用相同文件，可用 `sha256sum` 核验。
- `yes`：真正的 `yes` 输出固定一行，由 `timeout` 在 30 秒结束。
- `vim`：`-Nu NONE -i NONE -n`，打开同一文件，20 ms 定时器执行一次 Ctrl+E，共 1500 次（30 秒目标时间）；实际可能因终端背压延长，检查 `.done`。
- `tmux`：独立 socket，四个 tiled 窗格同时 `yes` 30 秒，3 秒后退出。使用终端真实尺寸，退出清理该 socket，不影响用户 tmux。
- 空闲：通过应用正常的新本地标签功能分别创建 1、10、30 个会话，等待 15 秒稳定后采样 40 秒；前台标签一个，其余隐藏，不运行负载。每种数量在独立应用启动中测量更易复现；若累积增加，报告需说明。
- `vtebench`：在 WSL home 的官方检出中先编译 release，再设置 `VTEBENCH_DIR` 或使用默认 `$HOME/vtebench`；脚本执行标准 `benchmarks/` 全集并保存 `.dat`。当前待补（WSL 缺少 Rust 工具链）；脚本退出 2，不输出伪造数字。不要在 Windows 直接编译或运行。

负载 `.started` / `.done` 文件记录 UTC 完成证据。输出负载采样窗口为启动前后共 40 秒，包含启动和尾部排空；`cat` 的持续时间由文件大小和背压决定。报告必须记录实际窗口及是否看到完成标记。吞吐量与帧耗时是不同指标，不用生产者退出时间代替可见绘制完成时间。

## Windows 隔离与采集

在仓库根用 PowerShell 7：

```powershell
pnpm tauri build --no-bundle --config '{"identifier":"com.stone.tTerm.perf"}'
```

若命令行转义丢失引号，把相同 JSON 写到 `target/perf/identifier.json`，用 `--config target/perf/identifier.json`；该临时文件不提交。绝不启动正式或开发标识二进制。运行 `scripts/perf/start-windows.ps1` 设置全新空 APPDATA（LOCALAPPDATA 不变）并以 WebView2 调试端口启动，不绕过旧数据迁移保护。

启动后、输入前，确认系统原 APPDATA 下只有 `com.stone.tTerm.perf` 是本次数据目录；通过 CDP 只读调用 `list_profiles`、`load_session`，核验没有连接、没有恢复远程标签，同时保存 DOM/截图。发现任何连接或远程标签立即退出并阻塞，不输入。仅从正常界面打开本地终端，CDP 发键盘输入，不点击文件对话框按钮。

```powershell
pwsh -File scripts/perf/collect-windows.ps1 -RootPid <应用PID> -Workload yes -Seconds 40 -Output target/perf/run1-yes.json
node scripts/perf/summarize.mjs <原始JSON目录> docs/gpui-migration/baseline/windows.csv
```

采集器目标每秒枚举后代，含 WebView browser、renderer、utility、GPU 进程，保存 PID、父 PID、命令行和性能计数器实例。查询开销可能使实际间隔大于一秒（本机约 3 秒），以 JSON 的 elapsed_s 与 UTC 时间为准。以进程创建时间避免 PID 重用，持续跟踪已知后代以覆盖重挂父节点。只包含待测应用树，不计 DevTools 驱动进程或别的 WebView。WSL 虚拟机内负载生产者不包含在 Windows 进程树 CPU 中，报告说明此边界。

PowerShell `Get-Counter` 读取 Process 的 `% Processor Time`、Working Set、Working Set - Private、Private Bytes。CPU 总和除逻辑处理器数，单位为整机百分比；排除首个计数器初始化样本。Working Set 是各进程 RSS 相加，会重复计共享页，同时保留 private working set 与 committed private bytes。GPU Process Memory Dedicated/Shared Usage 按树中 PID 汇总；失败保留错误并标不可获得。不存在的数值留空，不当作零。

CSV 列固定为：平台、构建、负载、指标、数值、单位、样本数、备注。均值和峰值都由原始 JSON 计算，差异为 `abs(run2-run1)/abs(run1)*100%`；run1 为零时相对差异未定义。超 10% 要在报告解释噪声来源，尤其空闲接近零的 CPU、一次性 GPU 分配与峰值，不把它们悄悄丢弃。

测完退出待测应用，确认所有子进程退出；严格核对绝对路径后删除本轮 `com.stone.tTerm.perf` 与临时 APPDATA。不要删正式/开发数据目录，不操作真实 SSH 标签。启动器拒绝复用已有隔离目录。

## 帧耗时口径（P0-21 沿用）

Tauri 使用 WebView 开发者工具 Performance 录制，可用 CDP `Tracing.start`（`devtools.timeline,disabled-by-default-devtools.timeline.frame,benchmark,cc,viz`）实现同一录制来源；保存原始 trace。录制时窗口可见、在前台，不打开 DevTools 面板，以固定窗口尺寸运行相同负载。固定 40 秒窗口，记录 CPU 采样和 trace 是否同时开启（trace 有额外开销）。

主要帧指标定义为**渲染器合成帧绘制事件的连续时间戳差**（ms），由浏览器 trace 的 `DrawFrame` 瞬时事件（ph=I）同一进程、同一线程序列计算，明确报告实际 PID/TID；同时列 p50、p95、p99、最大值、样本数及 >16.667 ms 比例。它是合成帧间隔，不是实际显示完成时间、JS 工作耗时、显示器扫描延迟或输入到光子的延迟。`FramePresented` 如可用可另列为显示反馈，不能混入 DrawFrame 序列。本机 trace 的反馈事件稀少，所以没有据此推断实际呈现帧率。空闲不连续产帧时长间隔表示无新帧，不解释为卡顿；输出负载与空闲分开比较。若无 DrawFrame，保留 trace 标未获得，不用 rAF 回调模拟帧耗时。

`node scripts/perf/frames.mjs target/perf docs/gpui-migration/baseline/windows-frames.csv` 从 trace 提取以上指标，同时输出带源 trace SHA256 的原始帧事件子集（`.frames.json`）。完整 trace 保留在本机 target/perf，可导入开发者工具；提交的子集用于复算数字，不宣称可替代完整性能剖析。进程资源数字由 summarize.mjs 复算。

P0-21 原生端需在相同合成绘制阶段打点，用连续合成帧时间戳差、同样窗口和采样区间比较；GPUI 内部单帧绘制耗时另列，不与帧间隔混用。若无法提供同阶段打点，必须标口径不等价，不能直接作性能提升结论。固定 60 Hz 的门槛只是参照值；高刷新率机器要记录实际刷新率，比较仍用完全相同条件。前后台切换、窗口遮挡、trace 丢包会破坏样本，需记录或重跑。

## macOS / Linux（未运行验证）

```sh
bash scripts/perf/collect-macos.sh --pid <PID> --load yes --seconds 40 --output run1-yes.csv
bash scripts/perf/collect-linux.sh --pid <PID> --load yes --seconds 40 --output run1-yes.csv
```

共享 Python 标准库采集器用 `ps` 获取进程树与 RSS，macOS 另外保存 `footprint -p` 原始输出/权限错误；Linux 读取 `/proc/PID/stat` CPU ticks 和 `smaps_rollup` PSS。Linux CPU 为区间 ticks 差除整机核数；进程退出导致计数下降时需检查原始成员列表，不能直接当有效 CPU。macOS `ps %cpu` 是工具自身的时间口径，CSV 明确标 lifetime average，不冒充 Windows 区间 CPU。`footprint` 原始文本保留供人工解释，脚本不猜测跨版本的输出格式。GPU 内存没有通用可移植接口，标不可获得。两平台采集尚未运行，缺少本轮对应桌面环境；提供的脚本不能证明已通过验证。
