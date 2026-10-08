import React from "react"
import {
  ChevronDown,
  ChevronUp,
  Copy,
  Edit,
  GalleryHorizontal,
  Import,
  Library,
  MonitorCog,
  Palette,
  Plus,
  RotateCcw,
  AppWindow,
  Trash2,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { ThemeCard } from "@/components/ThemeCard"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import {
  FollowSystemThemeSetting,
  LibraryThemeCards,
} from "@/components/SettingsDialog/ThemeLibrarySettings"
import {
  applyUiScalePercent,
  getDetectedPlatform,
  isWindowBlurSupported,
  WINDOW_BLUR_RADIUS_RANGE,
  WINDOW_OPACITY_PERCENT_RANGE,
  type TabWidthMode,
  type WindowBlurMaterial,
} from "@/contexts/ConfigContext"
import { isImeKeyEvent } from "@/lib/ime"
import { setWindowBlur } from "@/lib/themePreloader"
import { useSyncedState } from "@/hooks/useSyncedState"
import type { CustomTheme, PresetTheme, PresetThemeId } from "@/types/theme"

interface AppearanceSettingsTabProps {
  currentTheme: string
  customThemes: CustomTheme[]
  handleDeleteTheme: (themeId: string) => Promise<void>
  handleDuplicateTheme: (themeId: string) => Promise<void>
  handleResetPresetTheme: (themeId: PresetThemeId) => Promise<void>
  handleThemeChange: (themeId: string) => Promise<void>
  handleTabStandardWidthChange: (width: number) => Promise<boolean>
  handleTabWidthModeChange: (mode: TabWidthMode) => Promise<void>
  handleUiScaleChange: (scale: number) => Promise<boolean>
  handleWindowBlurChange: (enabled: boolean) => Promise<boolean>
  handleWindowBlurMaterialChange: (material: WindowBlurMaterial) => Promise<boolean>
  handleWindowBlurRadiusChange: (radius: number) => Promise<boolean>
  handleWindowOpacityChange: (percent: number) => Promise<boolean>
  onOpenThemeGallery: () => void
  onOpenThemeImport: () => void
  presetThemes: PresetTheme[]
  presetThemeOverrides: CustomTheme[]
  setCreatingFromTheme: React.Dispatch<React.SetStateAction<string | null>>
  setEditingThemeId: React.Dispatch<React.SetStateAction<string | null>>
  tabStandardWidth: number
  tabWidthMode: TabWidthMode
  uiScalePercent: number
  windowOpacityPercent: number
  windowBlur: boolean
  windowBlurMaterial: WindowBlurMaterial
  windowBlurRadius: number
}

export const AppearanceSettingsTab: React.FC<AppearanceSettingsTabProps> = ({
  currentTheme,
  customThemes,
  handleDeleteTheme,
  handleDuplicateTheme,
  handleResetPresetTheme,
  handleThemeChange,
  handleTabStandardWidthChange,
  handleTabWidthModeChange,
  handleUiScaleChange,
  handleWindowBlurChange,
  handleWindowBlurMaterialChange,
  handleWindowBlurRadiusChange,
  handleWindowOpacityChange,
  onOpenThemeGallery,
  onOpenThemeImport,
  presetThemes,
  presetThemeOverrides,
  setCreatingFromTheme,
  setEditingThemeId,
  tabStandardWidth,
  tabWidthMode,
  uiScalePercent,
  windowOpacityPercent,
  windowBlur,
  windowBlurMaterial,
  windowBlurRadius,
}) => {
  const { t } = useTranslation()
  const [tabWidthDraft, setTabWidthDraft] = useSyncedState(String(tabStandardWidth))
  const [uiScaleDraft, setUiScaleDraft] = useSyncedState(uiScalePercent)
  const [savingUiScale, setSavingUiScale] = React.useState(false)
  const [showAllPresetThemes, setShowAllPresetThemes] = React.useState(false)
  const savingUiScaleRef = React.useRef(false)
  const savedUiScaleRef = React.useRef(uiScalePercent)
  const mountedRef = React.useRef(true)

  React.useEffect(() => {
    savedUiScaleRef.current = uiScalePercent
  }, [uiScalePercent])

  React.useEffect(
    () => () => {
      mountedRef.current = false
      applyUiScalePercent(savedUiScaleRef.current)
    },
    []
  )

  const previewUiScale = React.useCallback(
    (scale: number) => {
      setUiScaleDraft(scale)
      applyUiScalePercent(scale)
    },
    [setUiScaleDraft]
  )

  const commitUiScale = React.useCallback(async () => {
    if (uiScaleDraft === uiScalePercent || savingUiScaleRef.current) return
    savingUiScaleRef.current = true
    setSavingUiScale(true)
    const saved = await handleUiScaleChange(uiScaleDraft)
    if (!mountedRef.current) return
    savingUiScaleRef.current = false
    setSavingUiScale(false)
    if (!saved) {
      previewUiScale(uiScalePercent)
    }
  }, [handleUiScaleChange, previewUiScale, uiScaleDraft, uiScalePercent])

  const [blurRadiusDraft, setBlurRadiusDraft] = useSyncedState(windowBlurRadius)
  const [opacityDraft, setOpacityDraft] = useSyncedState(windowOpacityPercent)
  const savedWindowBlurRef = React.useRef({
    windowBlur,
    windowBlurMaterial,
    windowBlurRadius,
    windowOpacityPercent,
  })

  React.useEffect(() => {
    savedWindowBlurRef.current = {
      windowBlur,
      windowBlurMaterial,
      windowBlurRadius,
      windowOpacityPercent,
    }
  }, [windowBlur, windowBlurMaterial, windowBlurRadius, windowOpacityPercent])

  // Undo a slider preview that was never saved.
  React.useEffect(
    () => () => {
      const saved = savedWindowBlurRef.current
      setWindowBlur({
        enabled: saved.windowBlur,
        radius: saved.windowBlurRadius,
        material: saved.windowBlurMaterial,
        opacity: saved.windowOpacityPercent / 100,
      })
    },
    []
  )

  const previewWindowBlur = React.useCallback(
    (radius: number, opacityPercent: number) => {
      setBlurRadiusDraft(radius)
      setOpacityDraft(opacityPercent)
      setWindowBlur({
        enabled: true,
        radius,
        material: windowBlurMaterial,
        opacity: opacityPercent / 100,
      })
    },
    [setBlurRadiusDraft, setOpacityDraft, windowBlurMaterial]
  )

  const commitWindowBlurRadius = React.useCallback(async () => {
    if (blurRadiusDraft === windowBlurRadius) return
    const saved = await handleWindowBlurRadiusChange(blurRadiusDraft)
    if (!saved && mountedRef.current) {
      previewWindowBlur(windowBlurRadius, opacityDraft)
    }
  }, [
    blurRadiusDraft,
    handleWindowBlurRadiusChange,
    opacityDraft,
    previewWindowBlur,
    windowBlurRadius,
  ])

  const commitWindowOpacity = React.useCallback(async () => {
    if (opacityDraft === windowOpacityPercent) return
    const saved = await handleWindowOpacityChange(opacityDraft)
    if (!saved && mountedRef.current) {
      previewWindowBlur(blurRadiusDraft, windowOpacityPercent)
    }
  }, [
    blurRadiusDraft,
    handleWindowOpacityChange,
    opacityDraft,
    previewWindowBlur,
    windowOpacityPercent,
  ])

  const commitTabStandardWidth = React.useCallback(async () => {
    const value = Number.parseInt(tabWidthDraft, 10)
    if (Number.isNaN(value)) {
      setTabWidthDraft(String(tabStandardWidth))
      return
    }

    const normalizedValue = Math.min(Math.max(value, 80), 300)
    setTabWidthDraft(String(normalizedValue))
    if (normalizedValue !== tabStandardWidth) {
      const saved = await handleTabStandardWidthChange(normalizedValue)
      if (!saved) {
        setTabWidthDraft(String(tabStandardWidth))
      }
    }
  }, [handleTabStandardWidthChange, setTabWidthDraft, tabStandardWidth, tabWidthDraft])
  const getPresetTone = (themeId: PresetThemeId) => {
    if (themeId === "default" || themeId === "light") {
      return {
        label: t("theme.workbenchRecommended", { defaultValue: "Recommended" }),
        variant: "secondary" as const,
      }
    }

    return {
      label: t("theme.expressive", { defaultValue: "Expressive" }),
      variant: "outline" as const,
    }
  }

  const compactPresetThemes = presetThemes.filter(
    (theme) => theme.id === "default" || theme.id === "light" || theme.id === currentTheme
  )
  const visiblePresetThemes = showAllPresetThemes ? presetThemes : compactPresetThemes
  const hiddenPresetThemeCount = presetThemes.length - compactPresetThemes.length

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Palette size={16} />}
          title={t("theme.title")}
          description={t("theme.description", {
            defaultValue: "Choose the UI and terminal palette used across tTerm.",
          })}
        >
          <FollowSystemThemeSetting />

          <div className="mb-4">
            <h4 className="text-muted-foreground mb-2 text-xs font-medium">
              {t("themeEditor.presetThemes")}
            </h4>
            <div className="grid gap-1.5">
              {visiblePresetThemes.map((theme) => {
                const hasOverride = presetThemeOverrides.some(
                  (override) => override.id === theme.id
                )
                const tone = getPresetTone(theme.id as PresetThemeId)

                return (
                  <ThemeCard
                    key={theme.id}
                    currentTheme={currentTheme}
                    description={
                      hasOverride
                        ? (theme.description ?? t("themeEditor.noDescription"))
                        : t(`theme.${theme.id}Desc`)
                    }
                    name={hasOverride ? theme.name : t(`theme.${theme.id}`)}
                    onSelect={() => handleThemeChange(theme.id)}
                    compactPreview
                    toneLabel={tone.label}
                    toneVariant={tone.variant}
                    theme={theme}
                    actionSlot={
                      <div className="flex">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => setEditingThemeId(theme.id)}
                              aria-label={t("themeEditor.edit")}
                              className="h-auto px-2 py-2"
                            >
                              <Edit size={14} />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{t("themeEditor.edit")}</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleDuplicateTheme(theme.id)}
                              aria-label={t("themeEditor.duplicate")}
                              className="h-auto px-2 py-2"
                            >
                              <Copy size={14} />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{t("themeEditor.duplicate")}</TooltipContent>
                        </Tooltip>
                        {hasOverride && (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => handleResetPresetTheme(theme.id as PresetThemeId)}
                                aria-label={t("themeEditor.restorePreset")}
                                className="h-auto px-2 py-2"
                              >
                                <RotateCcw size={14} />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent>{t("themeEditor.restorePreset")}</TooltipContent>
                          </Tooltip>
                        )}
                      </div>
                    }
                  />
                )
              })}
            </div>
            {hiddenPresetThemeCount > 0 && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="text-muted-foreground mt-1.5 w-full"
                aria-expanded={showAllPresetThemes}
                onClick={() => setShowAllPresetThemes((visible) => !visible)}
              >
                {showAllPresetThemes ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                {showAllPresetThemes
                  ? t("themeEditor.showFewerPresets", { defaultValue: "Show fewer themes" })
                  : t("themeEditor.showMorePresets", {
                      defaultValue: "Show {{count}} more themes",
                      count: hiddenPresetThemeCount,
                    })}
              </Button>
            )}
          </div>

          <LibraryThemeCards
            currentTheme={currentTheme}
            onCopy={(themeId) => void handleDuplicateTheme(themeId)}
            onSelect={(themeId) => void handleThemeChange(themeId)}
          />

          {customThemes.length > 0 && (
            <div>
              <h4 className="text-muted-foreground mb-2 text-xs font-medium">
                {t("themeEditor.customThemes")}
              </h4>
              <div className="grid gap-2">
                {customThemes.map((theme) => (
                  <ThemeCard
                    key={theme.id}
                    currentTheme={currentTheme}
                    description={theme.description || t("themeEditor.noDescription")}
                    name={theme.name}
                    onSelect={() => handleThemeChange(theme.id)}
                    compactPreview
                    theme={theme}
                    actionSlot={
                      <div className="flex">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => setEditingThemeId(theme.id)}
                              aria-label={t("themeEditor.edit")}
                              className="h-auto px-2 py-2"
                            >
                              <Edit size={14} />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{t("themeEditor.edit")}</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleDuplicateTheme(theme.id)}
                              aria-label={t("themeEditor.duplicate")}
                              className="h-auto px-2 py-2"
                            >
                              <Copy size={14} />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{t("themeEditor.duplicate")}</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              onClick={() => handleDeleteTheme(theme.id)}
                              aria-label={t("themeEditor.delete")}
                              className="text-destructive hover:text-destructive h-auto px-2 py-2"
                            >
                              <Trash2 size={14} />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>{t("themeEditor.delete")}</TooltipContent>
                        </Tooltip>
                      </div>
                    }
                  />
                ))}
              </div>
            </div>
          )}

          <div className="mt-3 grid gap-2 sm:grid-cols-3">
            <Button variant="outline" onClick={onOpenThemeGallery}>
              <Library size={16} />
              {t("themeGallery.open", { defaultValue: "Browse theme library" })}
            </Button>
            <Button variant="outline" onClick={onOpenThemeImport}>
              <Import size={16} />
              {t("themeImport.open", { defaultValue: "Import theme" })}
            </Button>
            <Button variant="outline" onClick={() => setCreatingFromTheme("default")}>
              <Plus size={16} />
              {t("themeEditor.createNew")}
            </Button>
          </div>
        </SettingsSection>

        <SettingsSection
          icon={<MonitorCog size={16} />}
          title={t("settings.interface", { defaultValue: "Interface" })}
          description={t("settings.interfaceScaleDesc", {
            defaultValue:
              "Scale application text without changing controls, spacing, terminal text, or editor content.",
          })}
        >
          <SettingsRow
            title={t("settings.interfaceScale", { defaultValue: "Interface font scale" })}
            description={t("settings.interfaceScaleRange", {
              defaultValue: "Choose a text scale from 80% to 200%.",
            })}
          >
            <div className="flex w-full max-w-sm items-center gap-3">
              <input
                type="range"
                min={80}
                max={200}
                step={10}
                value={uiScaleDraft}
                aria-label={t("settings.interfaceScale", {
                  defaultValue: "Interface font scale",
                })}
                aria-valuetext={`${uiScaleDraft}%`}
                disabled={savingUiScale}
                onChange={(event) => previewUiScale(Number(event.target.value))}
                onPointerUp={() => void commitUiScale()}
                onKeyUp={() => void commitUiScale()}
                onBlur={() => void commitUiScale()}
                className="accent-primary min-w-0 flex-1 cursor-pointer"
              />
              <output className="w-12 text-right text-sm font-medium" aria-live="polite">
                {uiScaleDraft}%
              </output>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={uiScaleDraft === 100 || savingUiScale}
                onClick={async () => {
                  if (savingUiScaleRef.current) return
                  previewUiScale(100)
                  savingUiScaleRef.current = true
                  setSavingUiScale(true)
                  const saved = await handleUiScaleChange(100)
                  if (!mountedRef.current) return
                  savingUiScaleRef.current = false
                  setSavingUiScale(false)
                  if (!saved) previewUiScale(uiScalePercent)
                }}
              >
                <RotateCcw size={14} />
                {t("settings.resetScale", { defaultValue: "Reset" })}
              </Button>
            </div>
          </SettingsRow>
        </SettingsSection>

        {isWindowBlurSupported() && (
          <SettingsSection
            icon={<AppWindow size={16} />}
            title={t("settings.window", { defaultValue: "Window" })}
          >
            <SettingsRow
              title={t("settings.windowBlur", { defaultValue: "Background blur" })}
              description={t("settings.windowBlurDesc", {
                defaultValue:
                  "Blur what is behind the window and show it through the theme background.",
              })}
              action={
                <Switch
                  aria-label={t("settings.windowBlur", { defaultValue: "Background blur" })}
                  checked={windowBlur}
                  onCheckedChange={(enabled) => void handleWindowBlurChange(enabled)}
                />
              }
            />
            {windowBlur && getDetectedPlatform() === "windows" && (
              <SettingsRow
                title={t("settings.windowBlurMaterial", { defaultValue: "Material" })}
                description={t("settings.windowBlurMaterialDesc", {
                  defaultValue:
                    "Acrylic blurs what is behind the window; on Windows 10 and early Windows 11 the window can lag while dragged. Mica tints with the desktop wallpaper instead and needs Windows 11.",
                })}
                action={
                  <Select
                    aria-label={t("settings.windowBlurMaterial", { defaultValue: "Material" })}
                    value={windowBlurMaterial}
                    onChange={(event) =>
                      void handleWindowBlurMaterialChange(event.target.value as WindowBlurMaterial)
                    }
                    className="w-44"
                  >
                    <option value="acrylic">
                      {t("settings.windowBlurMaterialAcrylic", { defaultValue: "Acrylic" })}
                    </option>
                    <option value="mica">
                      {t("settings.windowBlurMaterialMica", { defaultValue: "Mica" })}
                    </option>
                  </Select>
                }
              />
            )}
            {windowBlur && (
              <>
                {getDetectedPlatform() === "macos" && (
                  <SettingsRow
                    title={t("settings.windowBlurRadius", { defaultValue: "Blur radius" })}
                    description={t("settings.windowBlurRadiusDesc", {
                      defaultValue: "Higher values blur the background more.",
                    })}
                  >
                    <div className="flex w-full max-w-sm items-center gap-3">
                      <input
                        type="range"
                        min={WINDOW_BLUR_RADIUS_RANGE.min}
                        max={WINDOW_BLUR_RADIUS_RANGE.max}
                        step={1}
                        value={blurRadiusDraft}
                        aria-label={t("settings.windowBlurRadius", { defaultValue: "Blur radius" })}
                        onChange={(event) =>
                          previewWindowBlur(Number(event.target.value), opacityDraft)
                        }
                        onPointerUp={() => void commitWindowBlurRadius()}
                        onKeyUp={() => void commitWindowBlurRadius()}
                        onBlur={() => void commitWindowBlurRadius()}
                        className="accent-primary min-w-0 flex-1 cursor-pointer"
                      />
                      <output className="w-12 text-right text-sm font-medium" aria-live="polite">
                        {blurRadiusDraft}
                      </output>
                    </div>
                  </SettingsRow>
                )}
                <SettingsRow
                  title={t("settings.windowOpacity", { defaultValue: "Background opacity" })}
                  description={t("settings.windowOpacityDesc", {
                    defaultValue: "Lower values let more of the blurred background show through.",
                  })}
                >
                  <div className="flex w-full max-w-sm items-center gap-3">
                    <input
                      type="range"
                      min={WINDOW_OPACITY_PERCENT_RANGE.min}
                      max={WINDOW_OPACITY_PERCENT_RANGE.max}
                      step={5}
                      value={opacityDraft}
                      aria-label={t("settings.windowOpacity", {
                        defaultValue: "Background opacity",
                      })}
                      aria-valuetext={`${opacityDraft}%`}
                      onChange={(event) =>
                        previewWindowBlur(blurRadiusDraft, Number(event.target.value))
                      }
                      onPointerUp={() => void commitWindowOpacity()}
                      onKeyUp={() => void commitWindowOpacity()}
                      onBlur={() => void commitWindowOpacity()}
                      className="accent-primary min-w-0 flex-1 cursor-pointer"
                    />
                    <output className="w-12 text-right text-sm font-medium" aria-live="polite">
                      {opacityDraft}%
                    </output>
                  </div>
                </SettingsRow>
              </>
            )}
          </SettingsSection>
        )}

        <SettingsSection
          icon={<GalleryHorizontal size={16} />}
          title={t("settings.tabWidth", { defaultValue: "Tab width" })}
          description={t("settings.tabWidthDesc", {
            defaultValue: "Control the width of title-bar and split-group tabs.",
          })}
        >
          <SettingsRow
            title={t("settings.tabWidthMode", { defaultValue: "Width mode" })}
            description={t("settings.tabWidthModeDesc", {
              defaultValue:
                "Adaptive width follows each title. Standard width gives every tab the same width.",
            })}
          >
            <div className="grid max-w-xs gap-4 sm:grid-cols-2">
              <div className="space-y-1.5">
                <Label htmlFor="tab-width-mode" className="text-muted-foreground text-xs">
                  {t("settings.tabWidthModeLabel", { defaultValue: "Mode" })}
                </Label>
                <Select
                  id="tab-width-mode"
                  value={tabWidthMode}
                  onChange={(event) =>
                    void handleTabWidthModeChange(event.target.value as TabWidthMode)
                  }
                >
                  <option value="adaptive">
                    {t("settings.tabWidthAdaptive", { defaultValue: "Adaptive width" })}
                  </option>
                  <option value="standard">
                    {t("settings.tabWidthStandard", { defaultValue: "Standard width" })}
                  </option>
                </Select>
              </div>

              {tabWidthMode === "standard" && (
                <div className="space-y-1.5">
                  <Label htmlFor="tab-standard-width" className="text-muted-foreground text-xs">
                    {t("settings.tabStandardWidth", { defaultValue: "Standard width" })}
                  </Label>
                  <div className="flex items-center gap-2">
                    <Input
                      id="tab-standard-width"
                      type="number"
                      min={80}
                      max={300}
                      step={1}
                      value={tabWidthDraft}
                      aria-describedby="tab-standard-width-range"
                      onChange={(event) => setTabWidthDraft(event.target.value)}
                      onBlur={() => void commitTabStandardWidth()}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                          event.currentTarget.blur()
                        }
                      }}
                    />
                    <span className="text-muted-foreground text-xs">px</span>
                  </div>
                  <p id="tab-standard-width-range" className="text-muted-foreground text-xs">
                    {t("settings.tabStandardWidthRange", { defaultValue: "80-300px" })}
                  </p>
                </div>
              )}
            </div>
          </SettingsRow>
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
