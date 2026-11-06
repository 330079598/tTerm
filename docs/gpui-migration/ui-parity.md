# 界面一致性：能做到什么、怎么做、怎么验

> 日期：2026-11-05。目标：GPUI 版与现有 Tauri 版界面一致。
> 本文给出逐项的可行性判断、还原方法、对比工具与通过门槛。门槛由评估者（Claude）按 [acceptance.md](acceptance.md) 执行。

## 1. 结论

**结构、尺寸、颜色、图标、交互状态可以做到一致；文字、动画和终端字形只能高度接近；有三处做不到原样，要换一种画法。**

| 等级 | 含义                                                         | 内容                                                                                       |
| ---- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| 一致 | 数值逐一相同，截图差异只来自文字抗锯齿                       | 布局、间距、尺寸、圆角、边框、颜色与透明度、阴影、SVG 图标、图片、悬停与按下状态、光标形状 |
| 接近 | 并排看得出细微差别，单独看不出                               | 文字渲染与行高、过渡与关键帧动画、终端字形、代码编辑器、Markdown、滚动手感、焦点环         |
| 替代 | 现有效果在 GPUI 中没有对应能力，换一种画法，外观会有可见变化 | 元素级毛玻璃、圆角裁剪子内容、径向渐变与遮罩                                               |

“完全一致”的可验证定义见第 5 节：macOS 上逐屏截图对比，非文字区域近乎像素一致，文字区域只比字体、字号、字重、行高和位置。

## 2. 能做到一致的部分

GPUI 的布局引擎是 taffy，支持 flex 和 grid；样式方法按 Tailwind 命名（`px_2`、`h_8`、`rounded_md`、`text_sm`），单位同为 rem。现有代码中 grid 用了 137 处、flex 更多，都能一对一表达。

做到一致的前提是**取现有界面的实际计算值，而不是 Tailwind 的默认值**。tTerm 改过几处基础数值，照抄 Tailwind 文档会错：

| 项             | 现有实现                                                                                                 | GPUI 中的写法                                                                                                          |
| -------------- | -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 根字号         | `body` 为 `13px × --ui-font-scale`（`src/styles/base.css:8`），rem 仍为 16px                             | `rem_size` 保持 16px，正文字号单独设为 13px × 缩放比例                                                                 |
| 文字字号与行高 | `applyUiScalePercent` 改写 `--text-xs` 到 `--text-xl`，行高为 1.15、1.2、1.25（`src/lib/uiPlatform.ts`） | `tterm-ui` 的字号令牌照抄这组数值，不用 GPUI 的 `text_sm` 默认行高                                                     |
| 界面缩放       | 只缩放文字，不缩放间距                                                                                   | 同样只改字号令牌                                                                                                       |
| 字体栈         | `-apple-system, BlinkMacSystemFont, "Segoe UI", ...`                                                     | macOS `.SystemUIFont`，Windows `Segoe UI`，Linux 按同一顺序回退                                                        |
| 颜色           | `src/styles/themes.css` 的 HSL 变量，含 `/ 0.3` 这类透明度                                               | UP-03 从同一份源生成不依赖框架的 HSL 分量常量（`tterm_ui::tokens`），P0-24 再映射为 GPUI 的 `Hsla`，透明度由使用处传入 |
| 滚动条         | `::-webkit-scrollbar` 宽 8px，滑块圆角 4px，颜色取 `--muted-foreground / 0.5`                            | `tterm-ui::Scrollbar` 照此绘制，悬停变 0.8                                                                             |
| 阴影           | `shadow-*` 共 92 处                                                                                      | GPUI `BoxShadow` 支持多层、偏移、模糊和扩展                                                                            |
| 文字截断       | `truncate`、`text-ellipsis` 共 114 处                                                                    | `text_ellipsis` 与 `overflow_hidden`                                                                                   |
| 图标           | lucide-react                                                                                             | 同版本 Lucide SVG，相同的 `stroke-width` 与尺寸                                                                        |

## 3. 只能接近的部分

| 项           | 原因                                                                                                                                                    | 处理                                                                                                                                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 文字渲染     | macOS 两边都用 CoreText，Windows 都用 DirectWrite，差异小；Linux 上 WebKitGTK 用 FreeType 与 HarfBuzz，GPUI 用 cosmic-text，差异明显                    | 只要求字体、字号、字重、行高与基线位置一致；Linux 单独人工比对                                                                                                                                                                         |
| 字间距       | P0-25 已核实：`TextStyle`（`src/style.rs:438`）、`TextRun`（`src/text_system.rs:1070`）与 `shape_line`（`src/text_system.rs:672`）都没有 letter-spacing | 现有 `tracking-*` 无处可映射，逐处去掉并记入差异表；确需加宽的位置整行 `shape_line` 一次，再遍历 `runs[].glyphs[]`，用 `window.paint_glyph` 画在 `position.x + i × spacing` 处，kerning 不变，0px 时与整行绘制逐像素相同（P0-25 实测） |
| 过渡         | CSS `transition` 共 107 处，状态一变就自动插值；GPUI 没有自动过渡，每处要用动画 API 手写                                                                | 逐处登记时长与曲线照抄；用户看不出的可以去掉，但要在差异表中列出                                                                                                                                                                       |
| 关键帧动画   | `@keyframes` 与 `animate-*` 共 31 处，包括螃蟹（`Crab/crab.css`）与加载动画                                                                             | 用 GPUI 的 `with_animation` 重写，帧率与曲线对齐；空闲时不得持续重绘                                                                                                                                                                   |
| 终端字形     | xterm.js 用 canvas 测量字符宽度，自绘方框字符与 Powerline 字形；新终端用字体度量                                                                        | 同一字体下行列数必须一致；单元格宽高误差不超过 0.5 物理像素；方框字符与 Powerline 字形人工比对                                                                                                                                         |
| 代码编辑器   | CodeMirror 换成 gpui-kit 编辑器                                                                                                                         | 主题色、字号、行号栏宽度对齐，其余接受差异                                                                                                                                                                                             |
| Markdown     | react-markdown 换成 gpui-component 的渲染                                                                                                               | 更新说明等少数位置使用，标题、列表、代码块的间距对齐                                                                                                                                                                                   |
| 滚动手感     | WebKit 的惯性与橡皮筋效果                                                                                                                               | macOS 触控板惯性由系统事件提供；橡皮筋效果不做                                                                                                                                                                                         |
| 焦点环       | CSS `ring` 与 `outline-offset`；GPUI 没有 outline                                                                                                       | 用边框或阴影模拟，尺寸与颜色一致                                                                                                                                                                                                       |
| 普通文本选择 | 18 处 `select-text`；GPUI 的普通文本默认不能选                                                                                                          | 只在需要复制的位置（日志、错误信息、主机指纹）用可选文本组件                                                                                                                                                                           |

## 4. 需要替代的部分

这三项按原样做需要修改 GPUI 的绘制管线。默认采用替代画法；如果你要求原样，就按“原样方案”在自己的 GPUI 分支中实现，工作量另计。

### 4.1 元素级毛玻璃

现有 6 处在窗口内部的元素上用了 `backdrop-filter: blur`：对话框遮罩（`ui/dialog.tsx:134`，`backdrop-blur-sm`）、终端浮层（`TerminalOverlays.css:16`，14px）、终端内的提示（`TerminalXterm.css:103`，8px）、主机密钥对话框、广播管理器、SFTP 抽屉。

- 上游 GPUI 只支持**窗口级**的背景模糊（`WindowBackgroundAppearance::Blurred`）。P0-25 的三平台结论：macOS 用 `NSVisualEffectView`，可用；Windows 的 `Blurred` 走 `SetWindowCompositionAttribute` 的 Acrylic（Windows 10 17763 以上），`MicaBackdrop`／`MicaAltBackdrop` 走 DWM（Windows 11 22621 以上）；Linux 只有 Wayland 在合成器提供 `ext-background-effect` 或 `kde-blur` 时才有模糊，X11 只有透明、没有模糊。详见 [P0-25 报告](reports/P0-25.md)。
- 元素级模糊只在社区分支中见到实现（bite-gpui-ce 的 `backdrop_blur` 示例，且注明不支持圆角）。P0-25 已核实（2026-11-06）：gpui-pre 0.3.11 的 `Background` 只有纯色、线性渐变、斜纹与棋盘格四种（`src/color.rs:745`），`backdrop` 一词在整个 crate 里只出现在 `WindowBackgroundAppearance` 的文档中（`src/platform.rs:2899`），`gpui-base`、`gpui-component` 也没有。演示 `reports/assets/P0-25-blur-element.png`：80% 不透明底只把下层压暗，边缘仍是硬的。
- **替代：** 用不透明度更高的同色底代替模糊。视觉差别是透过浮层看不到被虚化的终端内容。
- **原样方案：** 在自己的 GPUI 分支中实现背景模糊着色器，Metal、wgpu、D3D11 三个后端都要写，规模 L 以上，而且要维护到每次升级 GPUI。

### 4.2 圆角裁剪子内容

GPUI 的内容遮罩（content mask）是轴对齐的矩形。父元素设了圆角和 `overflow_hidden`，子元素画到角上时**不会被裁成圆角**。

受影响最大的是孤岛风格（`src/styles/islands.css`）：圆角面板里的终端与列表。图片不在此列：`img()` 自己把 `style.corner_radii` 传给 `paint_image`（`src/elements/img.rs:494`），元素给同样的 `rounded` 就能与父容器圆角一致，不需要内边距方案；只有终端、列表这类子内容需要。

- P0-25 已核实（2026-11-06）：`ContentMask` 只有 `bounds` 一个字段（`src/window.rs:2455`），`overflow_mask` 也只按边界裁剪（`src/style.rs:638`）。演示 `reports/assets/P0-25-rounded-mask.png`：左边是 `overflow_hidden` 的失败效果，右边是内缩子元素的替代效果。
- **替代：** 圆角容器的内容区留出不小于圆角半径的内边距，让内容到不了角落；需要贴边的（终端背景色、列表选中行）由容器自己画圆角背景，子元素不画背景。
- 在不透明背景上可以在四角补画背景色遮住溢出；在透明或模糊背景上不行，只能靠内边距。
- **原样方案：** 在分支中给内容遮罩加圆角，涉及所有图元的着色器，规模 L 以上。

### 4.3 径向渐变、遮罩与 clip-path

- 加载动画的环形与 comet 的 `conic-gradient`（`ui/spinner.css:50-57`）。P0-25 已核实：gpui-pre 0.3.11 源码里 `radial` 与 `conic` 都零命中（`gpui-base`、`gpui-component` 同样没有），也没有渐变遮罩或 `clip-path`。**替代：** 圆环用 `PathBuilder::stroke` 直接描边，中间不画任何东西，下层的渐变透出来；径向渐变用 24 或 96 个同心圆近似，24 级有可见色带，96 级色带明显变窄但不会消失；comet 用 48 段圆弧、逐段把 alpha 从 0 提到 1（`reports/assets/P0-25-radial-gradient.png`）。
- 线性渐变共 31 处；P0-25 已核实**上限是 2 个色标**：`Background.colors` 是 `[LinearColorStop; 2]`（`src/color.rs:784`），构造函数只收 from 与 to（`src/color.rs:865`），着色器同形（`gpui-pre-wgpu-0.3.11/src/shaders.wgsl:404`）。需要第三个颜色时拼接相邻矩形，或把两个色标放在同一百分比上做硬停（`reports/assets/P0-25-linear-gradient.png`）；31 处要逐处确认是否真的需要三色，多数两色就够。
- `clip-path: inset(...)` 两处都是矩形，用内容遮罩即可，不受影响。

### 4.4 其他做不到或不做的

- **像素完全相同。** 两套渲染管线的抗锯齿与次像素定位不同，文字像素不可能逐点相等。第 5 节的门槛据此设定。
- **WebView 提供的输入框附加功能：** 拼写检查、自动填充、文本框的系统右键菜单（查询、翻译、朗读）。GPUI 文本框没有这些。
- **浏览器开发者工具与 React DevTools。** 用 gpui-kit 的 inspector 与 gpui-devtools 代替，能力较弱。
- **多色 SVG。** `svg()` 只取一个 `text_color`：文件栅格化成 alpha 掩码后单色着色（`src/elements/svg.rs:150`、`src/window.rs:5210`），SVG 自带的颜色被丢掉，演示 `reports/assets/P0-25-svg-color.png`。Lucide 图标是单色描边，不受影响；确实需要多色的，用 `SvgRenderer::render_single_frame`（`src/svg_renderer.rs:222`）栅格化后按图片画，缓存自己做。
- **Linux 上没有 Vulkan 的机器。** GPUI 在 Linux 上经 wgpu 走 Vulkan，只能退到 llvmpipe 等软件实现，CPU 占用高。WebKitGTK 在这类机器上仍可用。是否支持由 P0-17 决定。
- **无 GPU 的 Windows 云桌面。** nyaterm #938 已标为 `not_planned`（验证报告 4.3 节），上游不会修。

## 5. 对比方法

人眼比对无法复现，也无法让评估者给出“通过”或“不通过”。所以先建工具，再写界面。

### 5.1 流程

```text
页面状态清单（screens.md）+ 夹具数据（fixtures/*.json）
        │
        ├─ 参考端：Vite 跑 React 前端，模拟 invoke 返回夹具数据
        │          Playwright WebKit，固定窗口尺寸、DPR 2、字体
        │          → reference/<屏幕ID>.png + reference/<屏幕ID>.layout.json
        │
        └─ GPUI 端：VisualTestAppContext 用同一份夹具构建视图
                   capture_screenshot → actual/<屏幕ID>.png
                   debug_bounds 读取各标记元素 → actual/<屏幕ID>.layout.json
        ↓
tterm-parity compare → 每屏的布局差、颜色差、图像差热力图，汇总为 HTML 报告
```

- **元素标记。** React 侧给需要比对的元素加 `data-parity="<ID>"`，GPUI 侧用同一个 ID 设 `debug_selector`。只加属性，不改行为，Tauri 版可以合并。
- **参考端用 Playwright WebKit 而不是 Tauri 窗口。** macOS 上它与 WKWebView 是同一个引擎、同一套 CoreText；Tauri 窗口在 macOS 上没有 WebDriver 入口。每个大类抽一屏，用真实 Tauri 窗口截图（`screencapture -l`）交叉核对一次，证明两者没有系统性偏差。
- **截图只在 macOS 自动化。** `VisualTestAppContext` 只在 macOS 上有，用的是真实平台渲染，窗口放在屏幕外。Windows 与 Linux 做人工截图对照（第 5.3 节）。
- **布局 JSON 记录：** 每个标记元素的位置与尺寸、背景色、边框色与宽度、圆角、字体族、字号、字重、行高、文字颜色。

### 5.2 通过门槛（macOS，DPR 2）

| 检查 | 对象                                                       | 门槛                                                                         |
| ---- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- |
| 布局 | 尺寸不随文字变化的元素（容器、按钮、输入框、图标、分隔线） | 位置与尺寸偏差 ≤ 1 逻辑像素                                                  |
| 布局 | 尺寸随文字变化的元素（标签、自适应宽度的按钮）             | 偏差 ≤ 2 逻辑像素，或不超过宽度的 2%，取较大者                               |
| 样式 | 字体族、字号、字重、行高、圆角、边框宽度                   | 完全相等                                                                     |
| 颜色 | 背景、边框、文字颜色                                       | 8 位 sRGB 完全相等；有透明度叠加的区域 CIEDE2000 ≤ 1                         |
| 图像 | 遮掉文字区域后的整屏                                       | SSIM ≥ 0.99                                                                  |
| 图像 | 整屏（含文字）                                             | SSIM ≥ 0.97                                                                  |
| 例外 | 第 3、4 节列出的项                                         | 在 `parity/exceptions.md` 中逐项登记后，对应区域不参与图像比较，改为人工判定 |

任何一屏不达标，报告给出热力图和偏差最大的元素，评估结论为“需修改”。登记例外需要写明原因；第 4 节以外的新增例外要你同意。

### 5.3 Windows 与 Linux

- 每个大类（工作区、终端、SFTP、设置、对话框）至少一屏，在真实机器上分别截 Tauri 版和 GPUI 版，同一窗口尺寸与缩放 `[人工]`。
- 评估者并排检查布局、颜色和字体回退；Linux 上文字差异单独列出，不计入不通过。
- 布局 JSON 可以在三平台 CI 中由无界面测试生成。除了文字度量带来的差异，三平台的布局应当与 macOS 相同；超出第 5.2 节布局门槛的视为缺陷。

### 5.4 页面状态清单

`docs/gpui-migration/parity/screens.md` 列出每一个要比对的状态，ID 规则为 `<功能>.<视图>.<状态>`，例如 `settings.appearance.default`、`sftp.drawer.uploading`、`dialog.host-key.changed`。

每个状态都要比对的组合：

- 浅色、深色；
- 默认风格、孤岛风格（`ui_style`）；
- 窗口模糊关、开（模糊只做截图人工判定，不进图像比较）；
- 界面缩放 100% 与 125%。

按现有组件规模粗估：设置约 15 个页签共 60–80 个状态，SFTP 与传输约 20 个，连接与命令库约 20 个，工作区、标签与分屏约 25 个，各类对话框与提示约 30 个，合计约 150–180 个基础状态。乘以组合后约 1,200–1,400 张截图，必须全自动生成。

## 6. 新增任务卡

这些卡插入现有阶段，汇总见 [README 第 11 节](README.md#11-任务总表)。

### W0-04 界面对齐工具链

- 依赖：W0-01、P0-08
- 规模：L
- 并行：P0-09 之后的各卡
- 参考：本文第 5 节、[P0-23](p0-validation.md#p0-23-供-agent-使用的界面自动化与截图)、Zed 的 `visual_test_runner`
- 工作：
  1. 参考端：Vite 的 `parity` 模式，用夹具替换 `@tauri-apps/api` 的 `invoke`、事件与 Channel；Playwright WebKit 脚本按 `screens.md` 逐屏截图并导出布局 JSON。脚本放在 `scripts/parity/`。
  2. GPUI 端：`crates/tterm-parity` 提供按夹具构建视图、截图和导出布局 JSON 的测试辅助。
  3. 对比：`cargo run -p tterm-parity -- compare` 输出每屏的布局差、颜色差、SSIM 和热力图，以及汇总 HTML 报告与一份机器可读的 JSON 结果。
  4. 支持 `UPDATE_REFERENCE=1` 重新生成参考截图；参考截图不进 git，按 React 前端的提交哈希缓存。
- 产出：上述工具、`parity/README.md`（使用说明）、一个示例屏幕的端到端对比。
- 验收：
  - 示例屏幕（一个按钮加一个输入框）跑通全流程，报告中能看到布局 JSON 和热力图。
  - 故意把 GPUI 端按钮内边距改 1px，工具报告不通过并指出该元素；改回后通过。
  - 同一输入连续跑两次，结果完全相同。
- 评估重点：参考端的模拟是否覆盖界面实际调用的 command；夹具是否与真实数据结构一致（用 TypeScript 类型检查夹具）。
- 不做：编写全部屏幕的夹具（W0-05）。

### W0-05 页面状态清单与夹具

- 依赖：W0-04
- 规模：L，按功能拆成若干提交
- 工作：
  1. 写 `parity/screens.md`，覆盖第 5.4 节的全部基础状态。
  2. 为每个状态写夹具，并在 React 代码中给关键元素加 `data-parity`。
  3. 生成全部参考截图，检查有没有因为夹具不全而渲染出错的屏幕。
- 产出：清单、夹具、`data-parity` 标记、参考截图的生成记录。
- 验收：清单中每个状态都能生成参考截图；`pnpm test` 与 `pnpm check:size` 通过；`data-parity` 的改动不改变任何行为（现有测试全过）。
- 评估重点：是否漏掉了只在特定条件下出现的状态（错误、加载、空列表、超长文本、权限不足）。

### UP-03 设计令牌单一来源

- 依赖：W0-01
- 规模：M
- 工作：把 `src/styles/themes.css` 中的颜色、圆角与 `uiPlatform.ts` 中的字号行高抽成一份 `tokens.json`（2026-11-05 修订：现有代码没有阴影令牌，阴影来自 Tailwind 的 `shadow-*` 与组件 CSS，不在本卡范围），脚本由它生成 CSS 变量（替换现有的手写部分，生成结果与现状逐字节相同）和 `tterm-ui` 的 Rust 常量。
- 验收：生成的 CSS 与现有 CSS 的计算值相同（W0-05 的参考截图不变）；Rust 常量有测试与 JSON 对照。
- 不做：改任何颜色或尺寸。

### P0-24 UI 原语还原验证

- 依赖：W0-04、P0-08、UP-03
- 规模：M
- 工作：给 `tterm-ui` 加上 P0-06 固定的 GPUI 依赖，把 UP-03 的 `Hsl` 与 `FontSize` 令牌映射为 `Hsla` 与像素值（界面缩放与透明度在映射时传入）。然后用 gpui-kit 还原 `src/components/ui/` 中最难的 8 个组件：按钮（全部变体与尺寸）、输入框、密码输入框、选择框、开关、标签页、对话框、右键菜单。每个先尝试“封装 gpui-component 并调主题”，达不到第 5.2 节门槛再改为“gpui-base 行为加自绘”。
- 产出：8 个组件、对比报告、每个组件的做法与理由。
- 验收：8 个组件在浅色、深色下全部达到第 5.2 节门槛，或在 `exceptions.md` 中登记并说明原因。
- 评估重点：封装 gpui-component 时，主题调整是否会影响其他组件；中文输入法在输入框中是否正常。
- G1 必需。

### P0-25 GPUI 绘制能力核实

- 依赖：P0-06
- 规模：S
- 工作：在固定 rev 上逐项核实并写最小演示：窗口级模糊（macOS、Windows Mica 与 Acrylic、Linux）、元素级背景模糊、圆角内容遮罩、线性渐变色标上限、径向渐变、字间距、多层阴影、SVG 着色、`debug_selector` 与 `capture_screenshot`。
- 产出：`reports/P0-25.md`，逐项给出“支持”“不支持”“部分支持”和演示截图；据此更新本文第 3、4 节。
- 验收：每项都有演示代码或源码位置作为证据。
- G1 必需。

## 7. 对工作量的影响

“一致”而不是“相似”的要求，主要增加 P3 与 P4 的工作量：每个视图要写夹具、标记元素、反复对比到达标。按每个视图多出 20%–30% 估算，加上 W0-04、W0-05、UP-03、P0-24、P0-25、P0-26，总量从 90–120 人周升到约 **105–145 人周**。agent 执行能缩短编码时间，但对比、评估和三平台人工验收的时间不会减少。
