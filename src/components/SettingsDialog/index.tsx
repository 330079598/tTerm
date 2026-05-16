import React, { useEffect, useRef, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { invoke } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ThemeEditor } from "@/components/ThemeEditor"
import { useConfirmDialog, useInfoDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"
import { cn } from "@/lib/utils"
import type { PresetThemeId } from "@/types/theme"
import { AppearanceSettingsTab } from "@/components/SettingsDialog/AppearanceSettingsTab"
import { FontSettingsTab } from "@/components/SettingsDialog/FontSettingsTab"
import { GeneralSettingsTab } from "@/components/SettingsDialog/GeneralSettingsTab"
import { SecuritySettingsTab } from "@/components/SettingsDialog/SecuritySettingsTab"
import { SettingsSidebar } from "@/components/SettingsDialog/SettingsSidebar"
import { UpdateSettingsTab } from "@/components/SettingsDialog/UpdateSettingsTab"
import {
  FONT_SIZE_OPTIONS,
  languages,
  SettingsDialogProps,
  SettingsPanelProps,
} from "@/components/SettingsDialog/types"
import type { UpdateChannel } from "@/lib/updater"

const SECRET_STATUS_CACHE_MS = 30_000
const FONT_LOAD_TIMEOUT_MS = 5_000

let cachedSystemFonts: string[] | null = null
let systemFontsPromise: Promise<string[]> | null = null
let lastSecretStatusRefreshAt = 0
let secretStatusPromise: Promise<unknown> | null = null

const fallbackAppVersion = import.meta.env.PACKAGE_VERSION ?? "0.0.0"

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
  defaultTab = "appearance",
  className,
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
    unlockSecretVault,
    lockSecretVault,
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
  const [activeTab, setActiveTab] = useState(defaultTab)
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { prompt, PromptDialog } = usePromptDialog()
  const { info, InfoDialog } = useInfoDialog()

  const [fontFamily, setFontFamily] = useState(config.font_family)
  const [fontSize, setFontSize] = useState(config.font_size)
  const [cursorStyle, setCursorStyle] = useState(config.cursor_style)
  const [scrollbackLines, setScrollbackLines] = useState(config.scrollback_lines || 10000)
  const [systemFonts, setSystemFonts] = useState<string[]>(() => cachedSystemFonts ?? [])
  const [fontsLoaded, setFontsLoaded] = useState(cachedSystemFonts !== null)
  const [loadingFonts, setLoadingFonts] = useState(false)
  const [fontLoadError, setFontLoadError] = useState<string | null>(null)

  const [password, setPassword] = useState("")
  const [secretError, setSecretError] = useState<string | null>(null)
  const [secretBusy, setSecretBusy] = useState(false)

  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)

  useEffect(() => {
    setActiveTab(defaultTab)
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
    if (activeTab !== "font" || fontsLoaded || loadingFonts) {
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
  }, [activeTab, refreshSecretStatus])

  const handleFontSave = async () => {
    try {
      await saveConfig({
        font_family: fontFamily,
        font_size: fontSize,
        cursor_style: cursorStyle,
        scrollback_lines: scrollbackLines,
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
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      })
    }
  }

  const handleLanguageChange = async (langCode: string) => {
    await i18n.changeLanguage(langCode)
    try {
      await updateLanguage(langCode)
    } catch (error) {
      console.error("Failed to save language:", error)
    }
  }

  const handleThemeChange = async (themeId: string) => {
    try {
      await setTheme(themeId)
    } catch (error) {
      console.error("Failed to save theme:", error)
    }
  }

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
      }
    }
  }

  const handleResetPresetTheme = async (themeId: PresetThemeId) => {
    try {
      await resetPresetTheme(themeId)
    } catch (error) {
      console.error("Failed to reset preset theme:", error)
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
      setSecretError(error instanceof Error ? error.message : String(error))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleUnlock = async () => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await unlockSecretVault(password, config.secret_vault_enabled)
      setPassword("")
    } catch (error) {
      setSecretError(error instanceof Error ? error.message : String(error))
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
      setSecretError(error instanceof Error ? error.message : String(error))
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
    try {
      await saveConfig({ startup_session_restore_mode: checked ? "all" : "active" })
    } catch (error) {
      console.error("Failed to save startup session restore mode:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      })
    }
  }

  const handleShowJumpHostConnectionInfoChange = async (checked: boolean) => {
    try {
      await saveConfig({ show_jump_host_connection_info: checked })
    } catch (error) {
      console.error("Failed to save jump host connection info preference:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      })
    }
  }

  const handleUpdateChannelChange = async (channel: UpdateChannel) => {
    try {
      await saveConfig({ update_channel: channel })
    } catch (error) {
      console.error("Failed to save update channel:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      })
    }
  }

  const handleAutoDownloadUpdatesChange = async (checked: boolean) => {
    try {
      await saveConfig({ auto_download_updates: checked })
    } catch (error) {
      console.error("Failed to save auto update preference:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      })
    }
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
              handleLanguageChange={handleLanguageChange}
              handleResetPresetTheme={handleResetPresetTheme}
              handleThemeChange={handleThemeChange}
              i18nLanguage={i18n.language}
              languages={languages}
              presetThemes={presetThemes}
              presetThemeOverrides={presetThemeOverrides}
              setCreatingFromTheme={setCreatingFromTheme}
              setEditingThemeId={setEditingThemeId}
            />
          </TabsContent>

          <TabsContent value="font" className="m-0 flex-1 overflow-y-auto p-6">
            <FontSettingsTab
              fontFamily={fontFamily}
              fontSize={fontSize}
              cursorStyle={cursorStyle}
              fontLoadError={fontLoadError}
              handleFontSave={handleFontSave}
              loadingFonts={loadingFonts}
              scrollbackLines={scrollbackLines}
              setFontFamily={setFontFamily}
              setFontSize={setFontSize}
              setCursorStyle={setCursorStyle}
              setScrollbackLines={setScrollbackLines}
              systemFonts={systemFonts}
              fontSizeOptions={FONT_SIZE_OPTIONS}
            />
          </TabsContent>

          <TabsContent value="security" className="m-0 flex-1 overflow-y-auto p-6">
            <SecuritySettingsTab
              backendLabel={backendLabel}
              configSecretVaultEnabled={config.secret_vault_enabled}
              handleEnableVault={handleEnableVault}
              handleLock={handleLock}
              handleUnlock={handleUnlock}
              password={password}
              secretBusy={secretBusy}
              secretError={secretError}
              secretStatus={secretStatus}
              setPassword={setPassword}
            />
          </TabsContent>

          <TabsContent value="general" className="m-0 flex-1 overflow-y-auto p-6">
            <GeneralSettingsTab
              handleAbout={handleAbout}
              handleClearSession={handleClearSession}
              handleRestoreAllSessionConnectionsChange={handleRestoreAllSessionConnectionsChange}
              handleShowJumpHostConnectionInfoChange={handleShowJumpHostConnectionInfoChange}
              restoreAllSessionConnections={config.startup_session_restore_mode === "all"}
              showJumpHostConnectionInfo={config.show_jump_host_connection_info}
            />
          </TabsContent>

          <TabsContent value="updates" className="m-0 flex-1 overflow-y-auto p-6">
            <UpdateSettingsTab
              autoDownloadUpdates={config.auto_download_updates}
              handleAutoDownloadUpdatesChange={handleAutoDownloadUpdatesChange}
              handleUpdateChannelChange={handleUpdateChannelChange}
              updateChannel={config.update_channel}
            />
          </TabsContent>
        </Tabs>
      </div>

      {editingThemeId && (
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      )}

      {creatingFromTheme && (
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      )}
      <ConfirmDialog />
      <PromptDialog />
      <InfoDialog />
    </>
  )
}

export const SettingsDialog: React.FC<SettingsDialogProps> = ({ onClose, defaultTab }) => {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[85vh] p-0 sm:max-w-4xl">
        <SettingsPanel defaultTab={defaultTab} className="h-[calc(85vh-2rem)]" />
      </DialogContent>
    </Dialog>
  )
}
