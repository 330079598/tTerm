import React from "react"
import { GalleryHorizontal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { TabIconSettings } from "@/components/SettingsDialog/TabIconSettings"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useSyncedState } from "@/hooks/useSyncedState"
import { isImeKeyEvent } from "@/lib/ime"
import type { TabWidthMode } from "@/lib/tabSettings"

interface TabSettingsSectionProps {
  handleTabStandardWidthChange: (width: number) => Promise<boolean>
  handleTabWidthModeChange: (mode: TabWidthMode) => Promise<void>
  tabStandardWidth: number
  tabWidthMode: TabWidthMode
}

/** Appearance settings for the tabs: how wide they are and the icons local tabs show. */
export function TabSettingsSection({
  handleTabStandardWidthChange,
  handleTabWidthModeChange,
  tabStandardWidth,
  tabWidthMode,
}: TabSettingsSectionProps) {
  const { t } = useTranslation()
  const [tabWidthDraft, setTabWidthDraft] = useSyncedState(String(tabStandardWidth))

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

  return (
    <>
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
      <TabIconSettings />
    </>
  )
}
