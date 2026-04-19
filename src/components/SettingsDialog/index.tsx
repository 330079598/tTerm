import React, { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Settings } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ThemeEditor } from "@/components/ThemeEditor"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"
import { AppearanceSettingsTab } from "@/components/SettingsDialog/AppearanceSettingsTab"
import { FontSettingsTab } from "@/components/SettingsDialog/FontSettingsTab"
import { GeneralSettingsTab } from "@/components/SettingsDialog/GeneralSettingsTab"
import { SecuritySettingsTab } from "@/components/SettingsDialog/SecuritySettingsTab"
import { SettingsSidebar } from "@/components/SettingsDialog/SettingsSidebar"
import {
  FONT_SIZE_OPTIONS,
  languages,
  SettingsDialogProps,
} from "@/components/SettingsDialog/types"

const SECRET_STATUS_CACHE_MS = 30_000

let cachedSystemFonts: string[] | null = null
let systemFontsPromise: Promise<string[]> | null = null
let lastSecretStatusRefreshAt = 0
let secretStatusPromise: Promise<unknown> | null = null

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
  systemFontsPromise = invoke<string[]>("list_fonts")
    .then((fonts) => {
      cachedSystemFonts = fonts
      return fonts
    })
    .finally(() => {
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

export const SettingsDialog: React.FC<SettingsDialogProps> = ({
  onClose,
  defaultTab = "appearance",
}) => {
  const mountStartRef = useRef(getPerfNow())
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
  const { currentTheme, presetThemes, customThemes, setTheme, deleteCustomTheme, duplicateTheme } =
    useTheme()
  const { toast } = useToast()
  const [activeTab, setActiveTab] = useState(defaultTab)

  const [fontFamily, setFontFamily] = useState(config.font_family)
  const [fontSize, setFontSize] = useState(config.font_size)
  const [cursorStyle, setCursorStyle] = useState(config.cursor_style)
  const [scrollbackLines, setScrollbackLines] = useState(config.scrollback_lines || 10000)
  const [systemFonts, setSystemFonts] = useState<string[]>(() => cachedSystemFonts ?? [])
  const [fontsLoaded, setFontsLoaded] = useState(cachedSystemFonts !== null)
  const [loadingFonts, setLoadingFonts] = useState(false)

  const [password, setPassword] = useState("")
  const [secretError, setSecretError] = useState<string | null>(null)
  const [secretBusy, setSecretBusy] = useState(false)

  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)

  useEffect(() => {
    setActiveTab(defaultTab)
  }, [defaultTab])

  useEffect(() => {
    logPerf("settings.open", mountStartRef.current, `tab=${activeTab}`)
  }, [activeTab])

  useEffect(() => {
    if (activeTab !== "font" || fontsLoaded || loadingFonts) {
      return
    }

    let cancelled = false
    setLoadingFonts(true)

    loadSystemFontsCached()
      .then((fonts) => {
        if (cancelled) {
          return
        }
        setSystemFonts(fonts)
      })
      .catch(() => {
        if (cancelled) {
          return
        }
        setSystemFonts([])
      })
      .finally(() => {
        if (cancelled) {
          return
        }
        setFontsLoaded(true)
        setLoadingFonts(false)
      })

    return () => {
      cancelled = true
    }
  }, [activeTab, fontsLoaded, loadingFonts])

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
    if (confirm(t("themeEditor.confirmDelete"))) {
      try {
        await deleteCustomTheme(themeId)
      } catch (error) {
        console.error("Failed to delete theme:", error)
      }
    }
  }

  const handleDuplicateTheme = async (themeId: string) => {
    const sourceName =
      presetThemes.find((theme) => theme.id === themeId)?.name ||
      customThemes.find((theme) => theme.id === themeId)?.name ||
      "Theme"
    const newName = prompt(t("themeEditor.duplicateName"), `${sourceName} Copy`)

    if (newName && newName.trim()) {
      try {
        await duplicateTheme(themeId, newName.trim())
      } catch (error) {
        console.error("Failed to duplicate theme:", error)
      }
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
    if (confirm(t("settings.clearSessionConfirm"))) {
      window.location.reload()
    }
  }

  const handleAbout = () => {
    alert(`${t("app.title")} - ${t("app.subtitle")}
${t("app.version")}
${t("app.builtWith")}`)
  }

  const backendLabel =
    secretStatus.activeBackend === "system"
      ? t("secretStorage.backends.system")
      : secretStatus.activeBackend === "vault"
        ? t("secretStorage.backends.vault")
        : t("secretStorage.backends.memory")

  return (
    <>
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="max-h-[85vh] p-0 sm:max-w-4xl">
          <DialogHeader className="px-6 pt-6 pb-4">
            <DialogTitle className="flex items-center gap-2">
              <Settings size={18} />
              {t("settings.title")}
            </DialogTitle>
          </DialogHeader>

          <Tabs value={activeTab} onValueChange={setActiveTab} className="flex h-[calc(85vh-5rem)]">
            <SettingsSidebar />

            <TabsContent value="appearance" className="m-0 flex-1 overflow-y-auto p-6">
              <AppearanceSettingsTab
                currentTheme={currentTheme}
                customThemes={customThemes}
                handleDeleteTheme={handleDeleteTheme}
                handleDuplicateTheme={handleDuplicateTheme}
                handleLanguageChange={handleLanguageChange}
                handleThemeChange={handleThemeChange}
                i18nLanguage={i18n.language}
                languages={languages}
                presetThemes={presetThemes}
                setCreatingFromTheme={setCreatingFromTheme}
                setEditingThemeId={setEditingThemeId}
              />
            </TabsContent>

            <TabsContent value="font" className="m-0 flex-1 overflow-y-auto p-6">
              <FontSettingsTab
                fontFamily={fontFamily}
                fontSize={fontSize}
                cursorStyle={cursorStyle}
                handleFontSave={handleFontSave}
                loadingFonts={loadingFonts || !fontsLoaded}
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
              />
            </TabsContent>
          </Tabs>
        </DialogContent>
      </Dialog>

      {editingThemeId && (
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      )}

      {creatingFromTheme && (
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      )}
    </>
  )
}
