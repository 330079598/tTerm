import React, { useState, useEffect } from "react"
import {
  Settings,
  Palette,
  Languages,
  Type,
  Shield,
  Info,
  Trash2,
  Check,
  Copy,
  Edit,
  Plus,
  AlertTriangle,
  Lock,
  Unlock,
} from "lucide-react"
import { useTranslation } from "react-i18next"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { invoke } from "@tauri-apps/api/core"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { cn, hslToCssColor } from "@/lib/utils"
import type { PresetThemeId } from "@/types/theme"
import { ThemeEditor } from "@/components/ThemeEditor"

interface SettingsDialogProps {
  onClose: () => void
  defaultTab?: string
}

const FONT_SIZE_OPTIONS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24]

const PRESET_THEME_COLORS: Record<PresetThemeId, string> = {
  default: "hsl(220 13% 12%)",
  light: "hsl(0 0% 98%)",
  ocean: "hsl(200 30% 10%)",
  forest: "hsl(140 25% 12%)",
  sunset: "hsl(20 30% 12%)",
  ubuntu: "hsl(300 100% 6%)",
}

const languages = [
  { code: "en", label: "English", nativeLabel: "English" },
  { code: "zh", label: "中文", nativeLabel: "Chinese" },
]

export const SettingsDialog: React.FC<SettingsDialogProps> = ({
  onClose,
  defaultTab = "appearance",
}) => {
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

  // Font settings state
  const [fontFamily, setFontFamily] = useState(config.font_family)
  const [fontSize, setFontSize] = useState(config.font_size)
  const [systemFonts, setSystemFonts] = useState<string[]>([])
  const [loadingFonts, setLoadingFonts] = useState(true)

  // Secret storage state
  const [password, setPassword] = useState("")
  const [secretError, setSecretError] = useState<string | null>(null)
  const [secretBusy, setSecretBusy] = useState(false)

  // Theme editor state
  const [editingThemeId, setEditingThemeId] = useState<string | null>(null)
  const [creatingFromTheme, setCreatingFromTheme] = useState<string | null>(null)

  useEffect(() => {
    invoke<string[]>("list_fonts")
      .then((fonts) => setSystemFonts(fonts))
      .catch(() => setSystemFonts([]))
      .finally(() => setLoadingFonts(false))
  }, [])

  useEffect(() => {
    refreshSecretStatus().catch(() => {})
  }, [refreshSecretStatus])

  const handleFontSave = async () => {
    await saveConfig({ font_family: fontFamily, font_size: fontSize })
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
      presetThemes.find((t) => t.id === themeId)?.name ||
      customThemes.find((t) => t.id === themeId)?.name ||
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

  const getThemeColor = (themeId: string): string => {
    const customTheme = customThemes.find((t) => t.id === themeId)
    if (customTheme) {
      return hslToCssColor(customTheme.colors.background)
    }
    return PRESET_THEME_COLORS[themeId as PresetThemeId] || "hsl(0 0% 50%)"
  }

  const handleEnableVault = async (checked: boolean) => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await setSecretVaultEnabled(checked)
      if (!checked) {
        setPassword("")
      }
    } catch (err) {
      setSecretError(err instanceof Error ? err.message : String(err))
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
    } catch (err) {
      setSecretError(err instanceof Error ? err.message : String(err))
    } finally {
      setSecretBusy(false)
    }
  }

  const handleLock = async () => {
    setSecretBusy(true)
    setSecretError(null)
    try {
      await lockSecretVault()
    } catch (err) {
      setSecretError(err instanceof Error ? err.message : String(err))
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
    alert(
      `${t("app.title")} - ${t("app.subtitle")}
${t("app.version")}
${t("app.builtWith")}`
    )
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

          <Tabs defaultValue={defaultTab} className="flex h-[calc(85vh-5rem)]">
            {/* Sidebar Navigation */}
            <div className="border-border bg-muted/30 w-48 border-r p-3">
              <TabsList className="flex h-auto w-full flex-col gap-1 bg-transparent">
                <TabsTrigger
                  value="appearance"
                  className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
                >
                  <Palette size={16} />
                  {t("settings.appearance")}
                </TabsTrigger>
                <TabsTrigger
                  value="font"
                  className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
                >
                  <Type size={16} />
                  {t("settings.font")}
                </TabsTrigger>
                <TabsTrigger
                  value="security"
                  className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
                >
                  <Shield size={16} />
                  {t("settings.security")}
                </TabsTrigger>
                <TabsTrigger
                  value="general"
                  className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
                >
                  <Info size={16} />
                  {t("settings.general")}
                </TabsTrigger>
              </TabsList>
            </div>

            {/* Appearance Tab */}
            <TabsContent value="appearance" className="m-0 flex-1 overflow-hidden p-6">
              <ScrollArea className="h-full pr-4">
                <div className="space-y-6">
                  {/* Language */}
                  <div>
                    <div className="mb-3 flex items-center gap-2">
                      <Languages size={16} />
                      <h3 className="text-sm font-semibold">{t("language.title")}</h3>
                    </div>
                    <div className="grid gap-2">
                      {languages.map((lang) => {
                        const isActive = i18n.language === lang.code
                        return (
                          <Card
                            key={lang.code}
                            className="overflow-hidden border-transparent shadow-none"
                          >
                            <CardContent className="p-0">
                              <Button
                                type="button"
                                variant="ghost"
                                onClick={() => handleLanguageChange(lang.code)}
                                className={cn(
                                  "h-auto w-full justify-between rounded-lg px-4 py-3 text-left",
                                  isActive && "bg-muted"
                                )}
                              >
                                <div>
                                  <div className="text-sm font-semibold">{lang.nativeLabel}</div>
                                  <div className="text-muted-foreground text-xs">{lang.label}</div>
                                </div>
                                {isActive && <Check size={16} className="text-primary" />}
                              </Button>
                            </CardContent>
                          </Card>
                        )
                      })}
                    </div>
                  </div>

                  {/* Theme */}
                  <div>
                    <div className="mb-3 flex items-center gap-2">
                      <Palette size={16} />
                      <h3 className="text-sm font-semibold">{t("theme.title")}</h3>
                    </div>

                    {/* Preset themes */}
                    <div className="mb-4">
                      <h4 className="text-muted-foreground mb-2 text-xs font-medium">
                        {t("themeEditor.presetThemes")}
                      </h4>
                      <div className="grid gap-2">
                        {presetThemes.map((theme) => (
                          <Card
                            key={theme.id}
                            className="overflow-hidden border-transparent shadow-none"
                          >
                            <CardContent className="p-0">
                              <div className="flex items-center">
                                <Button
                                  type="button"
                                  variant="ghost"
                                  onClick={() => handleThemeChange(theme.id)}
                                  className={cn(
                                    "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                                    currentTheme === theme.id
                                      ? "border-primary bg-accent"
                                      : "border-transparent"
                                  )}
                                >
                                  <span
                                    className="border-border size-5 shrink-0 rounded-full border"
                                    style={{ background: getThemeColor(theme.id) }}
                                  />
                                  <div className="flex flex-col items-start">
                                    <span className="text-sm leading-none font-medium">
                                      {t(`theme.${theme.id}`)}
                                    </span>
                                    <span className="text-muted-foreground mt-1 text-xs">
                                      {t(`theme.${theme.id}Desc`)}
                                    </span>
                                  </div>
                                  {currentTheme === theme.id && (
                                    <Check size={16} className="text-primary ml-auto" />
                                  )}
                                </Button>
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="sm"
                                  onClick={() => setCreatingFromTheme(theme.id)}
                                  className="h-auto px-2 py-2"
                                  title={t("themeEditor.customize")}
                                >
                                  <Plus size={16} />
                                </Button>
                              </div>
                            </CardContent>
                          </Card>
                        ))}
                      </div>
                    </div>

                    {/* Custom themes */}
                    {customThemes.length > 0 && (
                      <div>
                        <h4 className="text-muted-foreground mb-2 text-xs font-medium">
                          {t("themeEditor.customThemes")}
                        </h4>
                        <div className="grid gap-2">
                          {customThemes.map((theme) => (
                            <Card
                              key={theme.id}
                              className="overflow-hidden border-transparent shadow-none"
                            >
                              <CardContent className="p-0">
                                <div className="flex items-center">
                                  <Button
                                    type="button"
                                    variant="ghost"
                                    onClick={() => handleThemeChange(theme.id)}
                                    className={cn(
                                      "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                                      currentTheme === theme.id
                                        ? "border-primary bg-accent"
                                        : "border-transparent"
                                    )}
                                  >
                                    <span
                                      className="border-border size-5 shrink-0 rounded-full border"
                                      style={{ background: getThemeColor(theme.id) }}
                                    />
                                    <div className="flex flex-col items-start">
                                      <span className="text-sm leading-none font-medium">
                                        {theme.name}
                                      </span>
                                      {theme.description && (
                                        <span className="text-muted-foreground mt-1 text-xs">
                                          {theme.description}
                                        </span>
                                      )}
                                    </div>
                                    {currentTheme === theme.id && (
                                      <Check size={16} className="text-primary ml-auto" />
                                    )}
                                  </Button>
                                  <div className="flex">
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      onClick={() => setEditingThemeId(theme.id)}
                                      className="h-auto px-2 py-2"
                                      title={t("themeEditor.edit")}
                                    >
                                      <Edit size={14} />
                                    </Button>
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      onClick={() => handleDuplicateTheme(theme.id)}
                                      className="h-auto px-2 py-2"
                                      title={t("themeEditor.duplicate")}
                                    >
                                      <Copy size={14} />
                                    </Button>
                                    <Button
                                      type="button"
                                      variant="ghost"
                                      size="sm"
                                      onClick={() => handleDeleteTheme(theme.id)}
                                      className="text-destructive hover:text-destructive h-auto px-2 py-2"
                                      title={t("themeEditor.delete")}
                                    >
                                      <Trash2 size={14} />
                                    </Button>
                                  </div>
                                </div>
                              </CardContent>
                            </Card>
                          ))}
                        </div>
                      </div>
                    )}

                    <Button
                      variant="outline"
                      className="mt-3 w-full"
                      onClick={() => setCreatingFromTheme("default")}
                    >
                      <Plus size={16} className="mr-2" />
                      {t("themeEditor.createNew")}
                    </Button>
                  </div>
                </div>
              </ScrollArea>
            </TabsContent>

            {/* Font Tab */}
            <TabsContent value="font" className="m-0 flex-1 overflow-hidden p-6">
              <ScrollArea className="h-full pr-4">
                <div className="space-y-5">
                  {/* Font Size */}
                  <div>
                    <Label className="mb-2 block">{t("fontSettings.fontSize")}</Label>
                    <div className="flex flex-wrap gap-1.5">
                      {FONT_SIZE_OPTIONS.map((size) => (
                        <Button
                          key={size}
                          type="button"
                          variant={fontSize === size ? "default" : "outline"}
                          size="xs"
                          onClick={() => setFontSize(size)}
                          className={cn(
                            "min-w-[2.25rem]",
                            fontSize !== size && "text-muted-foreground"
                          )}
                        >
                          {size}
                        </Button>
                      ))}
                      <Input
                        type="number"
                        min={6}
                        max={72}
                        value={fontSize}
                        onChange={(e) => {
                          const v = parseInt(e.target.value)
                          if (!isNaN(v) && v >= 6 && v <= 72) setFontSize(v)
                        }}
                        className="h-7 w-16 px-2 text-xs"
                      />
                    </div>
                  </div>

                  {/* Font Family */}
                  <div>
                    <Label className="mb-2 block">{t("fontSettings.fontFamily")}</Label>
                    <Input
                      type="text"
                      value={fontFamily}
                      onChange={(e) => setFontFamily(e.target.value)}
                      placeholder={t("fontSettings.customFont")}
                      className="mb-2"
                    />

                    {loadingFonts ? (
                      <p className="text-muted-foreground text-xs">
                        {t("fontSettings.loadingFonts")}
                      </p>
                    ) : (
                      <ScrollArea className="border-border h-48 rounded border">
                        <div className="p-1">
                          {systemFonts.length === 0 ? (
                            <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                              {t("fontSettings.noFontsFound")}
                            </p>
                          ) : (
                            systemFonts.map((font) => (
                              <button
                                key={font}
                                onClick={() => setFontFamily(`"${font}", monospace`)}
                                className={cn(
                                  "hover:bg-accent w-full rounded px-3 py-1.5 text-left text-sm transition-colors",
                                  fontFamily.includes(font)
                                    ? "bg-accent text-foreground"
                                    : "text-muted-foreground"
                                )}
                                style={{ fontFamily: font }}
                              >
                                {font}
                              </button>
                            ))
                          )}
                        </div>
                      </ScrollArea>
                    )}
                  </div>

                  {/* Preview */}
                  <div>
                    <Label className="mb-2 block">{t("fontSettings.preview")}</Label>
                    <Card>
                      <CardContent
                        className="bg-secondary text-foreground px-4 py-3"
                        style={{ fontFamily, fontSize: `${fontSize}px` }}
                      >
                        The quick brown fox jumps over the lazy dog 0123456789
                      </CardContent>
                    </Card>
                  </div>

                  <Button onClick={handleFontSave} className="w-full">
                    {t("fontSettings.save")}
                  </Button>
                </div>
              </ScrollArea>
            </TabsContent>

            {/* Security Tab */}
            <TabsContent value="security" className="m-0 flex-1 overflow-hidden p-6">
              <ScrollArea className="h-full pr-4">
                <div className="space-y-4">
                  <p className="text-muted-foreground text-sm">{t("secretStorage.description")}</p>

                  <Card>
                    <CardContent className="space-y-2 p-4 text-sm">
                      <div className="flex items-center justify-between gap-3">
                        <span>{t("secretStorage.activeBackend")}</span>
                        <Badge variant="secondary">{backendLabel}</Badge>
                      </div>
                      {secretStatus.message && (
                        <p className="text-muted-foreground text-xs leading-5">
                          {secretStatus.message}
                        </p>
                      )}
                    </CardContent>
                  </Card>

                  {!secretStatus.keyringAvailable && (
                    <Alert className="border-amber-500/40 bg-amber-500/10">
                      <div className="flex items-start gap-2">
                        <AlertTriangle size={16} className="mt-0.5 shrink-0" />
                        <div>
                          <AlertTitle>{t("secretStorage.keyringUnavailable")}</AlertTitle>
                          <AlertDescription className="mt-1 text-xs leading-5">
                            {t("secretStorage.keyringUnavailableDesc")}
                          </AlertDescription>
                        </div>
                      </div>
                    </Alert>
                  )}

                  <Card>
                    <CardContent className="flex items-center justify-between gap-4 p-4">
                      <div>
                        <div className="text-sm font-medium">{t("secretStorage.enableVault")}</div>
                        <div className="text-muted-foreground mt-1 text-xs">
                          {t("secretStorage.enableVaultDesc")}
                        </div>
                      </div>
                      <Switch
                        checked={config.secret_vault_enabled}
                        disabled={secretBusy || secretStatus.keyringAvailable}
                        onCheckedChange={handleEnableVault}
                      />
                    </CardContent>
                  </Card>

                  {config.secret_vault_enabled && !secretStatus.keyringAvailable && (
                    <Card>
                      <CardContent className="space-y-3 p-4">
                        <div>
                          <Label htmlFor="vault-password" className="mb-1.5 block">
                            {t("secretStorage.vaultPassword")}
                          </Label>
                          <Input
                            id="vault-password"
                            type="password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                            disabled={secretBusy}
                          />
                        </div>
                        <div className="flex gap-2">
                          {!secretStatus.strongholdUnlocked ? (
                            <Button
                              onClick={handleUnlock}
                              disabled={secretBusy || password.length === 0}
                            >
                              <Unlock size={14} className="mr-2" />
                              {t("secretStorage.unlockVault")}
                            </Button>
                          ) : (
                            <Button variant="outline" onClick={handleLock} disabled={secretBusy}>
                              <Lock size={14} className="mr-2" />
                              {t("secretStorage.lockVault")}
                            </Button>
                          )}
                        </div>
                      </CardContent>
                    </Card>
                  )}

                  {secretError && (
                    <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
                      <AlertTitle>{t("secretStorage.title")}</AlertTitle>
                      <AlertDescription className="mt-1 text-sm text-current">
                        {secretError}
                      </AlertDescription>
                    </Alert>
                  )}
                </div>
              </ScrollArea>
            </TabsContent>

            {/* General Tab */}
            <TabsContent value="general" className="m-0 flex-1 overflow-hidden p-6">
              <ScrollArea className="h-full pr-4">
                <div className="space-y-3">
                  <Card className="overflow-hidden border-transparent shadow-none">
                    <CardContent className="p-0">
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={handleClearSession}
                        className="h-auto w-full justify-start gap-3 rounded-lg px-4 py-3 text-left"
                      >
                        <Trash2 size={16} className="text-destructive" />
                        <div>
                          <div className="text-sm font-medium">{t("settings.clearSession")}</div>
                          <div className="text-muted-foreground text-xs">
                            {t("settings.clearSessionDesc")}
                          </div>
                        </div>
                      </Button>
                    </CardContent>
                  </Card>

                  <Card className="overflow-hidden border-transparent shadow-none">
                    <CardContent className="p-0">
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={handleAbout}
                        className="h-auto w-full justify-start gap-3 rounded-lg px-4 py-3 text-left"
                      >
                        <Info size={16} />
                        <div>
                          <div className="text-sm font-medium">{t("settings.about")}</div>
                          <div className="text-muted-foreground text-xs">{t("app.subtitle")}</div>
                        </div>
                      </Button>
                    </CardContent>
                  </Card>
                </div>
              </ScrollArea>
            </TabsContent>
          </Tabs>
        </DialogContent>
      </Dialog>

      {/* Theme editor */}
      {editingThemeId && (
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      )}

      {/* Create based on preset theme */}
      {creatingFromTheme && (
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      )}
    </>
  )
}
