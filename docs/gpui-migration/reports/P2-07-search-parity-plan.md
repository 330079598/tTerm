# P2-07 第三批：终端搜索栏原版对照与拖动

基点：本地 `GPUI` 的 `721af59d`。协调者定方案、复验、决定是否合并；Pi + DeepSeek V4.1 Flash 在 `../tTerm-P2-07-search-parity` 的 `migrate/P2-07-search-parity` 实现。按序读 AGENTS.md、迁移 README 第 1、2、4、6 节、environments.md、[上一批报告](P2-07-search-ui.md)的偏差及[设置参照](W0-05-settings-reference.md)。不要重做搜索算法、改设置页、重画原版或修改性能门槛。

## 先取参照，再改生产实现

1. 从 `src/parity/` 和 `scripts/parity/` 的 WebKit 工具挂载原版 `TerminalSearchBar`、`useTerminalSearch`、生产 CSS 与 xterm 搜索。仅用合成终端数据，不访问真实 shell、SSH、钥匙串、配置、历史或用户桌面。按上一批 capture 的 80 行输出、相同视口位置、`needle`、活动命中和计数复现。确认查询焦点、语言 en、dark、100%、800×600 内容区、DPR 2；记录实际状态，不硬编码静态计数。原版初始及拖到边界的截图和 DOM 实测尺寸先落盘。若无法取得可比状态，说明差异，不以源码或 GPUI 截图代替参照。
2. 用已有生产 `SearchBar` 和每窗格的终端表面对齐参照。仅左键把手启动，窗口移动/放开均能收尾，不能改变相邻窗格、触发文本选择或向 PTY 写入；关闭、切窗及缩放后不残留拖动。依实际栏和表面尺寸夹在四边（8 逻辑像素），处理首次布局、resize、分屏和栏宽变化。原版 ≤640px 的 CSS 按视口换行，不等于窗格断点；受限窗格要保证控件可达并记录实际取舍。外观核对把手、输入框、间距、按钮、计数、外框、活动高亮和非活动高亮；未达的模糊、颜色或控件差异用实测说明，不改视觉基线或增加例外。
3. 以真实分发的鼠标/键盘和生产焦点路径测试初始布局、拖超四边、把手外按下、放开后再移动、窄屏/分屏/resize、搜索与 Esc 后焦点和终端输入隔离。通过平台输入接口覆盖 marked text、取消和提交（如可用），与已提交 CJK 字符的测试分别记录。不要切换宿主输入法、发送系统全局按键或查看用户桌面。macOS 拼音候选窗、组合中 Esc、提交后 Esc 返回原窗格须单列；没有专用隔离桌面就给人工复验步骤，绝不标为通过。

## 交付与最终验收

- 小步本地提交（拆分超限文件单独提交），不 push、不合并；只改本卡文件。新增证据和复跑命令放 `reports/P2-07-search-parity.md` 与 `reports/evidence/P2-07-search-parity/`，两端各有 PNG 和结构化布局/状态，给出成对的实际偏差。截图重跑到临时目录，不覆盖已提交证据；不要把 WebKit/headless 写成原生桌面验证。
- 执行者先跑新增回归及相关搜索测试、格式化和尺寸检查，交付后由协调者从候选版本独立复跑交互与截图、根目录 `pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size` 及 `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`。若需准备前端依赖，只用 pnpm。重检查勿和 release 门并行。
- 协调者最后在空闲机器单跑 `node scripts/perf/gpui-release-frame-budget.mjs`；任一测量失败保留原始值、停止，不改预算。结论按「自动通过／未达／待桌面人工」分开；不能代替 P2-07 整卡或 G2 签收。仅协调者更新 dispatch 和 status。
