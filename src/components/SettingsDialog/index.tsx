import React, { useEffect, useRef, useState } from "react"
import { Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import { normalizeSettingsTab } from "@/components/SettingsDialog/settingsLoaders"
import { SettingsTabPanel } from "@/components/SettingsDialog/SettingsTabPanel"
import { ThemeDialogs } from "@/components/SettingsDialog/ThemeDialogs"
import { useSettingsTabLoaders } from "@/components/SettingsDialog/useSettingsTabLoaders"
import { useTerminalSettingsDraft } from "@/components/SettingsDialog/useTerminalSettingsDraft"
import { useThemeSettingsHandlers } from "@/components/SettingsDialog/useThemeSettingsHandlers"
import { useSettingsHandlers } from "@/components/SettingsDialog/useSettingsHandlers"
import { AboutDialog } from "@/components/AboutDialog"
import { useConfirmDialog, useInfoDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs } from "@/components/ui/tabs"
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
import { HighlightSettingsTab } from "@/components/SettingsDialog/HighlightSettingsTab"
import { NotificationSettingsTab } from "@/components/SettingsDialog/NotificationSettingsTab"
import { KeymapSettingsTab } from "@/components/SettingsDialog/KeymapSettingsTab"
import { ProfileGroupsSettingsTab } from "@/components/SettingsDialog/ProfileGroupsSettingsTab"
import { ProxySettingsTab } from "@/components/SettingsDialog/ProxySettingsTab"
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

  const { loadingFonts } = useSettingsTabLoaders({
    activeTab,
    isMountedRef,
    fontsLoaded,
    setSystemFonts,
    setFontLoadError,
    setFontsLoaded,
    refreshSecretStatus,
  })

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

          <SettingsTabPanel value="appearance">
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
          </SettingsTabPanel>

          <SettingsTabPanel value="terminal" scroll>
            <FontSettingsTab
              {...terminalDraft}
              loadingFonts={loadingFonts}
              fontSizeOptions={FONT_SIZE_OPTIONS}
            />
          </SettingsTabPanel>

          <SettingsTabPanel value="highlight">
            <HighlightSettingsTab />
          </SettingsTabPanel>

          <SettingsTabPanel value="notifications">
            <NotificationSettingsTab onOpenTerminalSettings={() => setActiveTab("terminal")} />
          </SettingsTabPanel>

          <SettingsTabPanel value="connection">
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
              handlePasswordFeedbackChange={settingsHandlers.handlePasswordFeedbackChange}
              monitorRefreshIntervalSecs={config.monitor_refresh_interval_secs}
              monitorVisibleMetrics={config.monitor_visible_metrics}
              passwordFeedback={config.password_feedback}
              reconnectEnabled={config.reconnect_enabled}
              reconnectMaxAttempts={config.reconnect_max_attempts}
              showJumpHostConnectionInfo={config.show_jump_host_connection_info}
              sudoPromptPatterns={config.sudo_prompt_patterns}
            />
          </SettingsTabPanel>

          <SettingsTabPanel value="proxies">
            <ProxySettingsTab confirm={confirm} />
          </SettingsTabPanel>

          <SettingsTabPanel value="logging">
            <LoggingSettingsTab
              handleEnabledChange={settingsHandlers.handleTerminalLogEnabledChange}
              handleRecordInputChange={settingsHandlers.handleTerminalLogRecordInputChange}
            />
          </SettingsTabPanel>

          <SettingsTabPanel value="keymap">
            <KeymapSettingsTab />
          </SettingsTabPanel>

          <SettingsTabPanel value="profile-groups">
            <ProfileGroupsSettingsTab
              onConnectProfile={onConnectProfile}
              onEditProfile={onEditProfile}
              refreshKey={profilesRefreshKey}
            />
          </SettingsTabPanel>

          <SettingsTabPanel value="data-migration">
            <DataMigrationSettingsTab />
          </SettingsTabPanel>

          <SettingsTabPanel value="security">
            <SecuritySettingsTab confirm={confirm} info={info} />
          </SettingsTabPanel>

          <SettingsTabPanel value="general">
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
              handleRestoreTerminalContentChange={
                settingsHandlers.handleRestoreTerminalContentChange
              }
              handleSftpPasteUploadEnabledChange={
                settingsHandlers.handleSftpPasteUploadEnabledChange
              }
              handleSftpStartDirectoryChange={settingsHandlers.handleSftpStartDirectoryChange}
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
              restoreTerminalContent={config.restore_terminal_content}
              sftpPasteUploadEnabled={config.sftp_paste_upload_enabled}
              sftpStartDirectory={config.sftp_start_directory}
              sftpTransferParallelism={config.sftp_transfer_parallelism}
              sftpUploadLimitKib={config.sftp_upload_limit_kib}
              sftpDownloadLimitKib={config.sftp_download_limit_kib}
              zmodemAutoDetectEnabled={config.zmodem_auto_detect_enabled}
              zmodemDownloadDirectory={config.zmodem_download_directory}
            />
          </SettingsTabPanel>

          <SettingsTabPanel value="updates">
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
          </SettingsTabPanel>
        </Tabs>
      </div>

      <ThemeDialogs
        editingThemeId={editingThemeId}
        setEditingThemeId={setEditingThemeId}
        creatingFromTheme={creatingFromTheme}
        setCreatingFromTheme={setCreatingFromTheme}
        themeGalleryOpen={themeGalleryOpen}
        setThemeGalleryOpen={setThemeGalleryOpen}
        themeImportOpen={themeImportOpen}
        setThemeImportOpen={setThemeImportOpen}
        themeHandlers={themeHandlers}
      />
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
