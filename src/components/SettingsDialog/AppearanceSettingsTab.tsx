import React from "react"
import {
  ChevronDown,
  ChevronUp,
  Copy,
  Edit,
  GalleryHorizontal,
  MonitorCog,
  Palette,
  Plus,
  RotateCcw,
  Trash2,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { ThemeCard } from "@/components/ThemeCard"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { applyUiScalePercent, type TabWidthMode } from "@/contexts/ConfigContext"
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
  presetThemes: PresetTheme[]
  presetThemeOverrides: CustomTheme[]
  setCreatingFromTheme: React.Dispatch<React.SetStateAction<string | null>>
  setEditingThemeId: React.Dispatch<React.SetStateAction<string | null>>
  tabStandardWidth: number
  tabWidthMode: TabWidthMode
  uiScalePercent: number
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
  presetThemes,
  presetThemeOverrides,
  setCreatingFromTheme,
  setEditingThemeId,
  tabStandardWidth,
  tabWidthMode,
  uiScalePercent,
}) => {
  const { t } = useTranslation()
  const [tabWidthDraft, setTabWidthDraft] = React.useState(String(tabStandardWidth))
  const [uiScaleDraft, setUiScaleDraft] = React.useState(uiScalePercent)
  const [savingUiScale, setSavingUiScale] = React.useState(false)
  const [showAllPresetThemes, setShowAllPresetThemes] = React.useState(false)
  const savingUiScaleRef = React.useRef(false)
  const savedUiScaleRef = React.useRef(uiScalePercent)
  const mountedRef = React.useRef(true)

  React.useEffect(() => {
    setTabWidthDraft(String(tabStandardWidth))
  }, [tabStandardWidth])

  React.useEffect(() => {
    setUiScaleDraft(uiScalePercent)
    savedUiScaleRef.current = uiScalePercent
  }, [uiScalePercent])

  React.useEffect(
    () => () => {
      mountedRef.current = false
      applyUiScalePercent(savedUiScaleRef.current)
    },
    []
  )

  const previewUiScale = React.useCallback((scale: number) => {
    setUiScaleDraft(scale)
    applyUiScalePercent(scale)
  }, [])

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
  }, [handleTabStandardWidthChange, tabStandardWidth, tabWidthDraft])
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

          <Button
            variant="outline"
            className="mt-3 w-full"
            onClick={() => setCreatingFromTheme("default")}
          >
            <Plus size={16} className="mr-2" />
            {t("themeEditor.createNew")}
          </Button>
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
                        if (event.key === "Enter") {
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
