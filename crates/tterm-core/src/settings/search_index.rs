//! The settings pages and their entries for settings search, from
//! `src/components/SettingsDialog/settingsSearchIndex.ts`.
//!
//! Both fields of an entry are i18n keys. A client translates them and
//! builds its search results; this module keeps the list of settings that
//! belong to pages it does not currently have open.

#[cfg(test)]
mod tests;

/// One setting the settings search can find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsSearchEntry {
    /// The i18n key whose translation is the setting's title.
    pub title: &'static str,
    /// The i18n key for the setting's description.
    pub description: Option<&'static str>,
}

/// One page in the settings sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsPage {
    /// The stable id used by search results and by the page's entries.
    pub value: &'static str,
    /// The i18n key for the page's sidebar label.
    pub label_key: &'static str,
    /// The fallback English label used until the translation is available.
    pub default_label: Option<&'static str>,
}

/// The pages in sidebar order.
pub const SETTINGS_PAGES: [SettingsPage; 14] = [
    SettingsPage {
        value: "profile-groups",
        label_key: "settings.profileGroups",
        default_label: Some("Groups"),
    },
    SettingsPage {
        value: "connection",
        label_key: "settings.connection",
        default_label: None,
    },
    SettingsPage {
        value: "proxies",
        label_key: "proxies.sidebar",
        default_label: None,
    },
    SettingsPage {
        value: "appearance",
        label_key: "settings.appearance",
        default_label: None,
    },
    SettingsPage {
        value: "terminal",
        label_key: "settings.terminal",
        default_label: Some("Terminal"),
    },
    SettingsPage {
        value: "shells",
        label_key: "localShells.sidebar",
        default_label: None,
    },
    SettingsPage {
        value: "highlight",
        label_key: "keywordHighlight.sidebar",
        default_label: None,
    },
    SettingsPage {
        value: "notifications",
        label_key: "notifications.sidebar",
        default_label: None,
    },
    SettingsPage {
        value: "keymap",
        label_key: "settings.keymap",
        default_label: None,
    },
    SettingsPage {
        value: "logging",
        label_key: "terminalLogging.title",
        default_label: None,
    },
    SettingsPage {
        value: "data-migration",
        label_key: "dataMigration.sidebar",
        default_label: None,
    },
    SettingsPage {
        value: "security",
        label_key: "settings.security",
        default_label: None,
    },
    SettingsPage {
        value: "general",
        label_key: "settings.general",
        default_label: None,
    },
    SettingsPage {
        value: "updates",
        label_key: "settings.updates",
        default_label: Some("Updates"),
    },
];

/// The settings each page holds, in the order the page shows them.
#[derive(Debug, Clone, Copy)]
pub struct SettingsPageIndex {
    /// The page id from [`SETTINGS_PAGES`].
    pub page: &'static str,
    /// The page's searchable settings.
    pub entries: &'static [SettingsSearchEntry],
}

/// The setting entries for every page the sidebar lists.
pub const SETTINGS_SEARCH_INDEX: [SettingsPageIndex; 14] = [
    SettingsPageIndex {
        page: "profile-groups",
        entries: &[SettingsSearchEntry {
            title: "profileGroups.title",
            description: Some("profileGroups.pageDescription"),
        }],
    },
    SettingsPageIndex {
        page: "connection",
        entries: &[
            SettingsSearchEntry {
                title: "settings.reconnect",
                description: Some("settings.reconnectDesc"),
            },
            SettingsSearchEntry {
                title: "settings.autoReconnect",
                description: Some("settings.autoReconnectDesc"),
            },
            SettingsSearchEntry {
                title: "settings.reconnectMaxAttempts",
                description: Some("settings.reconnectMaxAttemptsDesc"),
            },
            SettingsSearchEntry {
                title: "settings.typing",
                description: Some("settings.typingDesc"),
            },
            SettingsSearchEntry {
                title: "settings.predictiveEcho",
                description: Some("settings.predictiveEchoDesc"),
            },
            SettingsSearchEntry {
                title: "settings.predictiveEchoThreshold",
                description: Some("settings.predictiveEchoThresholdDesc"),
            },
            SettingsSearchEntry {
                title: "settings.shellIntegration",
                description: Some("settings.shellIntegrationDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sshShellIntegration",
                description: Some("settings.sshShellIntegrationDesc"),
            },
            SettingsSearchEntry {
                title: "settings.jumpHost",
                description: Some("settings.connectionDesc"),
            },
            SettingsSearchEntry {
                title: "settings.showJumpHostConnectionInfo",
                description: Some("settings.showJumpHostConnectionInfoDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sudoAutofill",
                description: Some("settings.sudoAutofillDesc"),
            },
            SettingsSearchEntry {
                title: "settings.passwordFeedback",
                description: Some("settings.passwordFeedbackDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sudoPromptPatterns",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.monitor",
                description: Some("settings.monitorDesc"),
            },
            SettingsSearchEntry {
                title: "settings.monitorRefreshInterval",
                description: Some("settings.monitorRefreshIntervalDesc"),
            },
            SettingsSearchEntry {
                title: "settings.monitorVisibleMetrics",
                description: Some("settings.monitorVisibleMetricsDesc"),
            },
        ],
    },
    SettingsPageIndex {
        page: "proxies",
        entries: &[
            SettingsSearchEntry {
                title: "proxies.title",
                description: Some("proxies.description"),
            },
            SettingsSearchEntry {
                title: "proxies.defaultTitle",
                description: None,
            },
        ],
    },
    SettingsPageIndex {
        page: "appearance",
        entries: &[
            SettingsSearchEntry {
                title: "theme.title",
                description: Some("theme.description"),
            },
            SettingsSearchEntry {
                title: "theme.followSystem",
                description: Some("theme.followSystemDesc"),
            },
            SettingsSearchEntry {
                title: "themeEditor.presetThemes",
                description: None,
            },
            SettingsSearchEntry {
                title: "themeEditor.customThemes",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.interface",
                description: Some("settings.interfaceScaleDesc"),
            },
            SettingsSearchEntry {
                title: "settings.interfaceScale",
                description: Some("settings.interfaceScaleRange"),
            },
            SettingsSearchEntry {
                title: "settings.uiStyle",
                description: Some("settings.uiStyleDesc"),
            },
            SettingsSearchEntry {
                title: "settings.window",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.windowBlur",
                description: Some("settings.windowBlurDesc"),
            },
            SettingsSearchEntry {
                title: "settings.windowBlurMaterial",
                description: Some("settings.windowBlurMaterialDesc"),
            },
            SettingsSearchEntry {
                title: "settings.windowBlurRadius",
                description: Some("settings.windowBlurRadiusDesc"),
            },
            SettingsSearchEntry {
                title: "settings.windowOpacity",
                description: Some("settings.windowOpacityDesc"),
            },
            SettingsSearchEntry {
                title: "settings.tabWidth",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.tabWidthMode",
                description: Some("settings.tabWidthModeDesc"),
            },
            SettingsSearchEntry {
                title: "settings.tabStandardWidth",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.tabHoverTooltip",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.tabHoverTooltipShow",
                description: Some("settings.tabHoverTooltipShowDesc"),
            },
            SettingsSearchEntry {
                title: "tabIcons.title",
                description: Some("tabIcons.description"),
            },
            SettingsSearchEntry {
                title: "tabIcons.show",
                description: Some("tabIcons.showDescription"),
            },
            SettingsSearchEntry {
                title: "tabIcons.agents",
                description: Some("tabIcons.agentsDescription"),
            },
            SettingsSearchEntry {
                title: "tabIcons.byShell",
                description: Some("tabIcons.byShellDescription"),
            },
            SettingsSearchEntry {
                title: "tabAgentIndicator.title",
                description: Some("tabAgentIndicator.description"),
            },
            SettingsSearchEntry {
                title: "tabAgentIndicator.working",
                description: Some("tabAgentIndicator.workingDescription"),
            },
            SettingsSearchEntry {
                title: "tabAgentIndicator.waiting",
                description: Some("tabAgentIndicator.waitingDescription"),
            },
            SettingsSearchEntry {
                title: "settings.panes",
                description: Some("settings.panesDesc"),
            },
            SettingsSearchEntry {
                title: "settings.paneHeaders",
                description: Some("settings.paneHeadersDesc"),
            },
            SettingsSearchEntry {
                title: "settings.dimInactivePanes",
                description: Some("settings.dimInactivePanesDesc"),
            },
            SettingsSearchEntry {
                title: "layoutTemplates.saved",
                description: Some("settings.layoutTemplatesDesc"),
            },
        ],
    },
    SettingsPageIndex {
        page: "terminal",
        entries: &[
            SettingsSearchEntry {
                title: "fontSettings.textRendering",
                description: Some("fontSettings.textRenderingDesc"),
            },
            SettingsSearchEntry {
                title: "fontSettings.renderer",
                description: Some("fontSettings.rendererDesc"),
            },
            SettingsSearchEntry {
                title: "fontSettings.rendererRelease",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.fontFamily",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.fontSize",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.lineHeight",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.letterSpacing",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.behavior",
                description: Some("fontSettings.behaviorDesc"),
            },
            SettingsSearchEntry {
                title: "fontSettings.cursorStyle",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.scrollbackLines",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.macOptionIsMeta",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.confirmMultilinePaste",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.copyOnSelect",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.rightClickPaste",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.osc52ClipboardWrite",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.commandMarks",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.localShellIntegration",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.terminalPadding",
                description: None,
            },
            SettingsSearchEntry {
                title: "fontSettings.preview",
                description: Some("fontSettings.previewDesc"),
            },
        ],
    },
    SettingsPageIndex {
        page: "shells",
        entries: &[
            SettingsSearchEntry {
                title: "localShells.title",
                description: Some("localShells.description"),
            },
            SettingsSearchEntry {
                title: "localShells.listTitle",
                description: Some("localShells.listDescription"),
            },
        ],
    },
    SettingsPageIndex {
        page: "highlight",
        entries: &[
            SettingsSearchEntry {
                title: "keywordHighlight.title",
                description: Some("keywordHighlight.description"),
            },
            SettingsSearchEntry {
                title: "keywordHighlight.enabled",
                description: Some("keywordHighlight.enabledDesc"),
            },
            SettingsSearchEntry {
                title: "keywordHighlight.defaultSet",
                description: Some("keywordHighlight.defaultSetDesc"),
            },
            SettingsSearchEntry {
                title: "keywordHighlight.alternateScreen",
                description: Some("keywordHighlight.alternateScreenDesc"),
            },
            SettingsSearchEntry {
                title: "keywordHighlight.sets",
                description: Some("keywordHighlight.setsDesc"),
            },
            SettingsSearchEntry {
                title: "keywordHighlight.preview",
                description: Some("keywordHighlight.previewDesc"),
            },
        ],
    },
    SettingsPageIndex {
        page: "notifications",
        entries: &[
            SettingsSearchEntry {
                title: "notifications.title",
                description: Some("notifications.description"),
            },
            SettingsSearchEntry {
                title: "notifications.enabled",
                description: Some("notifications.enabledDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.sound",
                description: Some("notifications.soundDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.toastMaxVisible",
                description: Some("notifications.toastMaxVisibleDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.events",
                description: Some("notifications.eventsDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.commandFinishedSetting",
                description: Some("notifications.commandFinishedSettingDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.terminalRequests",
                description: Some("notifications.terminalRequestsDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.agents.title",
                description: Some("notifications.agents.description"),
            },
            SettingsSearchEntry {
                title: "notifications.agents.notifyWaiting",
                description: Some("notifications.agents.notifyWaitingDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.agents.notifyDone",
                description: Some("notifications.agents.notifyDoneDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.bellSection",
                description: None,
            },
            SettingsSearchEntry {
                title: "notifications.bellStyle",
                description: Some("notifications.bellStyleDesc"),
            },
            SettingsSearchEntry {
                title: "notifications.bellNotify",
                description: Some("notifications.bellNotifyDesc"),
            },
        ],
    },
    SettingsPageIndex {
        page: "keymap",
        entries: &[SettingsSearchEntry {
            title: "keymap.title",
            description: Some("keymap.description"),
        }],
    },
    SettingsPageIndex {
        page: "logging",
        entries: &[
            SettingsSearchEntry {
                title: "terminalLogging.title",
                description: Some("terminalLogging.description"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.enabled",
                description: Some("terminalLogging.enabledDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.recordInput",
                description: Some("terminalLogging.recordInputDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.browse",
                description: Some("terminalLogging.browseDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.storage",
                description: None,
            },
            SettingsSearchEntry {
                title: "terminalLogging.directory",
                description: Some("terminalLogging.directoryDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.format",
                description: Some("terminalLogging.formatDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.plainTimestamps",
                description: Some("terminalLogging.plainTimestampsDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.nameTemplate",
                description: Some("terminalLogging.nameTemplateDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.maxFileSize",
                description: Some("terminalLogging.maxFileSizeDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.compress",
                description: Some("terminalLogging.compressDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.cleanup",
                description: Some("terminalLogging.cleanupDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.retentionDays",
                description: Some("terminalLogging.retentionDaysDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.maxTotalSize",
                description: Some("terminalLogging.maxTotalSizeDesc"),
            },
            SettingsSearchEntry {
                title: "terminalLogging.status",
                description: None,
            },
        ],
    },
    SettingsPageIndex {
        page: "data-migration",
        entries: &[
            SettingsSearchEntry {
                title: "dataMigration.automaticTitle",
                description: None,
            },
            SettingsSearchEntry {
                title: "dataMigration.exportTitle",
                description: None,
            },
            SettingsSearchEntry {
                title: "dataMigration.webdav.title",
                description: None,
            },
            SettingsSearchEntry {
                title: "dataMigration.sync.title",
                description: None,
            },
            SettingsSearchEntry {
                title: "dataMigration.importTitle",
                description: None,
            },
            SettingsSearchEntry {
                title: "dataMigration.historyTitle",
                description: Some("dataMigration.historyDescription"),
            },
        ],
    },
    SettingsPageIndex {
        page: "security",
        entries: &[
            SettingsSearchEntry {
                title: "secretStorage.title",
                description: Some("secretStorage.description"),
            },
            SettingsSearchEntry {
                title: "secretStorage.storageMode",
                description: None,
            },
            SettingsSearchEntry {
                title: "secretStorage.changePassword",
                description: None,
            },
            SettingsSearchEntry {
                title: "secretStorage.promptUnlockOnStartup",
                description: Some("secretStorage.promptUnlockOnStartupDesc"),
            },
            SettingsSearchEntry {
                title: "secretStorage.savedPasswords",
                description: Some("secretStorage.savedPasswordsDesc"),
            },
            SettingsSearchEntry {
                title: "keyVault.title",
                description: Some("keyVault.description"),
            },
            SettingsSearchEntry {
                title: "knownHosts.title",
                description: Some("knownHosts.description"),
            },
        ],
    },
    SettingsPageIndex {
        page: "general",
        entries: &[
            SettingsSearchEntry {
                title: "language.title",
                description: Some("language.description"),
            },
            SettingsSearchEntry {
                title: "settings.sftp",
                description: Some("settings.sftpDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpStartDirectory",
                description: Some("settings.sftpStartDirectoryDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpPasteUpload",
                description: Some("settings.sftpPasteUploadDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpTransferParallelism",
                description: Some("settings.sftpTransferParallelismDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpUploadLimit",
                description: Some("settings.sftpUploadLimitDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpDownloadLimit",
                description: Some("settings.sftpDownloadLimitDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpExternalEditor",
                description: Some("settings.sftpExternalEditorDesc"),
            },
            SettingsSearchEntry {
                title: "settings.sftpExternalUpload",
                description: Some("settings.sftpExternalUploadDesc"),
            },
            SettingsSearchEntry {
                title: "settings.zmodem",
                description: Some("settings.zmodemDesc"),
            },
            SettingsSearchEntry {
                title: "settings.zmodemAutoDetect",
                description: Some("settings.zmodemAutoDetectDesc"),
            },
            SettingsSearchEntry {
                title: "settings.zmodemDownloadDirectory",
                description: Some("settings.zmodemDownloadDirectoryDesc"),
            },
            SettingsSearchEntry {
                title: "settings.startup",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.restoreAllSessionConnections",
                description: Some("settings.restoreAllSessionConnectionsDesc"),
            },
            SettingsSearchEntry {
                title: "settings.restoreTerminalContent",
                description: Some("settings.restoreTerminalContentDesc"),
            },
            SettingsSearchEntry {
                title: "settings.background",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.closeBehavior",
                description: Some("settings.closeBehaviorDesc"),
            },
            SettingsSearchEntry {
                title: "settings.backgroundThrottling",
                description: Some("settings.backgroundThrottlingDesc"),
            },
            SettingsSearchEntry {
                title: "systemIntegration.title",
                description: Some("systemIntegration.description"),
            },
            SettingsSearchEntry {
                title: "systemIntegration.shellCommand",
                description: None,
            },
            SettingsSearchEntry {
                title: "systemIntegration.folderMenu",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.advanced",
                description: Some("settings.advancedDesc"),
            },
            SettingsSearchEntry {
                title: "settings.devtools",
                description: Some("settings.devtoolsDesc"),
            },
            SettingsSearchEntry {
                title: "settings.maintenance",
                description: None,
            },
            SettingsSearchEntry {
                title: "settings.clearSession",
                description: Some("settings.clearSessionDesc"),
            },
            SettingsSearchEntry {
                title: "settings.about",
                description: None,
            },
        ],
    },
    SettingsPageIndex {
        page: "updates",
        entries: &[
            SettingsSearchEntry {
                title: "updates.channel",
                description: Some("updates.channelDesc"),
            },
            SettingsSearchEntry {
                title: "updates.autoDownload",
                description: Some("updates.autoDownloadDesc"),
            },
            SettingsSearchEntry {
                title: "updates.checkFrequency",
                description: Some("updates.checkFrequencyDesc"),
            },
            SettingsSearchEntry {
                title: "updates.manualCheck",
                description: Some("updates.manualCheckDesc"),
            },
        ],
    },
];

/// The searchable entries of one page, or an empty slice for an unknown id.
pub fn entries_for_page(page: &str) -> &'static [SettingsSearchEntry] {
    SETTINGS_SEARCH_INDEX
        .iter()
        .find(|index| index.page == page)
        .map_or(&[], |index| index.entries)
}
