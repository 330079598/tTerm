import React, { useEffect, useRef, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { invoke } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useConfirmDialog, useInfoDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import {
  SavedSecretEntry,
  type TabWidthMode,
  type TerminalRenderer,
  useConfig,
} from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { normalizeScrollbackConfig } from "@/lib/scrollback"
import { cn, toErrorMessage } from "@/lib/utils"
import type { PresetThemeId } from "@/types/theme"
import { AppearanceSettingsTab } from "@/components/SettingsDialog/AppearanceSettingsTab"
import { ConnectionSettingsTab } from "@/components/SettingsDialog/ConnectionSettingsTab"
import { DataMigrationSettingsTab } from "@/components/SettingsDialog/DataMigrationSettingsTab"
import { FontSettingsTab } from "@/components/SettingsDialog/FontSettingsTab"
import { GeneralSettingsTab } from "@/components/SettingsDialog/GeneralSettingsTab"
import { LoggingSettingsTab } from "@/components/SettingsDialog/LoggingSettingsTab"
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
import type { UpdateChannel, UpdateCheckFrequency } from "@/lib/updater"

const ThemeEditor = React.lazy(() =>
  import("@/components/ThemeEditor").then((module) => ({
    default: module.ThemeEditor,
  }))
)

const SECRET_STATUS_CACHE_MS = 30_000
const FONT_LOAD_TIMEOUT_MS = 5_000

let cachedSystemFonts: string[] | null = null
let systemFontsPromise: Promise<string[]> | null = null
let lastSecretStatusRefreshAt = 0
let secretStatusPromise: Promise<unknown> | null = null

const fallbackAppVersion = import.meta.env.PACKAGE_VERSION ?? "0.0.0"

function normalizeSettingsTab(tab: string) {
  return tab === "font" ? "terminal" : tab
}

function normalizeScrollbackLines(value: number | undefined) {
  return normalizeScrollbackConfig(value)
}

function getPerfNow() {
  return typeof performance === "undefined" ? 0 : performance.now()
}

function logPerf(label: string, startTime: number, detail?: string) {
  if (!import.meta.env.DEV || typeof performance === "undefined") {
    return
  }

  const duration = Math.round(performance.now() - startTime)
  const suffix = detail ? ` ${detail}` : ""
  console.info(`[perf] ${label}: ${duration}ms${suffix}`)
}

async function loadSystemFontsCached() {
  if (cachedSystemFonts !== null) {
    return cachedSystemFonts
  }

  if (systemFontsPromise) {
    return systemFontsPromise
  }

  const startTime = getPerfNow()
  const fontRequest = invoke<string[]>("list_fonts").then((fonts) => {
    cachedSystemFonts = fonts
    return fonts
  })

  systemFontsPromise = new Promise<string[]>((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      reject(new Error("Timed out while loading system fonts"))
    }, FONT_LOAD_TIMEOUT_MS)

    fontRequest
      .then((fonts) => {
        window.clearTimeout(timeoutId)
        resolve(fonts)
      })
      .catch((error) => {
        window.clearTimeout(timeoutId)
        reject(error)
      })
  }).finally(() => {
    logPerf(
      "settings.fonts",
      startTime,
      cachedSystemFonts ? `count=${cachedSystemFonts.length}` : ""
    )
    systemFontsPromise = null
  })

  return systemFontsPromise
}

function refreshSecretStatusCached(refreshSecretStatus: () => Promise<unknown>) {
  const now = Date.now()
  if (now - lastSecretStatusRefreshAt < SECRET_STATUS_CACHE_MS) {
    return
  }

  if (secretStatusPromise) {
    return
  }

  const startTime = getPerfNow()
  secretStatusPromise = refreshSecretStatus()
    .then(() => {
      lastSecretStatusRefreshAt = Date.now()
    })
    .catch(() => {})
    .finally(() => {
      logPerf("settings.secret_status", startTime)
      secretStatusPromise = null
    })
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
  defaultTab = "profile-groups",
  className,
  onConnectProfile,
  onEditProfile,
  profilesRefreshKey,
}) => {
  const mountStartRef = useRef(getPerfNow())
  const isMountedRef = useRef(true)
  const { t, i18n } = useTranslation()
  const {
    config,
    saveConfig,
    updateLanguage,
    secretStatus,
    refreshSecretStatus,
    setSecretVaultEnabled,
    setSecretStorageMode,
    unlockSecretVault,
    lockSecretVault,
    changeVaultPassword,
    copySecretStore,
    listSavedSecrets,
    deleteSavedSecret,
  } = useConfig()
  const {
    currentTheme,
    presetThemes,
    customThemes,
    presetThemeOverrides,
    setTheme,
    deleteCustomTheme,
    resetPresetTheme,
    duplicateTheme,
  } = useTheme()
  const { toast } = useToast()
  const { saveSettings } = useSettingsSave()
  const [activeTab, setActiveTab] = useState(() => normalizeSettingsTab(defaultTab))
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { prompt, PromptDialog } = usePromptDialog()
  const { info, InfoDialog } = useInfoDialog()

  const [fontFamily, setFontFamily] = useState(config.font_family)
  const [fontSize, setFontSize] = useState(config.font_size)
  const [cursorStyle, setCursorStyle] = useState(config.cursor_style)
  const [scrollbackLines, setScrollbackLines] = useState(() =>
    normalizeScrollbackLines(config.scrollback_lines)
  )
  const [terminalRenderer, setTerminalRenderer] = useState<TerminalRenderer>(
    config.terminal_renderer
  )
  const [terminalPaddingLeftPx, setTerminalPaddingLeftPx] = useState(
    config.terminal_padding_left_px
  )
  const [terminalPaddingRightPx, setTerminalPaddingRightPx] = useState(
    config.terminal_padding_right_px
  )
  const [terminalPaddingBottomPx, setTerminalPaddingBottomPx] = useState(
    config.terminal_padding_bottom_px
  )
  const [systemFonts, setSystemFonts] = useState<string[]>(() => cachedSystemFonts ?? [])
  const [fontsLoaded, setFontsLoaded] = useState(cachedSystemFonts !== null)
  const [loadingFonts, setLoadingFonts] = useState(false)
  const [fontLoadError, setFontLoadError] = useState<string | null>(null)

  const [password, setPassword] = useState("")
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [secretError, setSecretError] = useState<string | null>(null)
  const [secretBusy, setSecretBusy] = useState(false)
  const [savedSecrets, setSavedSecrets] = useState<SavedSecretEntry[]>([])

  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)

  useEffect(() => {
    setActiveTab(normalizeSettingsTab(defaultTab))
  }, [defaultTab])

  useEffect(() => {
    return () => {
      isMountedRef.current = false
    }
  }, [])

  useEffect(() => {
    logPerf("settings.open", mountStartRef.current, `tab=${activeTab}`)
  }, [activeTab])

  useEffect(() => {
    if (activeTab !== "terminal" || fontsLoaded || loadingFonts) {
      return
    }

    setLoadingFonts(true)
    setFontLoadError(null)

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
        setLoadingFonts(false)
      })
  }, [activeTab, fontsLoaded, loadingFonts, t])

  useEffect(() => {
    if (activeTab !== "security") {
      return
    }

    refreshSecretStatusCached(refreshSecretStatus)
    listSavedSecrets()
      .then((entries) => {
        if (isMountedRef.current) {
          setSavedSecrets(entries)
        }
      })
      .catch(() => {
        if (isMountedRef.current) {
          setSavedSecrets([])
        }
      })
  }, [activeTab, listSavedSecrets, refreshSecretStatus])

  const handleFontSave = async () => {
    try {
      await saveConfig({
        font_family: fontFamily,
        font_size: fontSize,
        cursor_style: cursorStyle,
        scrollback_lines: scrollbackLines,
        terminal_renderer: terminalRenderer,
        terminal_padding_left_px: terminalPaddingLeftPx,
        terminal_padding_right_px: terminalPaddingRightPx,
        terminal_padding_bottom_px: terminalPaddingBottomPx,
      })
      toast({
        title: t("fontSettings.saved", { defaultValue: "Settings saved" }),
        description: t("fontSettings.savedDesc", {
          defaultValue:
            "Font, cursor, and scrollback settings have been saved. New terminal tabs will use the updated configuration.",
        }),
      })
    } catch (error) {
      console.error("Failed to save font settings:", error)
      toast({
        title: t("fontSettings.saveFailed", { defaultValue: "Failed to save" }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

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

  const handleThemeChange = async (themeId: string) => {
    try {
      await setTheme(themeId)
    } catch (error) {
      console.error("Failed to save theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("settings.themeChangeFailed", {
          defaultValue: "Failed to save theme.",
        }),
        variant: "destructive",
      })
    }
  }

  const handleTabWidthModeChange = async (mode: TabWidthMode) => {
    await saveSettings({ tab_width_mode: mode })
  }

  const handleTabStandardWidthChange = (width: number) =>
    saveSettings({ tab_standard_width: width })

  const handleUiScaleChange = (scale: number) => saveSettings({ ui_scale_percent: scale })

  const handleDeleteTheme = async (themeId: string) => {
    const confirmed = await confirm({
      title: t("themeEditor.delete"),
      description: t("themeEditor.confirmDelete"),
      confirmText: t("themeEditor.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (!confirmed) return

    try {
      await deleteCustomTheme(themeId)
    } catch (error) {
      console.error("Failed to delete theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("themeEditor.deleteFailed", {
          defaultValue: "Failed to delete theme.",
        }),
        variant: "destructive",
      })
    }
  }

  const handleDuplicateTheme = async (themeId: string) => {
    const sourceName =
      presetThemes.find((theme) => theme.id === themeId)?.name ||
      customThemes.find((theme) => theme.id === themeId)?.name ||
      "Theme"
    const newName = await prompt({
      title: t("themeEditor.duplicate"),
      label: t("themeEditor.duplicateName"),
      defaultValue: `${sourceName} Copy`,
      confirmText: t("themeEditor.duplicate"),
      cancelText: t("common.cancel"),
    })

    if (newName && newName.trim()) {
      try {
        await duplicateTheme(themeId, newName.trim())
      } catch (error) {
        console.error("Failed to duplicate theme:", error)
        toast({
          title: t("common.error", { defaultValue: "Error" }),
          description: t("themeEditor.duplicateFailed", {
            defaultValue: "Failed to duplicate theme.",
          }),
          variant: "destructive",
        })
      }
    }
  }

  const handleResetPresetTheme = async (themeId: PresetThemeId) => {
    try {
      await resetPresetTheme(themeId)
    } catch (error) {
      console.error("Failed to reset preset theme:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("themeEditor.resetFailed", {
          defaultValue: "Failed to reset theme.",
        }),
        variant: "destructive",
      })
    }
  }

  const handleEnableVault = async (checked: boolean) => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await setSecretVaultEnabled(checked)
      if (!checked) {
        setPassword("")
      }
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleSecretStorageModeChange = async (mode: typeof config.secret_storage_mode) => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await setSecretStorageMode(mode)
      toast({
        title: t("secretStorage.modeSaved"),
        description: t(`secretStorage.modeDescriptions.${mode}`),
      })
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleCopySecretStore = async (direction: "systemToVault" | "vaultToSystem") => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      const result = await copySecretStore(direction)
      toast({
        title: t("secretStorage.copyComplete"),
        description: t("secretStorage.copyCompleteDesc", {
          copied: result.copied,
          skipped: result.skipped,
        }),
      })
      await refreshSecretStatus()
      setSavedSecrets(await listSavedSecrets())
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handlePromptUnlockOnStartupChange = async (checked: boolean) => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await saveConfig({ prompt_unlock_vault_on_startup: checked })
      toast({
        title: t("secretStorage.startupPromptSaved"),
        description: checked
          ? t("secretStorage.startupPromptEnabledDesc")
          : t("secretStorage.startupPromptDisabledDesc"),
      })
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleUnlock = async () => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      const shouldEnable = config.secret_storage_mode === "hybrid" || config.secret_vault_enabled
      await unlockSecretVault(password, shouldEnable)
      setPassword("")
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleLock = async () => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await lockSecretVault()
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleChangePassword = async () => {
    if (newPassword !== confirmPassword) {
      await info({
        title: t("secretStorage.changeVaultPassword"),
        description: t("secretStorage.passwordMismatch"),
      })
      return
    }
    setSecretBusy(true)
    setSecretError(null)
    try {
      await changeVaultPassword(currentPassword, newPassword)
      setCurrentPassword("")
      setNewPassword("")
      setConfirmPassword("")
      toast({
        title: t("secretStorage.passwordChanged"),
        description: t("secretStorage.passwordChangedDesc"),
        variant: "success",
      })
    } catch (error) {
      const message = toErrorMessage(error)
      await info({
        title: t("secretStorage.changeVaultPassword"),
        description: message,
      })
    } finally {
      setSecretBusy(false)
    }
  }

  const handleDeleteSavedSecret = async (entry: SavedSecretEntry) => {
    const confirmed = await confirm({
      title: t("secretStorage.deleteSavedSecret"),
      description: t("secretStorage.deleteSavedSecretConfirm", { label: entry.label }),
      confirmText: t("secretStorage.deleteSavedSecret"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (!confirmed) {
      return
    }

    setSecretBusy(true)
    setSecretError(null)
    try {
      await deleteSavedSecret(entry.key)
      setSavedSecrets(await listSavedSecrets())
      toast({
        title: t("secretStorage.savedSecretDeleted"),
        description: t("secretStorage.savedSecretDeletedDesc", { label: entry.label }),
      })
    } catch (error) {
      setSecretError(toErrorMessage(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleClearSession = async () => {
    const confirmed = await confirm({
      title: t("settings.clearSession"),
      description: t("settings.clearSessionConfirm"),
      confirmText: t("settings.clearSession"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (confirmed) {
      window.location.reload()
    }
  }

  const handleRestoreAllSessionConnectionsChange = async (checked: boolean) => {
    await saveSettings({ startup_session_restore_mode: checked ? "all" : "active" })
  }

  const handleSftpPasteUploadEnabledChange = async (checked: boolean) => {
    await saveSettings({ sftp_paste_upload_enabled: checked })
  }

  const handleEnableDevtoolsChange = async () => {
    try {
      await invoke("toggle_devtools", { enable: true })
    } catch (error) {
      console.error("Failed to open devtools:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const handleShowJumpHostConnectionInfoChange = async (checked: boolean) => {
    await saveSettings({ show_jump_host_connection_info: checked })
  }

  const handleMonitorRefreshIntervalChange = async (seconds: number) => {
    await saveSettings({ monitor_refresh_interval_secs: seconds })
  }

  const handleMonitorVisibleMetricsChange = async (
    metrics: typeof config.monitor_visible_metrics
  ) => {
    await saveSettings({ monitor_visible_metrics: metrics })
  }

  const handleTerminalLogEnabledChange = async (enabled: boolean) => {
    if (enabled) {
      const confirmed = await confirm({
        title: t("terminalLogging.confirmTitle"),
        description: t("terminalLogging.confirmDescription"),
        confirmText: t("terminalLogging.enable"),
        cancelText: t("common.cancel"),
      })
      if (!confirmed) return
    }
    await saveSettings({ terminal_log_enabled: enabled })
  }

  const handleUpdateChannelChange = async (channel: UpdateChannel) => {
    await saveSettings({ update_channel: channel })
  }

  const handleAutoDownloadUpdatesChange = async (checked: boolean) => {
    await saveSettings({ auto_download_updates: checked })
  }

  const handleUpdateCheckFrequencyChange = async (frequency: UpdateCheckFrequency) => {
    await saveSettings({ update_check_frequency: frequency })
  }

  const handleAbout = async () => {
    let version = fallbackAppVersion

    try {
      version = await getVersion()
    } catch {
      // Use the build-time version when the Tauri app API is unavailable.
    }

    void info({
      title: t("settings.about"),
      description: (
        <>
          {t("app.title")} - {t("app.subtitle")}
          <br />
          {t("app.version", { version })}
          <br />
          {t("app.builtWith")}
          <br />
          <button
            type="button"
            className="mt-2 inline-flex items-center gap-1.5 text-blue-400 transition-colors hover:text-blue-300"
            onClick={() => openUrl("https://github.com/330079598/tTerm")}
          >
            <svg
              role="img"
              viewBox="0 0 24 24"
              className="size-3.5 fill-current"
              xmlns="http://www.w3.org/2000/svg"
            >
              <title>GitHub</title>
              <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
            </svg>
            <span>GitHub</span>
          </button>
        </>
      ),
      closeText: t("common.close"),
    })
  }

  const backendLabel =
    secretStatus.activeBackend === "system"
      ? t("secretStorage.backends.system")
      : secretStatus.activeBackend === "vault"
        ? t("secretStorage.backends.vault")
        : t("secretStorage.backends.memory")

  return (
    <>
      <div className={cn("bg-background flex h-full min-h-0 flex-col", className)}>
        <DialogHeader className="border-border border-b px-6 pt-6 pb-4">
          <DialogTitle className="flex items-center gap-2">
            <Settings size={18} />
            {t("settings.title")}
          </DialogTitle>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex min-h-0 flex-1">
          <SettingsSidebar />

          <TabsContent value="appearance" className="m-0 flex-1 overflow-y-auto p-6">
            <AppearanceSettingsTab
              currentTheme={currentTheme}
              customThemes={customThemes}
              handleDeleteTheme={handleDeleteTheme}
              handleDuplicateTheme={handleDuplicateTheme}
              handleResetPresetTheme={handleResetPresetTheme}
              handleThemeChange={handleThemeChange}
              handleTabStandardWidthChange={handleTabStandardWidthChange}
              handleTabWidthModeChange={handleTabWidthModeChange}
              handleUiScaleChange={handleUiScaleChange}
              presetThemes={presetThemes}
              presetThemeOverrides={presetThemeOverrides}
              setCreatingFromTheme={setCreatingFromTheme}
              setEditingThemeId={setEditingThemeId}
              tabStandardWidth={config.tab_standard_width}
              tabWidthMode={config.tab_width_mode}
              uiScalePercent={config.ui_scale_percent}
            />
          </TabsContent>

          <TabsContent value="terminal" className="m-0 flex-1 overflow-y-auto p-6">
            <FontSettingsTab
              fontFamily={fontFamily}
              fontSize={fontSize}
              cursorStyle={cursorStyle}
              fontLoadError={fontLoadError}
              handleFontSave={handleFontSave}
              loadingFonts={loadingFonts}
              scrollbackLines={scrollbackLines}
              terminalRenderer={terminalRenderer}
              terminalPaddingLeftPx={terminalPaddingLeftPx}
              terminalPaddingRightPx={terminalPaddingRightPx}
              terminalPaddingBottomPx={terminalPaddingBottomPx}
              setFontFamily={setFontFamily}
              setFontSize={setFontSize}
              setCursorStyle={setCursorStyle}
              setScrollbackLines={setScrollbackLines}
              setTerminalRenderer={setTerminalRenderer}
              setTerminalPaddingLeftPx={setTerminalPaddingLeftPx}
              setTerminalPaddingRightPx={setTerminalPaddingRightPx}
              setTerminalPaddingBottomPx={setTerminalPaddingBottomPx}
              systemFonts={systemFonts}
              fontSizeOptions={FONT_SIZE_OPTIONS}
            />
          </TabsContent>

          <TabsContent value="connection" className="m-0 flex-1 overflow-y-auto p-6">
            <ConnectionSettingsTab
              handleMonitorRefreshIntervalChange={handleMonitorRefreshIntervalChange}
              handleMonitorVisibleMetricsChange={handleMonitorVisibleMetricsChange}
              handleShowJumpHostConnectionInfoChange={handleShowJumpHostConnectionInfoChange}
              monitorRefreshIntervalSecs={config.monitor_refresh_interval_secs}
              monitorVisibleMetrics={config.monitor_visible_metrics}
              showJumpHostConnectionInfo={config.show_jump_host_connection_info}
            />
          </TabsContent>

          <TabsContent value="logging" className="m-0 flex-1 overflow-y-auto p-6">
            <LoggingSettingsTab handleEnabledChange={handleTerminalLogEnabledChange} />
          </TabsContent>

          <TabsContent value="profile-groups" className="m-0 flex-1 overflow-y-auto p-6">
            <ProfileGroupsSettingsTab
              onConnectProfile={onConnectProfile}
              onEditProfile={onEditProfile}
              refreshKey={profilesRefreshKey}
            />
          </TabsContent>

          <TabsContent value="data-migration" className="m-0 flex-1 overflow-y-auto p-6">
            <DataMigrationSettingsTab />
          </TabsContent>

          <TabsContent value="security" className="m-0 flex-1 overflow-y-auto p-6">
            <SecuritySettingsTab
              backendLabel={backendLabel}
              configSecretVaultEnabled={config.secret_vault_enabled}
              handleCopySecretStore={handleCopySecretStore}
              handleEnableVault={handleEnableVault}
              handleLock={handleLock}
              handleChangePassword={handleChangePassword}
              handlePromptUnlockOnStartupChange={handlePromptUnlockOnStartupChange}
              handleSecretStorageModeChange={handleSecretStorageModeChange}
              handleDeleteSavedSecret={handleDeleteSavedSecret}
              handleUnlock={handleUnlock}
              password={password}
              currentPassword={currentPassword}
              newPassword={newPassword}
              confirmPassword={confirmPassword}
              promptUnlockVaultOnStartup={config.prompt_unlock_vault_on_startup}
              secretBusy={secretBusy}
              secretError={secretError}
              savedSecrets={savedSecrets}
              secretStatus={secretStatus}
              secretStorageMode={config.secret_storage_mode}
              setPassword={setPassword}
              setCurrentPassword={setCurrentPassword}
              setNewPassword={setNewPassword}
              setConfirmPassword={setConfirmPassword}
            />
          </TabsContent>

          <TabsContent value="general" className="m-0 flex-1 overflow-y-auto p-6">
            <GeneralSettingsTab
              handleAbout={handleAbout}
              handleClearSession={handleClearSession}
              handleLanguageChange={handleLanguageChange}
              handleRestoreAllSessionConnectionsChange={handleRestoreAllSessionConnectionsChange}
              handleSftpPasteUploadEnabledChange={handleSftpPasteUploadEnabledChange}
              handleEnableDevtoolsChange={handleEnableDevtoolsChange}
              i18nLanguage={i18n.language}
              languages={languages}
              restoreAllSessionConnections={config.startup_session_restore_mode === "all"}
              sftpPasteUploadEnabled={config.sftp_paste_upload_enabled}
            />
          </TabsContent>

          <TabsContent value="updates" className="m-0 flex-1 overflow-y-auto p-6">
            <UpdateSettingsTab
              autoDownloadUpdates={config.auto_download_updates}
              handleAutoDownloadUpdatesChange={handleAutoDownloadUpdatesChange}
              handleUpdateCheckFrequencyChange={handleUpdateCheckFrequencyChange}
              handleUpdateChannelChange={handleUpdateChannelChange}
              handleUpdateCheckComplete={(checkedAt) =>
                saveConfig({ last_update_check_at: checkedAt })
              }
              updateChannel={config.update_channel}
              updateCheckFrequency={config.update_check_frequency}
              lastUpdateCheckAt={config.last_update_check_at}
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
      <ConfirmDialog />
      <PromptDialog />
      <InfoDialog />
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
