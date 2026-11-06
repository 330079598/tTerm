# 界面对齐工具链（W0-04）

本目录记录 React 参考端与 GPUI 端的逐屏对比工具：怎么生成参考截图、怎么渲染 GPUI 屏幕、怎么跑对比、怎么加一屏，以及门槛和输出格式。门槛本身来自 [ui-parity.md 第 5.2 节](../ui-parity.md#52-通过门槛macosdpr-2)，本工具只负责执行它。

W0-04 只提供工具和一张示例屏幕（一个按钮加一个输入框），全部状态清单与夹具是 W0-05 的范围。示例屏幕的 id 是 `example.button-input.default`。

## 1. 组成

```text
parity/screens.json                  屏幕清单：窗口尺寸、主题、缩放、逐屏 id 与夹具
parity/fixtures/*.json               每屏的 invoke 返回值，字段用 src/types 的真实类型检查
src/parity/                          Vite parity 模式的入口、屏幕注册表、Tauri IPC 模拟
  main.tsx                           读 ?screen= 与 ?theme=，挂载对应屏幕
  screens/ExampleButtonInput.tsx     W0-04 示例屏幕（使用真实的 Button 与 Input）
  tauri/{core,event,os}.ts           invoke/Channel、事件、固定 macOS 平台的模拟实现
  tauri/{window,app,path,webviewWindow,dialog,opener,process}.ts
                                      调用即报错的桩：这些能力只在原生窗口里存在
  tauri/modules.ts                   模块清单；modules.test.ts 对照 vite 别名与文件
scripts/parity/capture.mjs           Playwright WebKit：启动 Vite、逐屏截图、导出布局 JSON
scripts/parity/layout.mjs            在页面里采集 [data-parity] 元素与文字区域
crates/tterm-parity/                 GPUI 端：无头渲染、布局导出、比较、HTML/JSON 报告
docs/gpui-migration/parity/          本文档
```

两端的产物流向：

```text
parity/screens.json + parity/fixtures/*.json
        │
        ├─ 参考端：vite --mode parity + Playwright WebKit
        │          → target/parity/reference/<React 提交>[-dirty]/<屏幕>.png
        │          → target/parity/reference/<React 提交>[-dirty]/<屏幕>.layout.json
        │
        └─ GPUI 端：cargo run -p tterm-parity -- capture
                   → target/parity/actual/<屏幕>.png
                   → target/parity/actual/<屏幕>.layout.json
        ↓
cargo run -p tterm-parity -- compare
        → target/parity/report/index.html
        → target/parity/report/result.json
        → target/parity/report/heatmaps/<屏幕>.png
```

参考截图不进 git，缓存在被 `.gitignore` 的 `target/` 下。干净工作区按提交复用，`UPDATE_REFERENCE=1` 强制重生成；前端有未提交改动时目录带 `-dirty` 且每次重生成（见第 4 节）。GPUI 实际截图、热力图、报告也都在 `target/` 下，可随时重跑。

### Tauri 模块的 parity 处理

`vite.config.ts` 在 `--mode parity` 下用 `parityTauriAliases` 把下面 10 个模块全部改到 `src/parity/tauri/`；清单只在 `src/parity/tauri/modules.ts` 维护。`modules.test.ts` 遍历同一份清单，断言 Vite 确实从它生成别名、且每个实现文件都存在，新增 import 漏配时会失败。`plugin-os` 的返回值来自 `parity/screens.json` 的 `platform` 块（默认固定 macOS），不是浏览器探测。

| 模块 | parity 文件 | 处理方式 |
| --- | --- | --- |
| `@tauri-apps/api/core` | `core.ts` | 工作实现：夹具 `invoke`、`Channel`、`isTauri`、`convertFileSrc` |
| `@tauri-apps/api/event` | `event.ts` | 工作实现：进程内 `listen`/`once`/`emit` |
| `@tauri-apps/api/window` | `window.ts` | 桩：`getCurrentWindow()` 调用即报错 |
| `@tauri-apps/api/app` | `app.ts` | 桩：`getVersion()` 调用即报错 |
| `@tauri-apps/api/path` | `path.ts` | 桩：`homeDir()` 调用即报错 |
| `@tauri-apps/api/webviewWindow` | `webviewWindow.ts` | 桩：`getCurrentWebviewWindow()` 调用即报错 |
| `@tauri-apps/plugin-dialog` | `dialog.ts` | 桩：`open`/`save`/`message`/`ask`/`confirm` 调用即报错 |
| `@tauri-apps/plugin-os` | `os.ts` | 工作实现：`platform`/`version`/`family`/`type`/`arch`/`exeExtension`/`eol` 同步返回清单值 |
| `@tauri-apps/plugin-opener` | `opener.ts` | 桩：`openUrl`/`openPath`/`revealItemInDir` 调用即报错 |
| `@tauri-apps/plugin-process` | `process.ts` | 桩：`relaunch`/`exit` 调用即报错 |

## 2. 依赖与安装

| 依赖 | 版本 | 来源 | 用途 |
| --- | --- | --- | --- |
| Playwright（npm devDependency） | `1.54.0`（精确固定） | npm 官方 registry | 驱动 WebKit 截参考图 |
| Playwright WebKit 浏览器 | revision `2191` | `pnpm exec playwright install webkit` | 与 WKWebView 同引擎、同 CoreText |
| `gpui-kit` | `=0.9.0`（workspace pin） | crates.io | GPUI 与应用层 |
| `image` | `=0.25.10`（`default-features = false`，只开 `png`） | crates.io | 读写 PNG |
| `serde` / `serde_json` | 锁文件中的 `1.x` | crates.io | 布局与报告 JSON |
| `tterm-ui` | 本地 crate | `crates/tterm-ui` | 与 React 同源的 design tokens |

首次安装：

```sh
pnpm install
pnpm exec playwright install webkit
```

`playwright` 与浏览器是两件事：`pnpm install` 只装 JS 包，浏览器要单独下载。缓存目录在 `~/Library/Caches/ms-playwright/`（macOS）；本机已有 `webkit-2191` 时第二条命令立即返回。没有网络或缓存时，参考端会报错并提示这条安装命令；GPUI 端与 `capture` 子命令不受影响。

若 pnpm 输出 `ERR_PNPM_IGNORED_BUILDS`（本仓库的 esbuild 安装脚本被 pnpm 的构建脚本策略忽略），按项目规则如实记录，不通过修改配置绕过。esbuild 的平台二进制由可选依赖提供，Vite 与 Playwright 在本机可正常工作。

## 3. 常用命令

在仓库根执行：

```sh
# 生成/复用参考截图 + 渲染 GPUI + 比较，写 HTML 与 JSON
cargo run -p tterm-parity -- compare

# 强制重新生成当前 React 提交的参考截图
UPDATE_REFERENCE=1 cargo run -p tterm-parity -- compare

# 只渲染 GPUI 端，不碰参考端（写视图时的快循环）
cargo run -p tterm-parity -- capture

# 手动打开参考端页面（需要在另一个终端）
pnpm dev:parity
# http://127.0.0.1:1423/parity.html?screen=example.button-input.default&theme=dark&scale=100
```

`compare` 的退出码：`0` 每一屏都过门槛，`1` 至少一屏不通过，`2` 工具或输入出错（缺参考、缺 Playwright、JSON 解析失败等）。失败时终端会打印每一屏的布局最大偏差、颜色差、SSIM、热力图路径和逐条失败原因，失败原因带元素 id。

参考端单独运行：

```sh
node scripts/parity/capture.mjs --out target/parity/reference/dev --commit dev
node scripts/parity/capture.mjs --screen example.button-input.default
```

`--out` 默认是 `target/parity/reference/<前端提交>`；前端有未提交改动时默认目录变成 `<前端提交>-dirty`，并打印所用目录。`--commit` 默认由脚本自己从 git 读；`--screen` 可重复，只截指定屏幕。直接运行时 dirty 目录不会自动清空，`compare` 会先清空再调用它。

## 4. 参考端缓存

缓存键是**最后一次改动参考前端的提交**，路径列表见 `crates/tterm-parity/src/reference.rs` 的 `FRONTEND_PATHS`：

```text
src parity/screens.json parity/fixtures scripts/parity package.json
pnpm-lock.yaml vite.config.ts tsconfig.json index.html parity.html
```

`parity/README.md` 和 `docs/` 不参与缓存键，改文档不会触发参考图重生成。

- 干净工作区：同一提交的第二次 `compare` 直接复用 `target/parity/reference/<hash>/`，不重新启动浏览器。复用前校验 `manifest.json` 的 `version` 为 2，且每个布局 JSON 都能按 schema 2 读取；版本不符就当作过期缓存重生成。
- **前端有未提交改动时不再静默用缓存。** `FRONTEND_PATHS` 下只要有一处改动，参考图写到 `target/parity/reference/<hash>-dirty/`，每次 `compare` 先删掉该目录再重新生成，不复用；终端会打印 `reference: the frontend is dirty; ...` 并说明用的是 dirty 参考。
- `UPDATE_REFERENCE=1` 会先删掉当前参考目录再重新生成，避免旧屏幕残留；干净与 dirty 都适用。
- 参考截图、布局 JSON、`manifest.json` 都在该目录；它们不提交。生成后工具会从 PNG 实测每个元素的背景/边框色，把实测值写回布局 JSON，同时保留 `declared*` 字段（见第 6 节）。
- 报告里的参考路径是相对路径 `../reference/<hash>[-dirty]/<屏幕>.png`，从 `target/parity/report/index.html` 打开即可看到。

`compare` 总是重新渲染 GPUI 端（写 `target/parity/actual/`），所以 GPUI 代码一改就能看到结果，不受参考缓存影响。

## 5. 加一屏

1. 在 `parity/screens.json` 的 `screens` 数组里加一项：

   ```json
   {
     "id": "settings.appearance.default",
     "title": "Settings / Appearance / Default",
     "fixture": "settings.appearance.default.json"
   }
   ```

   id 规则是 `<功能>.<视图>.<状态>`。当前 `uiScalePercent` 只接受 100、theme 只接受 `dark` 与 `light`；其它组合要在屏幕视图里补对应的字号/主题实现。

2. 在 `parity/fixtures/` 写同名夹具。键是 `commands`，值是该 command 的返回数据。`src/parity/fixtures.ts` 里用真实类型标注：

   ```ts
   import type { SavedProfile } from "@/types/tab"
   import settingsFixture from "@fixtures/settings.appearance.default.json"

   export const fixtures = {
     "settings.appearance.default": {
       commands: {
         list_profiles: settingsFixture.commands.list_profiles satisfies SavedProfile[],
       },
     },
   }
   ```

   夹具值可以是下表四种形状之一。`args` 用递归深比较，键顺序无关；列表按顺序找第一个全等匹配，匹配不到按缺夹具处理。空数组是普通空值，不会被当成 args 列表。

   | 夹具值 | 行为 |
   | --- | --- |
   | 普通 JSON 值 | `resolve` 返回它的深拷贝 |
   | `{ "$reject": "错误文本" }` | `reject(new Error("错误文本"))`，用于错误状态 |
   | `{ "$pending": true }` | 返回永不 resolve 的 Promise，用于加载状态 |
   | `[{ "args": {...}, "value": ... }, ...]` | 按调用参数选择 `value`；每个 `value` 仍可再是普通值或上面两种指令 |

   没有夹具、没有匹配的 args、或屏幕尚未激活夹具时，`invoke` 会抛错并列出已有 command；同时把 command 记进 `window.__PARITY_MISSING__`。即使组件 `catch` 了错误继续渲染，`capture.mjs` 也会在截图前后检查该列表并让这一屏失败，避免截出一张“空状态但看起来正常”的图。`src/parity/tauri/missing-fixture.test.tsx` 覆盖了组件 catch 的情况。

3. 在 `src/parity/screens.ts` 注册组件。组件接收 `{ onReady }`：夹具异步加载完成后再调 `onReady()`；没有异步数据的屏幕在 `useEffect` 里直接调用。入口在字体就绪和两帧之后再置 `window.__PARITY_READY__`。

4. 在需要比对的元素上加 `data-parity="<id>"`。尺寸随文字变化的元素再加 `data-parity-sizing="text"`（例如自适应宽度的按钮）；其余默认按固定尺寸比较。只加属性，不改行为。

5. 在 `crates/tterm-parity/src/screens/` 加对应的 GPUI 视图，并在 `screens/mod.rs` 的 `ScreenView` 里注册。每个标记元素用 `parity_element(sink, id, sizing, style, |view| ...)` 包一层：`apply_snapshot` 先按 `style` 设置 padding、边框宽度、圆角、字体、字号、字重、行高和文字色，builder 只加结构以及背景色/边框色。这样“画出来的”和“记下来的”对不可测字段是同一个值，不会一边写 16、一边画 17。文字用 `parity_text_for(sink, key, text, &style)`（或独立标签用 `parity_text`），它会把文字自身边界记进 `textRegions`。

   `background_color` / `border_color` 不再由快照自报：`capture` 和参考端生成后都会从 PNG 实测（见第 6 节）。`style` 里仍要填这两个字段，它们只作为 `declared*` 的声明值，用于校验“代码想画的颜色”和“像素实际颜色”是否一致；**声明值只能证明代码意图，不能证明渲染结果**。

6. 运行 `cargo run -p tterm-parity -- compare`，确认该屏通过；参考端截图与布局 JSON 在 `target/parity/reference/`，GPUI 端在 `target/parity/actual/`。

`pnpm test` 里的 `src/parity/fixtures.test.ts` 会检查清单、注册表、夹具三者是否一致；漏注册或漏夹具会让测试失败。

## 6. 布局 JSON 契约

两端写出同一形状（`version` 为 2）。`parity/screens.json` 也是 version 2，并带固定的 `platform` 块：

```json
{
  "version": 2,
  "screen": "example.button-input.default",
  "viewport": { "width": 800, "height": 600, "deviceScaleFactor": 2 },
  "elements": [
    {
      "id": "example.button",
      "sizing": "text",
      "bounds": { "x": 328, "y": 64, "width": 63.75, "height": 36 },
      "style": {
        "backgroundColor": [0, 112, 204, 255],
        "borderColor": [61, 61, 61, 255],
        "declaredBackgroundColor": [0, 112, 204, 255],
        "declaredBorderColor": [61, 61, 61, 255],
        "borderWidth": 0,
        "borderRadius": 4,
        "fontFamily": "system-ui",
        "fontSize": 14,
        "fontWeight": 500,
        "lineHeight": 16,
        "color": [255, 255, 255, 255],
        "padding": { "top": 8, "right": 16, "bottom": 8, "left": 16 }
      }
    }
  ],
  "textRegions": [{ "x": 344, "y": 73, "width": 31.74, "height": 17 }]
}
```

- **单位**：逻辑像素（CSS px），坐标相对窗口左上角。物理像素 = 逻辑像素 × `deviceScaleFactor`。
- **bounds**：元素的 border box，包含边框与内边距。
- **颜色是实测值**：`backgroundColor` 和 `borderColor` 由工具在 PNG 上按元素边界取众数，不是从 DOM 或快照抄来的。背景色取圆角矩形向内缩进 `边框宽度 × DPR + 1` 物理像素后的像素众数，并排除按 DPR 缩放、再膨胀 2 物理像素的文字区域；边框色只在 `borderWidth > 0` 时取四个直边的中段像素众数，角上的圆角段不参与。任何一边测不到都报错并带元素 id。
- **声明值**：`declaredBackgroundColor` / `declaredBorderColor` 是绘制代码当时拿到的 token 值（React 侧是 `getComputedStyle` 的读回值，GPUI 侧是 `StyleSnapshot`）。两者只在测量后写入；不透明声明值必须与实测值逐通道 8 位相等，否则该屏失败，并明确写“声明的颜色和像素不一致”。半透明声明值不参与这条相等检查，仍按下面的 CIEDE2000 规则比较。
- **颜色格式**：8 位 sRGB 的 `[r, g, b, a]`。React 侧用 1×1 canvas 读回，所以 `hsl()`、`rgba()`、`color-mix()`、`color(srgb …)` 都会落到同一形式；GPUI 侧用 `tterm_ui` 的 HSL token 按 CSS 相同的取整规则转换。
- **无边框元素**：`borderWidth` 为 0 时没有可测的边框带，`borderColor` 保留声明值，比较时跳过边框颜色。
- **fontFamily**：两端都归一成 `system-ui`。React 的 `-apple-system` 与 GPUI 的 `.SystemUIFont` 在 macOS 上是同一套系统字体；中文用系统回退。
- **lineHeight**：必须是像素值（CSS 的 `normal` 会报错）。GPUI 绘制时会把行高取整到物理像素，所以夹具屏幕用整数行高；本示例是 16px。
- **textRegions**：需要从图像比较里遮掉的文字矩形。React 侧收集所有文本节点的 `Range.getClientRects()` 和每个 input/textarea 的内容区；GPUI 侧由 `parity_text` / `parity_text_for` 记录文字元素自身边界。比较时取两端的并集，并按 2 个物理像素膨胀。
- **`invokedCommands`**：React 侧额外写出被 mock 的 `invoke` 实际回答过的 command（去重排序），作为“夹具覆盖了界面实际调用”的证据；GPUI 侧不写，比较时忽略。示例屏幕的输出应包含 `["list_profiles"]`。

## 7. 比较规则与门槛

| 检查 | 规则 |
| --- | --- |
| 布局，固定尺寸元素 | `x`、`y`、`width`、`height` 各自偏差 ≤ 1 逻辑像素 |
| 布局，文字尺寸元素（`sizing: "text"`） | 偏差 ≤ `max(2 逻辑像素, 参考宽度的 2%)` |
| 元素集合 | 两端 id 必须一一对应；多一个或少一个都失败 |
| 样式 | 字体族、字号、字重、行高、圆角、边框宽度完全相等（浮点容差 0.01px）；内边距按 0.01px 比较（比 5.2 节更严，见“限制”） |
| 元素颜色（实测） | 不透明颜色逐通道 8 位相等；带 alpha 时先叠在屏幕左上角（屏幕背景）上，再要求 CIEDE2000 ≤ 1 |
| 声明颜色（快照/计算样式） | 不透明声明值必须与实测值逐通道 8 位相等；不一致即失败，防止“改绘制、不改快照” |
| 图像，遮住文字 | SSIM ≥ 0.99 |
| 图像，整屏 | SSIM ≥ 0.97 |
| 整屏颜色门槛 | 遮掉文字区域和每个标记元素的边框带 + 两侧各 1 物理像素抗锯齿带后，CIEDE2000 > 1 的像素数必须为 0 |
| 颜色差（文字遮罩，报告项） | 未遮罩像素的 CIEDE2000 均值、最大值、>1 的像素数；不单独设门槛 |

整屏颜色门槛的遮罩由 `image_diff::screen_mask` 生成：文字遮罩取两端 `textRegions` 的并集并按 2 物理像素膨胀；元素边缘按两端的 `bounds`/`borderRadius`/`borderWidth` 用圆角矩形 SDF 标记 `|sdf| ≤ 边框宽度 × DPR + 1` 的物理像素，把边框本身和两侧抗锯齿都算进边缘带。门槛是 0 个超限像素（`SCREEN_COLOR_MAX_PIXELS = 0`）；若将来确实需要放宽，必须把理由写进本文档，而不是悄悄改常量。

SSIM 的实现：灰度取 `0.2126R + 0.7152G + 0.0722B`（gamma 编码值），按不重叠的 16×16 窗口计算标准 SSIM（`C1 = (0.01·255)²`、`C2 = (0.03·255)²`），取窗口平均。遮罩版本跳过文字像素；一个窗口剩余不足 32 个未遮罩像素时整个窗口跳过。这是本工具自己的实现，换实现得到的数值会略有不同，但门槛与结论以 `compare` 的输出为准。

热力图是逐像素“两图通道差的最大值”放大 8 倍后映射到白→红：白色表示完全一致，越红差得越多。文字差异也会出现在热力图里，与遮罩 SSIM 是两回事。

## 8. 输出

`target/parity/report/result.json`：

```json
{
  "schemaVersion": 2,
  "reference": { "frontendCommit": "…", "dirty": false },
  "thresholds": { "fixedPx": 1.0, "textPx": 2.0, "textFraction": 0.02,
                  "maskedSsim": 0.99, "fullSsim": 0.97, "screenColorPixels": 0 },
  "screens": [
    { "id": "example.button-input.default", "title": "…", "passed": true,
      "layout": { "passed": true, "elements": [ … ], "failures": [] },
      "colors": { "meanDeltaE": 0.0001, "maxDeltaE": 2.6153, "pixelsAboveOne": 52 },
      "screenColors": { "meanDeltaE": 0.0, "maxDeltaE": 0.82, "pixelsAboveOne": 0 },
      "ssim": { "full": 0.998504, "masked": 1.0 },
      "heatmap": "heatmaps/example.button-input.default.png",
      "images": { "reference": "../reference/<hash>[-dirty]/example.button-input.default.png",
                  "actual": "../actual/example.button-input.default.png" },
      "failures": [] }
  ],
  "passed": true
}
```

`index.html` 是同内容的可读版本：每屏并排参考图、GPUI 图、热力图，以及逐元素的边界差和失败原因。报告不含时间戳、绝对路径或随机数，同一输入重复运行得到逐字节相同的 `result.json` 和热力图（示例的两次运行 SHA-256 见 W0-04 报告）。

## 9. 限制与已知差异

- **只在 macOS 自动出图。** React 端固定用 Playwright WebKit（macOS 上同引擎、同 CoreText），GPUI 端用 `HeadlessAppContext` 的 Metal 渲染。Windows/Linux 的截图与并排检查按 ui-parity 5.3 节由人完成，本工具只提供布局 JSON 的三平台生成路径。
- **Playwright 的 WebKit 不是 Tauri 的 WKWebView。** 同一引擎与字体栈，但没有 Tauri 注入的窗口装饰、透明与模糊；窗口级效果用 `[人工]` 在真实 Tauri 窗口里核一次（ui-parity 5.1 节）。W0-04 的示例屏幕只包含普通控件。
- **文字渲染只要求接近。** 字体、字号、字重、行高与位置进入布局比较，实际字形由两端各自光栅化；文字像素通过 `textRegions` 从图像比较里遮掉，整屏 SSIM 仍会包含文字差异，因此门槛是 0.97 而不是 0.99。
- **GPUI 的 `debug_selector` 不能直接用于无头截图。** 按 P0-25 第 9 项，`debug_selector` 只在 `cfg(any(test, all(debug_assertions, feature = "test-support")))` 下记录（`gpui-pre-0.3.11/src/elements/div.rs:901-920`，不满足时是空操作 `:922-925`），并且只有 `VisualTestContext::debug_bounds` 能读；`HeadlessAppContext` 没有读取入口。本工具改为在标记元素外包一层 `Div`，用 `on_children_prepainted` 读取唯一子元素的 border box；任何渲染模式都可用，也能拿到文字元素自身边界。构建前提与 P0-25 第 6 条限制一致。
- **实测与声明是两类证据。** `backgroundColor` / `borderColor` 是从 PNG 取出的实测值，比较也以实测值为准；`padding`、`fontFamily`、`fontSize`、`fontWeight`、`lineHeight`、`borderRadius`、`borderWidth` 无法从像素反推，绘制与记录共用 `apply_snapshot`，但结构上仍只能证明“代码把同一个值用于绘制和记录”，不能独立证明渲染引擎没有改它。声明颜色只用于交叉校验绘制意图，不能代替实测。
- **界面缩放只支持 100%。** 字号、行高、间距的 125% 组合要等 W0-05 的夹具覆盖后再开放；清单校验会拒绝其它值，避免悄悄用 100% 的 token 截 125% 的图。
- **元素级毛玻璃、圆角裁剪子内容、径向渐变不参与图像比较。** 这些是 ui-parity 第 4 节的替代项；W0-04 没有示例。后续屏幕要在 `parity/exceptions.md` 登记（该文件由 W0-05 建立）。
- **内边距比较是扩展。** 5.2 节要求的是位置、尺寸与样式表里的字体/圆角/边框。W0-04 额外比较 padding，因为“把按钮内边距改 1px”是本卡的验收项，而 2px 的宽度变化可能落在文字尺寸元素的 2px 容差内。该比较只会更严格，不会放宽门槛。
- **alpha 颜色的合成基准是屏幕左上角像素。** 示例屏幕背景单色，等价于叠在页面背景上；嵌套在非均匀背景里的半透明元素需要 W0-05 再细化，当前会按屏幕背景判断并写入报告。

## 10. 故障排除

| 现象 | 处理 |
| --- | --- |
| `Could not start Playwright's WebKit` | `pnpm exec playwright install webkit`；确认 `playwright` 是 1.54.0 |
| `Vite started without a local URL` / 端口占用 | 抓图脚本用随机端口，不占用 1420/1423；手动 `pnpm dev:parity` 用 1423 |
| `reference capture failed` 且提到 `page error` | 打开 `pnpm dev:parity` 的 URL，看控制台；通常是屏幕没有调 `onReady` 或夹具缺 command |
| `Parity mock: no fixture for "…"` | 在 `parity/fixtures/*.json` 补该 command，并在 `src/parity/fixtures.ts` 用真实类型标注 |
| 参考截图过期 | 干净工作区设置 `UPDATE_REFERENCE=1`；前端有未提交改动时会自动使用 `<hash>-dirty` 并每次重生成 |
| `missing fixture(s) for: …` | 组件 catch 了没有夹具的 invoke；按列出的 command 补夹具，不要靠 catch 掩盖 |
| `image size 1600x1200 -> …` | GPUI 无头窗口的缩放不是 2；检查 `HeadlessAppContext` 的平台与清单的 `deviceScaleFactor` |
| 字体或行高偏差 | 屏幕必须显式设置整数行高；字体族两端都归一成 `system-ui` |

## 11. 复现 W0-04 的验收

```sh
# 1. 正常对比（应有 1 屏通过）
cargo run -p tterm-parity -- compare

# 2. 同一输入两次，比较 result.json 的哈希
cargo run -p tterm-parity -- compare
shasum -a 256 target/parity/report/result.json
cargo run -p tterm-parity -- compare
shasum -a 256 target/parity/report/result.json

# 3. 故意把 crates/tterm-parity/src/screens/example.rs 的
#    BUTTON_PADDING_X 从 16.0 改成 17.0，再运行，应退出 1 并指出 example.button；
#    改回 16.0 后应再次通过。
cargo run -p tterm-parity -- compare

# 4. 故意把 example.rs 里按钮的 .bg(...) 改成 gpui_kit::rgb(0x963c50)，
#    StyleSnapshot 不动，再运行，应退出 1，并指出声明颜色与实测像素不一致；
#    改回后应再次通过。
cargo run -p tterm-parity -- compare
```

示例的参考布局、GPUI 布局、实际截图和热力图在 `docs/gpui-migration/reports/assets/W0-04/` 有一份证据副本；完整的 HTML 报告与 `result.json` 在 `target/parity/report/`。
