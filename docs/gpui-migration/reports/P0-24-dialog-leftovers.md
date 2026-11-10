# P0-24 Dialog 低优先级遗留修复（macOS）

## 结论

完成待评估。三个低优先级问题各用一个逻辑提交修复，并保留修复撤回时失败、恢复后通过的回归测试日志。改动前后均从干净工作区在沙箱外重新生成 WebKit 参照，深浅主题各 16/16 屏通过，四个 Dialog 屏的颜色门槛计数均为 0，full / masked SSIM 完全相同。未合并、未 push。

环境：Mac mini M4、macOS Apple Silicon、DPR 2，100% UI scale；WebKit revision 2191、Playwright 1.54.0。使用本 worktree 的 `target/`，没有设置共享 `CARGO_TARGET_DIR`；所有编译命令均设 `CARGO_BUILD_JOBS=3`。没有运行正式 Tauri 应用、读取真实连接数据或阅读 Zed 源码。

## 改动

| 提交                                                                     | 原因与修法                                                                                                                                                                                                                                                                     | 测试                                                                                                                                                                  |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0b413d75` `:bug: keep non-breaking spaces together in dialog text`      | 原先空格回溯使用 `char::is_whitespace`，包含 U+00A0、U+2007、U+202F。回溯现在排除这三种不换行空白；若 GPUI 原生断行落在包含它们的词内，移到该词之前，若该词自身已超宽，则整词溢出。保留原生 shaping、精确宽度与普通词的断行。                                                  | `non_breaking_spaces_stay_with_the_adjacent_word`、`other_non_breaking_spaces_stay_with_the_adjacent_word`、`oversized_non_breaking_words_overflow_without_splitting` |
| `085986e2` `:bug: center narrow multiline dialog title rows like WebKit` | Confirm、Destructive、Info 的标题继承左对齐，且文字直接作为 flex 子项，不能正确按剩余宽度自动折行。增加可收缩的文字容器；仅在 viewport < 640px 且标题实际多于一行时逐行居中，绘制与观察共用同一对齐值。单行 Confirm/Destructive/Info 保持原有行首，Prompt 沿用其标题居中行为。 | 四个 `narrow_multiline_{confirm,destructive,info,prompt}_title_is_centered_per_row`；已有四个单行标题位置测试                                                         |
| `499592c1` `:bug: replace dialog text row records on each layout`        | HashMap 位于 Dialog 测试探针，parity 的 BTreeMap 也有同样的旧行残留问题。TextObserver 改为一次报告当前 slot 的全部行，探针先移除该 slot 的旧行再记录本次行；LayoutSink 增加按 slot 替换的方法并由 Dialog 屏调用。空行集合也会清除旧记录。                                      | `text_rows_are_replaced_when_description_shrinks`、`replacing_text_rows_removes_old_rows_and_preserves_other_slots`                                                   |

主要文件：`crates/tterm-ui/src/components/dialog/{text.rs,presentation.rs,tests.rs}` 与 `crates/tterm-parity/src/{layout.rs,layout/tests.rs,screens/dialog.rs}`。没有新依赖、生产 CSS 改动、夹具文案改动、门槛改动、遮罩扩展或新例外。未新增 parity 屏，组件测试已逐行断言实际 prepaint 坐标，并在 `base::Root` 下运行。

## WebKit 实测证据

可复现源码：[生产 hook 夹具](assets/P0-24-dialog-leftovers/probe.tsx.txt)、[WebKit/Vite 驱动](assets/P0-24-dialog-leftovers/webkit.mjs.txt)。原始输出：[webkit.json](assets/P0-24-dialog-leftovers/webkit.json)。复现时将两个 `.txt` 文件分别保存回本 worktree 的 `target/dialog-leftovers-evidence/probe.tsx` 与 `webkit.mjs`，从根目录在沙箱外执行 `node target/dialog-leftovers-evidence/webkit.mjs`。脚本用 Vite parity 模式加载真实 `app-dialog.tsx` hooks 与生产 CSS；在 `finally` 中关闭浏览器和 Vite。夹具只提供文案和变体，没有覆盖生产样式或写死颜色。

### 不换行空白

在 WebKit 上以生产描述的 `white-space: pre-line`，系统字体 14px、声明行高 16.8px、可用宽度 100px 实测；对每个字符分别测 `Alpha<字符>Beta Gamma` 与 `Alpha<字符>BetaGamma`。实际行盒每行高 16px：

| 字符                         | 含普通空格的字符串                         | 超宽连接词               | 结论                            |
| ---------------------------- | ------------------------------------------ | ------------------------ | ------------------------------- |
| U+00A0 NO-BREAK SPACE        | 两行，行宽 69.9248046875 / 49.5126953125px | 一行，宽 119.4375px      | U+00A0 前后不拆行；超宽整词溢出 |
| U+2007 FIGURE SPACE          | 两行，行宽 74.8056640625 / 49.5126953125px | 一行，宽 124.318359375px | 同上，不能视为普通可折叠空格    |
| U+202F NARROW NO-BREAK SPACE | 两行，行宽 67.9560546875 / 49.5126953125px | 一行，宽 117.46875px     | 同上，不能视为普通可折叠空格    |

测试在正常宽度下验证连接词与相邻文字保留整行，在内容宽度 100px 的窄窗口中验证三种字符的连接词宽度大于容器、描述只有 16px 高且不存在第二行。既有普通空格行尾、显式换行与多行描述测试继续通过。

### 标题与断点

先读 `src/components/ui/app-dialog.tsx`、`dialog.tsx`：Confirm/Destructive/Info 的 `DialogTitle` 是 `flex items-center gap-2`，其中真实文本成为匿名 flex 项；Prompt 是普通标题。`DialogHeader` 为 `text-center sm:text-left`，`sm` 断点为 640px。单行匿名 flex 项宽度随文字变化，因此观察单行行首左对齐不能推断多行文字的内部对齐。

实测通过真实 `useConfirmDialog`、`useInfoDialog`、`usePromptDialog` 打开 portal，DOM Range 只取标题文本节点，记录每一行：

| viewport | Confirm / Prompt 各行 x 中点 | Destructive / Info 各行 x 中点 | 结果                                     |
| -------- | ---------------------------- | ------------------------------ | ---------------------------------------- |
| 500px    | 250.000 / 250.000px          | 258.125 / 258.125px            | 全部逐行居中；带图标时文本区域在图标右侧 |
| 639px    | 319.500 / 319.500px          | 329.023 / 329.023px            | 全部逐行居中                             |
| 640px    | 行首均 121px                 | 行首均 136.84375px（三行）     | 全部逐行左对齐                           |
| 800px    | 行首均 201px                 | 行首均 216.84375px（三行）     | 全部逐行左对齐                           |

测试使用足够长的标题触发自动折行，遍历 500、639、640、800px，逐行比较 x 中点（< 640px）或行首 x（≥ 640px）；不是通过显式换行代替自动折行。既有单行测试 `narrow_confirm_title_stays_left`、`narrow_destructive_title_stays_left_after_icon`、`narrow_info_title_stays_left_after_icon`、`narrow_prompt_title_is_centered` 继续通过。

## 先失败、后通过

所有测试经生产 DialogOptions 路径并在 `base::Root` 下执行。以下日志均使用本 worktree 的 target；反向检查只临时撤回对应修复，随后恢复，没有提交故障代码。

| 项目       | 失败证据                                                                                                                                                                                                                                                                                   | 修复后证据                                                                                                                     |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| 不换行空白 | [nb-red.log](assets/P0-24-dialog-leftovers/nb-red.log)：撤回不换行空白修复后，超宽连接词测试失败，实际首行宽度没有超过容器                                                                                                                                                                 | [nb-green.log](assets/P0-24-dialog-leftovers/nb-green.log)：三个相关测试通过                                                   |
| 多行标题   | [title-red.log](assets/P0-24-dialog-leftovers/title-red.log)：原实现不能按宽度自动产生第二行；[title-alignment-red.log](assets/P0-24-dialog-leftovers/title-alignment-red.log)：保留折行容器、仅撤回居中逻辑，Confirm、Destructive、Info 的逐行中点断言失败；Prompt 由原有居中样式保持通过 | [title-alignment-green.log](assets/P0-24-dialog-leftovers/title-alignment-green.log)：四个变体、四种宽度通过                   |
| 旧行记录   | [rows-red.log](assets/P0-24-dialog-leftovers/rows-red.log)：先两行后改成一行，旧 `description-1` 仍在 HashMap，断言失败                                                                                                                                                                    | [rows-green.log](assets/P0-24-dialog-leftovers/rows-green.log)：同一组件更新后只留下本次行；完整测试同时覆盖 parity 的替换方法 |

最终 Dialog 测试共 36 项通过，见 [dialog-green.log](assets/P0-24-dialog-leftovers/dialog-green.log)；完整 workspace 日志包含 LayoutSink 替换、主题隔离、Root 焦点与所有既有测试。

## 改动前后对比

改动前在起始提交 `a1957a55`，改动后在实现提交 `499592c1`。两次比较前均确认 `git status --porcelain` 为空；先提交实现，再重新生成参照。每阶段均在沙箱外执行：

```sh
UPDATE_REFERENCE=1 PARITY_THEME=dark CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
# 保存 target/parity/report/result.json 后，再执行浅色。
UPDATE_REFERENCE=1 PARITY_THEME=light CARGO_BUILD_JOBS=3 cargo run -p tterm-parity -- compare
```

四份结果的 `reference.frontendCommit` 均为 `5ed24329cc0f95144fac0824e586b69b99b088eb`（最后一次前端变更），`reference.dirty=false`。每次均强制生成，不复用旧参照。原始结果与命令日志：[before-dark.json](assets/P0-24-dialog-leftovers/before-dark.json)、[before-light.json](assets/P0-24-dialog-leftovers/before-light.json)、[after-dark.json](assets/P0-24-dialog-leftovers/after-dark.json)、[after-light.json](assets/P0-24-dialog-leftovers/after-light.json)，同目录 `before/after-{dark,light}.log`。

下表直接来自这些 `target/parity/report/result.json` 的 `screenColors.pixelsAboveOne`，不是诊断字段 `colors.pixelsAboveOne`：

| Dialog 屏               | 深色改前 → 改后门槛计数 | 浅色改前 → 改后门槛计数 | 深色 full / masked（前后相同） | 浅色 full / masked（前后相同） |
| ----------------------- | ----------------------- | ----------------------- | ------------------------------ | ------------------------------ |
| `ui.dialog.confirm`     | 0 → 0                   | 0 → 0                   | 0.996235 / 0.999945            | 0.995766 / 0.999756            |
| `ui.dialog.destructive` | 0 → 0                   | 0 → 0                   | 0.995947 / 0.999942            | 0.995490 / 0.999710            |
| `ui.dialog.prompt`      | 0 → 0                   | 0 → 0                   | 0.990700 / 0.999934            | 0.989962 / 0.999729            |
| `ui.dialog.multiline`   | 0 → 0                   | 0 → 0                   | 0.992639 / 0.999945            | 0.992282 / 0.999748            |

四个 Dialog 屏深浅均 `passed=true`，布局 failures 为 0，所有图标检查通过，原有 E-001 不变。其他 12 屏也全部通过；未修改比较器、生产 CSS、颜色、遮罩范围、门槛或例外。四屏逐项结果与门槛指标前后相同，没有一屏变差。截图、布局与热力图可从 JSON 中的原路径在 `target/parity/` 查阅，两主题 actual/reference 另附于证据目录的 `dark/`、`light/`。

## 检查

AGENTS.md 全部规定的检查已通过：

```sh
pnpm lint && pnpm format:check && pnpm check:tokens && pnpm typecheck && pnpm test && pnpm check:size
CARGO_BUILD_JOBS=3 cargo fmt --all --check
CARGO_BUILD_JOBS=3 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=3 cargo test --workspace
```

- 前端 lint、Prettier、tokens、TypeScript 全部通过；Vitest **175 文件、1295 测试通过**；parity layout 脚本 **2 测试通过**；size **1060 文件通过**。未改变大小基线。[执行记录](assets/P0-24-dialog-leftovers/frontend-checks.md)。
- Rust fmt、clippy 全部通过；完整 workspace **1222 passed、0 failed、0 filtered**，原有 **52 ignored** 按仓库默认标记未执行。本轮没有新增 ignored，没有命令行过滤、skip、排除 package 或弱化检查。日志：[clippy.log](assets/P0-24-dialog-leftovers/clippy.log)、[workspace-tests.log](assets/P0-24-dialog-leftovers/workspace-tests.log)。
- 初次沙箱内 workspace 测试只有已知应用枚举 `macos_lists_the_applications_for_text_with_icons` 失败；之后在沙箱外完整复跑所有 workspace 测试，该项通过。WebKit 对比直接在沙箱外运行。
- 诊断浏览器与 Vite 均已关闭；`ps` 复核没有本任务遗留进程。没有启动持续运行的 GPUI 示例窗口。

## 偏差

没有需要扩大范围或确认的偏差。第 3 项提到的 HashMap 实际位于测试探针；生产观察者此前逐行回调，而 parity 收集器的 BTreeMap 也会保留旧行。本轮在同一行区域记录链上改为按 slot 替换，使两端诊断都保持本次布局。

未新增对比屏：四个标题变体和断点的逐行坐标已由 Root 组件测试与真实 React hook 的 WebKit DOM Range 实测覆盖；全部既有 Dialog 屏仍重跑深浅比较。

## 未验证与 [人工]

- Windows 通道：**未验证（需 Windows，决策 016 待补）**。
- [人工] 入场动画、真实中文输入法、系统辅助功能、真实 Tauri 窗口交叉截图：沿用 P0-24 既有待补项，本轮没有条件完成。
- 仓库已有 52 个 ignored 测试需要其原本要求的外部服务或设备，未执行，不将它们计为通过。
