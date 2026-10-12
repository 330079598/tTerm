import React, { useEffect, useRef, useState } from "react"
import { Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  ThemeEditor,
  ThemeGallery,
  ThemeImportDialog,
} from "@/components/SettingsDialog/settingsLazy"
import {
  getPerfNow,
  logPerf,
  loadSystemFontsCached,
  normalizeSettingsTab,
  refreshSecretStatusCached,
} from "@/components/SettingsDialog/settingsLoaders"
import { useTerminalSettingsDraft } from "@/components/SettingsDialog/useTerminalSettingsDraft"
import { useThemeSettingsHandlers } from "@/components/SettingsDialog/useThemeSettingsHandlers"
import { useSettingsHandlers } from "@/components/SettingsDialog/useSettingsHandlers"
import { AboutDialog } from "@/components/AboutDialog"
import { useConfirmDialog, useInfoDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useSyncedState } from "@/hooks/useSyncedState"
import { cn } from "@/lib/utils"
import { AppearanceSettingsTab } from "@/components/SettingsDialog/AppearanceSettingsTab"
import { ConnectionSettingsTab } from "@/components/SettingsDialog/ConnectionSettingsTab"
import { DataMigrationSettingsTab } from "@/components/SettingsDialog/DataMigrationSettingsTab"
import { FontSettingsTab } from "@/components/SettingsDialog/FontSettingsTab"
import { GeneralSettingsTab } from "@/components/SettingsDialog/GeneralSettingsTab"
import { LoggingSettingsTab } from "@/components/SettingsDialog/LoggingSettingsTab"
import { NotificationSettingsTab } from "@/components/SettingsDialog/NotificationSettingsTab"
import { KeymapSettingsTab } from "@/components/SettingsDialog/KeymapSettingsTab"
import { ProfileGroupsSettingsTab } from "@/components/SettingsDialog/ProfileGroupsSettingsTab"
import { SecuritySettingsTab } from "@/components/SettingsDialog/SecuritySettingsTab"
import { SettingsSidebar } from "@/components/SettingsDialog/SettingsSidebar"
import { UpdateSettingsTab } from "@/components/SettingsDialog/UpdateSettingsTab"
import {
  FONT_SIZE_OPTIONS,
  languages,
  SettingsDialogProps,
  SettingsPanelProps,
} from "@/components/SettingsDialog/types"

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
  defaultTab = "general",
  className,
  onConnectProfile,
  onEditProfile,
  profilesRefreshKey,
}) => {
  const mountStartRef = useRef(getPerfNow())
  const isMountedRef = useRef(true)
  const { t, i18n } = useTranslation()
  const { appState, config, saveAppState, saveConfig, updateLanguage, refreshSecretStatus } =
    useConfig()
  const {
    currentTheme,
    presetThemes,
    customThemes,
    presetThemeOverrides,
    setTheme,
    deleteCustomTheme,
    resetPresetTheme,
    duplicateTheme,
    getTheme,
    systemPrefersDark,
  } = useTheme()
  const { toast } = useToast()
  const { saveSettings } = useSettingsSave()
  const [activeTab, setActiveTab] = useSyncedState(normalizeSettingsTab(defaultTab))
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { prompt, PromptDialog } = usePromptDialog()
  const { info, InfoDialog } = useInfoDialog()
  const [aboutOpen, setAboutOpen] = useState(false)

  const terminalDraft = useTerminalSettingsDraft({ config, saveConfig, toast, t, isMountedRef })
  const { fontsLoaded, setSystemFonts, setFontLoadError, setFontsLoaded } = terminalDraft
  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)
  const [themeGalleryOpen, setThemeGalleryOpen] = useState(false)
  const [themeImportOpen, setThemeImportOpen] = useState(false)

  useEffect(() => {
    return () => {
      isMountedRef.current = false
    }
  }, [])

  useEffect(() => {
    logPerf("settings.open", mountStartRef.current, `tab=${activeTab}`)
  }, [activeTab])

  useEffect(() => {
    if (activeTab !== "terminal" || fontsLoaded) {
      return
    }

    // loadSystemFontsCached shares one in-flight request, so re-running this
    // (e.g. leaving and re-entering the tab mid-load) doesn't refetch.
    loadSystemFontsCached()
      .then((fonts) => {
        if (!isMountedRef.current) {
          return
        }
        setSystemFonts(fonts)
      })
      .catch(() => {
        if (!isMountedRef.current) {
          return
        }
        setSystemFonts([])
        setFontLoadError(
          t("fontSettings.loadingFontsFailed", {
            defaultValue:
              "Couldn't load the system font list. You can still enter a font name manually.",
          })
        )
      })
      .finally(() => {
        if (!isMountedRef.current) {
          return
        }
        setFontsLoaded(true)
      })
  }, [activeTab, fontsLoaded, t, setFontLoadError, setFontsLoaded, setSystemFonts])
  const loadingFonts = activeTab === "terminal" && !fontsLoaded

  useEffect(() => {
    if (activeTab !== "security") {
      return
    }

    refreshSecretStatusCached(refreshSecretStatus)
  }, [activeTab, refreshSecretStatus])

  const themeHandlers = useThemeSettingsHandlers({
    setTheme,
    systemPrefersDark,
    toast,
    t,
    saveSettings,
    confirm,
    deleteCustomTheme,
    prompt,
    duplicateTheme,
    getTheme,
    presetThemes,
    customThemes,
    setThemeGalleryOpen,
    setEditingThemeId,
    resetPresetTheme,
  })
  const settingsHandlers = useSettingsHandlers({ confirm, t, saveSettings, toast })

  const handleLanguageChange = async (langCode: string) => {
    try {
      await i18n.changeLanguage(langCode)
      await updateLanguage(langCode)
    } catch (error) {
      console.error("Failed to save language:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("settings.languageChangeFailed", {
          defaultValue: "Failed to save language setting.",
        }),
        variant: "destructive",
      })
    }
  }

  return (
    <>
      <div className={cn("flex h-full min-h-0 flex-col", className)}>
        <DialogHeader className="border-border border-b px-6 pt-6 pb-4">
          <DialogTitle className="flex items-center gap-2">
            <Settings size={18} />
            {t("settings.title")}
          </DialogTitle>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex min-h-0 flex-1">
          <SettingsSidebar activeTab={activeTab} onTabChange={setActiveTab} />

          <TabsContent
            value="appearance"
            id="settings-panel-appearance"
            role="tabpanel"
            aria-labelledby="settings-tab-appearance"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <AppearanceSettingsTab
              currentTheme={currentTheme}
              customThemes={customThemes}
              handleDeleteTheme={themeHandlers.handleDeleteTheme}
              handleDuplicateTheme={themeHandlers.handleDuplicateTheme}
              handleResetPresetTheme={themeHandlers.handleResetPresetTheme}
              handleThemeChange={themeHandlers.handleThemeChange}
              handleTabStandardWidthChange={themeHandlers.handleTabStandardWidthChange}
              handleTabWidthModeChange={themeHandlers.handleTabWidthModeChange}
              handleUiScaleChange={themeHandlers.handleUiScaleChange}
              handleWindowBlurChange={themeHandlers.handleWindowBlurChange}
              handleWindowBlurMaterialChange={themeHandlers.handleWindowBlurMaterialChange}
              handleWindowBlurRadiusChange={themeHandlers.handleWindowBlurRadiusChange}
              handleWindowOpacityChange={themeHandlers.handleWindowOpacityChange}
              presetThemes={presetThemes}
              presetThemeOverrides={presetThemeOverrides}
              onOpenThemeGallery={() => setThemeGalleryOpen(true)}
              onOpenThemeImport={() => setThemeImportOpen(true)}
              setCreatingFromTheme={setCreatingFromTheme}
              setEditingThemeId={setEditingThemeId}
              tabStandardWidth={config.tab_standard_width}
              tabWidthMode={config.tab_width_mode}
              uiScalePercent={config.ui_scale_percent}
              windowBlur={config.window_blur}
              windowBlurMaterial={config.window_blur_material}
              windowBlurRadius={config.window_blur_radius}
              windowOpacityPercent={config.window_opacity_percent}
            />
          </TabsContent>

          <TabsContent
            value="terminal"
            id="settings-panel-terminal"
            role="tabpanel"
            aria-labelledby="settings-tab-terminal"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-y-auto p-6"
          >
            <FontSettingsTab
              {...terminalDraft}
              loadingFonts={loadingFonts}
              fontSizeOptions={FONT_SIZE_OPTIONS}
            />
          </TabsContent>

          <TabsContent
            value="notifications"
            id="settings-panel-notifications"
            role="tabpanel"
            aria-labelledby="settings-tab-notifications"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <NotificationSettingsTab />
          </TabsContent>

          <TabsContent
            value="connection"
            id="settings-panel-connection"
            role="tabpanel"
            aria-labelledby="settings-tab-connection"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <ConnectionSettingsTab
              handleMonitorRefreshIntervalChange={
                settingsHandlers.handleMonitorRefreshIntervalChange
              }
              handleMonitorVisibleMetricsChange={settingsHandlers.handleMonitorVisibleMetricsChange}
              handleReconnectEnabledChange={settingsHandlers.handleReconnectEnabledChange}
              handleReconnectMaxAttemptsChange={settingsHandlers.handleReconnectMaxAttemptsChange}
              handleShowJumpHostConnectionInfoChange={
                settingsHandlers.handleShowJumpHostConnectionInfoChange
              }
              handleSudoPromptPatternsChange={settingsHandlers.handleSudoPromptPatternsChange}
              monitorRefreshIntervalSecs={config.monitor_refresh_interval_secs}
              monitorVisibleMetrics={config.monitor_visible_metrics}
              reconnectEnabled={config.reconnect_enabled}
              reconnectMaxAttempts={config.reconnect_max_attempts}
              showJumpHostConnectionInfo={config.show_jump_host_connection_info}
              sudoPromptPatterns={config.sudo_prompt_patterns}
            />
          </TabsContent>

          <TabsContent
            value="logging"
            id="settings-panel-logging"
            role="tabpanel"
            aria-labelledby="settings-tab-logging"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <LoggingSettingsTab
              handleEnabledChange={settingsHandlers.handleTerminalLogEnabledChange}
              handleRecordInputChange={settingsHandlers.handleTerminalLogRecordInputChange}
            />
          </TabsContent>

          <TabsContent
            value="keymap"
            id="settings-panel-keymap"
            role="tabpanel"
            aria-labelledby="settings-tab-keymap"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <KeymapSettingsTab />
          </TabsContent>

          <TabsContent
            value="profile-groups"
            id="settings-panel-profile-groups"
            role="tabpanel"
            aria-labelledby="settings-tab-profile-groups"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <ProfileGroupsSettingsTab
              onConnectProfile={onConnectProfile}
              onEditProfile={onEditProfile}
              refreshKey={profilesRefreshKey}
            />
          </TabsContent>

          <TabsContent
            value="data-migration"
            id="settings-panel-data-migration"
            role="tabpanel"
            aria-labelledby="settings-tab-data-migration"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <DataMigrationSettingsTab />
          </TabsContent>

          <TabsContent
            value="security"
            id="settings-panel-security"
            role="tabpanel"
            aria-labelledby="settings-tab-security"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <SecuritySettingsTab confirm={confirm} info={info} />
          </TabsContent>

          <TabsContent
            value="general"
            id="settings-panel-general"
            role="tabpanel"
            aria-labelledby="settings-tab-general"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <GeneralSettingsTab
              backgroundThrottling={config.background_throttling}
              closeBehavior={config.close_behavior}
              handleAbout={() => setAboutOpen(true)}
              handleBackgroundThrottlingChange={settingsHandlers.handleBackgroundThrottlingChange}
              handleCloseBehaviorChange={settingsHandlers.handleCloseBehaviorChange}
              handleClearSession={settingsHandlers.handleClearSession}
              handleLanguageChange={handleLanguageChange}
              handleRestoreAllSessionConnectionsChange={
                settingsHandlers.handleRestoreAllSessionConnectionsChange
              }
              handleSftpPasteUploadEnabledChange={
                settingsHandlers.handleSftpPasteUploadEnabledChange
              }
              handleSftpTransferParallelismChange={
                settingsHandlers.handleSftpTransferParallelismChange
              }
              handleSftpUploadLimitChange={settingsHandlers.handleSftpUploadLimitChange}
              handleSftpDownloadLimitChange={settingsHandlers.handleSftpDownloadLimitChange}
              handleEnableDevtoolsChange={settingsHandlers.handleEnableDevtoolsChange}
              handleZmodemAutoDetectEnabledChange={
                settingsHandlers.handleZmodemAutoDetectEnabledChange
              }
              handleZmodemDownloadDirectoryChange={
                settingsHandlers.handleZmodemDownloadDirectoryChange
              }
              i18nLanguage={i18n.language}
              languages={languages}
              restoreAllSessionConnections={config.startup_session_restore_mode === "all"}
              sftpPasteUploadEnabled={config.sftp_paste_upload_enabled}
              sftpTransferParallelism={config.sftp_transfer_parallelism}
              sftpUploadLimitKib={config.sftp_upload_limit_kib}
              sftpDownloadLimitKib={config.sftp_download_limit_kib}
              zmodemAutoDetectEnabled={config.zmodem_auto_detect_enabled}
              zmodemDownloadDirectory={config.zmodem_download_directory}
            />
          </TabsContent>

          <TabsContent
            value="updates"
            id="settings-panel-updates"
            role="tabpanel"
            aria-labelledby="settings-tab-updates"
            tabIndex={0}
            className="m-0 min-w-0 flex-1 overflow-hidden p-4 sm:p-6"
          >
            <UpdateSettingsTab
              autoDownloadUpdates={config.auto_download_updates}
              handleAutoDownloadUpdatesChange={settingsHandlers.handleAutoDownloadUpdatesChange}
              handleUpdateCheckFrequencyChange={settingsHandlers.handleUpdateCheckFrequencyChange}
              handleUpdateChannelChange={settingsHandlers.handleUpdateChannelChange}
              handleUpdateCheckComplete={(checkedAt) =>
                saveAppState({ lastUpdateCheckAt: checkedAt })
              }
              updateChannel={config.update_channel}
              updateCheckFrequency={config.update_check_frequency}
              lastUpdateCheckAt={appState.lastUpdateCheckAt}
            />
          </TabsContent>
        </Tabs>
      </div>

      {editingThemeId && (
        <React.Suspense fallback={null}>
          <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
        </React.Suspense>
      )}

      {creatingFromTheme && (
        <React.Suspense fallback={null}>
          <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
        </React.Suspense>
      )}

      {themeGalleryOpen && (
        <React.Suspense fallback={null}>
          <ThemeGallery
            onApply={themeHandlers.handleGalleryApply}
            onCopy={themeHandlers.handleCopyCatalogTheme}
            onClose={() => setThemeGalleryOpen(false)}
          />
        </React.Suspense>
      )}

      {themeImportOpen && (
        <React.Suspense fallback={null}>
          <ThemeImportDialog
            onImported={themeHandlers.handleThemesImported}
            onClose={() => setThemeImportOpen(false)}
          />
        </React.Suspense>
      )}
      <ConfirmDialog />
      <PromptDialog />
      <InfoDialog />
      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
    </>
  )
}

export const SettingsDialog: React.FC<SettingsDialogProps> = ({
  onClose,
  defaultTab,
  onConnectProfile,
  onEditProfile,
  profilesRefreshKey,
}) => {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] p-0 sm:max-w-4xl">
        <SettingsPanel
          defaultTab={defaultTab}
          className="h-[calc(85vh-2rem)]"
          onConnectProfile={onConnectProfile}
          onEditProfile={onEditProfile}
          profilesRefreshKey={profilesRefreshKey}
        />
      </DialogContent>
    </Dialog>
  )
}
