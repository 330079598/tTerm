# P4 业务功能与设置界面对齐

目标：逐项补齐业务界面，直到功能矩阵（`feature-matrix.md`）中的每一项都是“对齐”或经人确认的“明确放弃”。迁移计划要求：尚未实现的功能明确标记，不能因为存在相似页面就视为对齐。

前提：G2 与 G3 通过；P3-06 的基础组件已稳定。各卡之间基本独立，适合多个执行者同时进行。

## 共同要求

每张卡都需要：

1. **功能清单先行。** 开始写代码前，先通读负责的前端目录，在报告中列出所有功能点：每个按钮、菜单项、对话框、快捷键、校验规则、错误提示、空状态与加载状态。评估者先审清单，再审实现。
2. **调用同一套服务。** 业务逻辑只在 core 中，界面不直接操作网络、数据库或文件。发现 TS 端有尚未移入 core 的逻辑时，先移入 core（参照 P1-11 的做法，带测试），再写界面。
3. **文案全部走 i18n**，使用现有的键；需要新键时同时加到 `en.json` 与 `zh.json`。
4. **视图规模**遵守架构约定 A10。
5. **验收证据**：功能清单逐项打勾，附截图或录屏；与 Tauri 版的差异逐项说明。

## 规模参考

括号内为现有前端目录的行数（不含测试），用于估算，不代表 GPUI 实现的行数。

| 卡             | 范围                 | 现有规模                                                                        |
| -------------- | -------------------- | ------------------------------------------------------------------------------- |
| P4-01          | 连接列表与分组       | `ProfilesPanel/`（约 880）、`SettingsDialog/ProfileGroups*` 等                  |
| P4-02          | 连接编辑对话框       | `ConnectionDialog/`（约 4200）                                                  |
| P4-03          | SSH 配置导入         | `SshConfigImportDialog/`（约 560）                                              |
| P4-04          | 认证与主机密钥界面   | `TerminalTab/` 中的对话框、`ProxyCommandPrompt/`                                |
| P4-05          | SFTP 浏览器          | `SftpDrawer/`（约 8500）                                                        |
| P4-06          | 传输管理器           | `TransferManager/`（约 870）、`TransferContext.tsx`                             |
| P4-07          | 远程文件编辑器       | `RemoteFileEditor.tsx`、`CodeMirrorEditor.tsx`（约 520）                        |
| P4-08          | 隧道                 | `TunnelsPanel/`（约 1950）                                                      |
| P4-09          | 服务器监控           | `TerminalTab/ServerMonitor*` 等                                                 |
| P4-10          | 串口                 | `TerminalTab/Serial*`、`ConnectionDialog/Serial*`                               |
| P4-11          | 日志与回放           | `LogsPanel/`（约 1440）                                                         |
| P4-12          | 命令库、编写器与变量 | `CommandLibrary/`（约 2060）、`CommandComposer/`（约 580）、`CommandVariables*` |
| P4-13          | 广播输入             | `BroadcastManager.tsx`、`TTermApp/useBroadcast*`                                |
| P4-14          | 主题库、编辑与导入   | `ThemeEditor/`（约 1550）、`ThemeGallery.tsx`、`ThemeImportDialog.tsx` 等       |
| P4-15 至 P4-26 | 设置                 | `SettingsDialog/`（约 16200）                                                   |
| P4-27          | 功能对齐审核         | 全部                                                                            |

## P4-01 连接列表与分组

- 依赖：G3
- 规模：M
- 参考：`src/components/ProfilesPanel/`、`SettingsDialog/ProfileGroupsSettingsTab.tsx`、`ProfileGroupTree.tsx`、`ProfileGroupCard.tsx`、`ProfileDragRow.tsx`、`profileGroupDnd.ts`、`useProfileGroup*.ts`、`useProfileListActions.ts`、`useProfileSelection.ts`、`profileDeletion.ts`、`TTermApp/useProfileActions.tsx`、`useProfileRecents.ts`、`useConnectionIcons.ts`、`src/lib/brandLogos.ts`、`SimpleIcon.tsx`
- 工作：连接列表、搜索、排序、分组树、拖动分组、多选与批量删除、最近使用、连接图标与品牌标志。
- 验收：共同要求；分组拖动的规则与 `profileGroupDnd` 的测试一致。

## P4-02 连接编辑对话框

- 依赖：P4-01
- 规模：L，开始前拆成子提交
- 参考：`src/components/ConnectionDialog/` 全部文件
- 工作：建议按以下顺序拆分：
  1. 对话框框架、SSH 基本字段、表单校验与无效字段定位（`useInvalidFieldReveal.ts`）、保存（`useConnectionSubmit.ts`）。
  2. 认证方式：密码、私钥、证书、agent；保存的密码是否可用（`useSavedPasswordAvailability.ts`）；sudo 自动填充。
  3. 跳板机：手动填写、引用已有连接、来自 SSH 配置；多跳。
  4. 代理设置。
  5. 终端相关：编码、终端主题、关键词高亮规则集、会话自定义、图标。
  6. 本地 shell 与串口连接类型的字段（`useSerialPorts.ts`）。
  7. 连接测试（`useConnectionTest.ts`）。
- 验收：共同要求；保存后的数据与 Tauri 版保存的数据逐字段相同（用同一份输入在两个客户端中各保存一次后比较）。

## P4-03 SSH 配置导入

- 依赖：P4-01
- 规模：M
- 参考：`src/components/SshConfigImportDialog/`、`src-tauri/src/profiles/` 中的 SSH 配置解析
- 工作：读取 `~/.ssh/config`，显示可导入的主机、警告（包括 Include 文件的具体文件与行号）、跳板机使用的用户名提示，选择导入。
- 验收：使用与 Tauri 版相同的测试配置文件，导入结果相同。

## P4-04 认证与主机密钥界面

- 依赖：P1-05、G3
- 规模：M
- 参考：`TerminalTab/HostKeyPromptDialog.tsx`、`SshAuthPromptDialog.tsx`、`JumpHostInfoDialog.tsx`、`src/components/ProxyCommandPrompt/`、`TunnelsPanel/TunnelHostKeyPrompt.tsx`、`TunnelsPanel/CredentialsDialog.tsx`
- 工作：通过 P1-05 的 `PromptBroker` 显示并回应：新主机密钥、密钥变化、证书与 `@revoked` 的提示；密码与键盘交互认证；跳板链信息；ProxyCommand 审批与过期。
- 验收：用 WSL podman 测试主机制造每种情况；提示在会话关闭时自动消失。

## P4-05 SFTP 浏览器

- 依赖：G3、P4-04
- 规模：L，拆成以下子卡，可以分给不同执行者
- 参考：`src/components/SftpDrawer/` 全部文件
- 子卡：

| 子卡   | 内容                                                                       | 主要来源                                                                                                                                                                                                                                                                                                                |
| ------ | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P4-05a | 抽屉框架、目录列表（虚拟滚动、列宽、排序、视图模式）、路径栏、前进后退历史 | `index.tsx`、`SftpDrawerContent.tsx`、`SftpDrawerHeader.tsx`、`SftpPathBar.tsx`、`SftpTableHeader.tsx`、`useSftpView.ts`、`useSftpColumnWidths.ts`、`useSftpHistory.ts`、`useSftpDirectoryLoader.ts`、`useSftpDrawerLifecycle.ts`、`SftpEntryIcon.tsx`                                                                  |
| P4-05b | 选择与右键菜单、新建、重命名、删除、权限、冲突处理                         | `useSftpSelection.ts`、`useSftpSelectionMode.ts`、`useSftpPointerSelection.ts`、`SftpEntryContextMenu.tsx`、`useSftpEntryActions.ts`、`useSftpDeleteActions.ts`、`SftpDeleteTransferEvents.tsx`、`SftpPermissionsDialog.tsx`、`SftpConflictDialog.tsx`、`SftpDialogs.tsx`、`sftpDialogReducer.ts`、`SftpErrorAlert.tsx` |
| P4-05c | 上传与下载：对话框选择、拖入、粘贴上传、下载目录                           | `useSftpUploads.ts`、`useUploadEvents.ts`、`useSftpDownloads.ts`、`downloadEvents.ts`、`useSftpDragDrop.ts`、`useSftpPasteUpload.ts`、`useSftpTransfers.ts`                                                                                                                                                             |
| P4-05d | 拖出到系统文件管理器                                                       | `useSftpDragOut.ts`、`dragOut.ts`；P0-19 的方案                                                                                                                                                                                                                                                                         |
| P4-05e | 搜索、压缩与解压、sudo 模式、跟随终端目录、终端操作                        | `SftpHeaderSearch.tsx`、`useSftpRecursiveSearch.ts`、`SftpSearchStatus.tsx`、`SftpArchiveDialog.tsx`、`useSftpArchive.ts`、`SftpSudoToggle.tsx`、`useSftpSudoMode.ts`、`useSftpTerminalFollow.ts`、`SftpTerminalActions.tsx`                                                                                            |
| P4-05f | 用外部程序打开与外部编辑同步                                               | `SftpOpenWithDialog.tsx`、`useSftpOpenWith.ts`、`useSftpExternalEdits.tsx`、`SftpExternalEditsMenu.tsx`、`externalEdit.ts`                                                                                                                                                                                              |

- 验收：共同要求；现有 SFTP 测试中描述的每个行为都有对应的验证；1 万个文件的目录滚动流畅。
- 评估重点：传输进行中关闭抽屉或标签时，传输按设置继续或取消，不由视图生命周期决定。

## P4-06 传输管理器

- 依赖：P4-05c
- 规模：M
- 参考：`src/components/TransferManager/`、`src/contexts/TransferContext.tsx`、`SettingsDialog/BandwidthLimitInput.tsx`、`SftpSettingsSection.tsx`
- 工作：传输列表、进度、速度、剩余时间、取消、重试、清除已完成；并行数与限速设置。
- 验收：100 个文件同时传输时界面不卡顿；进度事件的节流与 Tauri 版一致。

## P4-07 远程文件编辑器

- 依赖：P0-20、P4-05a
- 规模：L
- 参考：`src/components/RemoteFileEditor.tsx`、`CodeMirrorEditor.tsx`、`RemoteFileEditor.css`；P0-20 的结论
- 工作：按 P0-20 选定的方案实现：打开、编辑、保存、保存冲突检测、编码与换行格式保留、查找替换、语法高亮、未保存提示、大文件策略。远程文件以标签打开，标签按“连接、身份与路径”区分：同一文件在 sudo 模式下以 root 打开时是另一个标签，读写都以该身份进行（现有逻辑在 `TTermApp/useTabOpeners.ts`）。
- 验收：P0-20 列出的全部场景；保存后的文件与原文件在未修改部分逐字节相同。

## P4-08 隧道

- 依赖：G3、P4-04
- 规模：M
- 参考：`src/components/TunnelsPanel/` 全部文件、`src-tauri/src/tunnel/`、托盘事件 `tray-open-tunnels`
- 工作：隧道列表与卡片、新建与编辑、启动与停止、自动启动、后台运行、凭据对话框、状态通知、从托盘打开。
- 验收：本地、远程、动态三种转发；关闭窗口后隧道按设置继续运行。

## P4-09 服务器监控

- 依赖：G2
- 规模：M
- 参考：`TerminalTab/ServerMonitorBar.tsx`、`ServerMonitorPanel.tsx`、`ServerMonitorMetrics.tsx`、`serverMetrics.ts`、`serverMonitorFormat.ts`、`useServerMetricsPolling.ts`、`src/lib/monitorMetrics.ts`、`SettingsDialog/MonitorMetricRow.tsx`、`monitorMetricOrder.ts`、`src-tauri/src/monitor/`
- 工作：监控栏与详细面板、指标顺序与显示设置、轮询节奏、不支持的系统或拒绝连接时的处理。
- 验收：Linux、macOS 远程主机各验证一次；隐藏标签时停止轮询。

## P4-10 串口

- 依赖：G2、P4-02
- 规模：M
- 参考：`TerminalTab/SerialControlBar.tsx`、`SerialSendBar.tsx`、`SerialSendHistoryMenu.tsx`、`SerialFileTransfer.tsx`、`useSerialControls.ts`、`useSerialSender.ts`、`useSerialSendHistory.ts`、`src/lib/serial.ts`、`serialSendStore.ts`、`src-tauri/src/serial/`
- 工作：控制栏（波特率、流控、DTR、RTS 等）、发送栏与历史、十六进制发送、XMODEM 文件传输。
- 验收：用虚拟串口对（Linux 上的 `socat`，Windows 上的 com0com）验证；有条件时用真实设备验证。

## P4-11 日志与回放

- 依赖：G2
- 规模：M
- 参考：`src/components/LogsPanel/`、`src/lib/terminalLogReplay.ts`、`SettingsDialog/LoggingSettingsTab.tsx`、`logNameTemplate.ts`、`useTerminalLogStatus.ts`
- 工作：日志会话列表、文本查看、按时间回放（播放、暂停、速度、字体）、日志命名模板与记录设置。
- 验收：Tauri 版录制的日志可以在 GPUI 版中回放。

## P4-12 命令库、编写器与变量

- 依赖：P2-17、G3
- 规模：L
- 参考：`src/components/CommandLibrary/`、`CommandComposer/`、`CommandVariablesDialog.tsx`、`CommandVariablesEditor.tsx`、`TTermApp/useCommandLibraryActions.ts`、`CommandLibraryDialogs.tsx`、`src/lib/commandVariables.ts`、`commandComposer.ts`、`composerRun*.ts`、`recentCommands.ts`、`src-tauri/src/command_library/`
- 工作：命令库的列表、详情、编辑、标签管理、插入到终端；命令编写器；带变量的命令及变量对话框；最近命令。
- 验收：共同要求；变量替换的结果与 TS 测试一致。

## P4-13 广播输入

- 依赖：P2-17、P3-08
- 规模：M
- 参考：`src/components/BroadcastManager.tsx`、`TTermApp/useBroadcastControls.ts`、`useBroadcastRuntime.ts`、`useBroadcastSending.ts`、`useBroadcastState.ts`、`useComposerBroadcast.ts`、`TerminalTab/BroadcastControls.tsx`
- 工作：选择广播目标、开始与停止广播、广播中的视觉提示、编写器向多个终端发送。
- 验收：10 个会话同时广播时输入顺序与内容一致。

## P4-14 主题库、编辑与导入

- 依赖：P3-05、P2-10
- 规模：M
- 参考：`src/components/ThemeEditor/`、`ThemeGallery.tsx`、`ThemeCard.tsx`、`ThemeImportDialog.tsx`、`ThemePreviewSwatches.tsx`、`TerminalPalettePreview.tsx`、`SettingsDialog/ThemeDialogs.tsx`、`ThemeLibrarySettings.tsx`、`useThemeSettingsHandlers.ts`、`src/lib/themeImport.ts`
- 工作：主题库浏览与预览、自定义主题的编辑（颜色选择、实时预览）、从文件导入、删除与复制。
- 验收：导入现有测试中使用的各种主题文件格式，结果与 TS 版一致。

## P4-15 至 P4-26 设置

`SettingsDialog/` 是最大的前端目录。先完成 P4-15 的框架，其余各卡可以并行。

| 卡    | 内容                                                                     | 主要来源（`src/components/SettingsDialog/` 下）                                                                                                                                                                                                                                                                                                                                                            | 规模 |
| ----- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| P4-15 | 设置框架：布局、侧边栏、搜索与搜索索引、懒加载、保存与草稿               | `index.tsx`、`SettingsLayout.tsx`、`SettingsSidebar.tsx`、`SettingsTabPanel.tsx`、`settingsTabs.ts`、`settingsSearch*.ts`、`SettingsSearch*.tsx`、`useSettingsSearch.ts`、`useSettingsHandlers.ts`、`useSettingsTabLoaders.ts`、`settingsLazy.ts`、`settingsLoaders.ts`                                                                                                                                    | M    |
| P4-16 | 通用、外观、字体、界面风格、窗口模糊、标签、窗格                         | `GeneralSettingsTab.tsx`、`AppearanceSettingsTab.tsx`、`FontSettingsTab.tsx`、`FontFamilyPicker.tsx`、`CursorStylePicker.tsx`、`TerminalPaddingFields.tsx`、`ScrollbackLinesField.tsx`、`UiStyleSetting.tsx`、`WindowBlurSection.tsx`、`useWindowBlurDraft.ts`、`TabSettingsSection.tsx`、`TabIconSettings.tsx`、`TabAgentIndicatorSettings.tsx`、`PaneSettingsSection.tsx`、`useTerminalSettingsDraft.ts` | L    |
| P4-17 | 连接、代理、本地 shell、shell 集成、预测回显、sudo 提示、SFTP            | `ConnectionSettingsTab.tsx`、`ProxySettingsTab.tsx`、`ProxyServerDialog.tsx`、`ProxyServerFields.tsx`、`LocalShells/`、`ShellIntegrationSection.tsx`、`PredictiveEchoSection.tsx`、`SudoPromptPatternsSetting.tsx`、`SftpSettingsSection.tsx`、`BandwidthLimitInput.tsx`                                                                                                                                   | L    |
| P4-18 | 安全：主密码、密钥库解锁、密钥存储方式、已保存的密码、SSH 密钥、已知主机 | `SecuritySettingsTab.tsx`、`SecurityCards.tsx`、`MasterPasswordCard.tsx`、`ChoosePasswordModeCard.tsx`、`UnlockVaultCard.tsx`、`SystemUnlockFailedCard.tsx`、`SecretStorageSection.tsx`、`SecretFields.tsx`、`SavedSecretsCard.tsx`、`useSavedSecrets.ts`、`useVaultActions.ts`、`SshKeysCard.tsx`、`ImportSshKeyDialog.tsx`、`useSshKeyActions.ts`、`KnownHostsCard.tsx`                                  | L    |
| P4-19 | 关键词高亮规则编辑                                                       | `HighlightSettingsTab.tsx`、`HighlightRuleEditor.tsx`、`HighlightRuleList.tsx`、`HighlightSetToolbar.tsx`、`HighlightPreview.tsx`、`HighlightColorPicker.tsx`、`useHighlightSettings.ts`                                                                                                                                                                                                                   | M    |
| P4-20 | 日志、通知、监控指标                                                     | `LoggingSettingsTab.tsx`、`NotificationSettingsTab.tsx`、`NotificationSoundRow.tsx`、`MonitorMetricRow.tsx`                                                                                                                                                                                                                                                                                                | M    |
| P4-21 | 快捷键设置                                                               | `KeymapSettingsTab.tsx`                                                                                                                                                                                                                                                                                                                                                                                    | M    |
| P4-22 | 备份：导出、导入、历史、自动备份、WebDAV 远程备份                        | `BackupExportCard.tsx`、`BackupImportCard.tsx`、`BackupHistoryCard.tsx`、`AutomaticBackupCard.tsx`、`RemoteBackupsCard.tsx`、`WebDavBackupPanel.tsx`、`WebDavBackupOptions.tsx`、`WebDavServerFields.tsx`、`webdavBackup.ts`、`backupShared.ts`                                                                                                                                                            | L    |
| P4-23 | 同步                                                                     | `WebDavSyncCard.tsx`、`src/hooks/useWebDavSync.ts`、`src/lib/sync.ts`                                                                                                                                                                                                                                                                                                                                      | M    |
| P4-24 | 数据迁移页：从备份或 WebDAV 导入、查看差异后应用                         | `DataMigrationSettingsTab.tsx`、`dataMigration.ts`、`MigrationViewTabs.tsx`                                                                                                                                                                                                                                                                                                                                | M    |
| P4-25 | 更新与关于                                                               | `UpdateSettingsTab.tsx`、`src/components/AboutDialog/`、`AppUpdateManager.tsx`、`UpdateReleaseNotes.tsx`、`src/lib/updater.ts`                                                                                                                                                                                                                                                                             | M    |
| P4-26 | 系统集成与 AI agent 集成                                                 | `SystemIntegrationSection.tsx`、`useSystemIntegration.ts`、`AgentIntegrationSection.tsx`、`src/components/AgentIntegrationRow/`、`RemoteAgentIntegrationDialog/`、`src-tauri/src/system_integration/`、`agent_integration/`                                                                                                                                                                                | M    |

- 依赖：P4-15 依赖 G3；其余依赖 P4-15，P4-25 另依赖 P5-03 的更新器接口。
- P4-25 补充：“关于”中的许可证页（`AboutDialog/LicensesView.tsx`）现在读取 `scripts/gen-licenses.mjs` 从 npm 依赖生成的 `public/licenses.json`。原生版需要从 Rust 依赖生成同类数据（例如用 `cargo-about`），包括 GPUI、gpui-kit、Alacritty、导入的 nyaterm 代码、随 Windows 版分发的 ConPTY，以及 P3-16 登记的图标与主题资源。
- 共同验收：每个设置项在两个客户端之间互通（在一个客户端中修改，另一个客户端读到相同的值）；设置搜索能找到每一项。
- 评估重点：P4-18 与 P4-22 涉及密钥与备份，必须用真实的旧数据验证，不能只用新建的数据。

## P4-27 功能对齐审核

- 依赖：P4-01 至 P4-26
- 规模：M
- 工作：
  1. 填满 `feature-matrix.md`：遍历 `src/components/` 的每个功能目录与 `en.json` 中的文案分组，确认每个功能在 GPUI 版中的位置与状态。
  2. 列出“明确放弃”的功能，附理由，交给人确认。
  3. 列出“进行中”的功能与剩余工作。
- 产出：完整的功能矩阵。
- 验收：矩阵中没有“未开始”；每个“对齐”都有证据链接。
- 评估重点：这是进入 P5 的前提，评估者会抽查对齐项。
