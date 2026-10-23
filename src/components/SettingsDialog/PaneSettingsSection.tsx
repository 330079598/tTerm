import { PanelsTopLeft } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { INACTIVE_PANE_DIM_OPTIONS, type PaneSettings } from "@/lib/tabSettings"

/**
 * Appearance settings for split tabs: their pane headers, and fading the
 * panes without the keyboard.
 */
export function PaneSettingsSection() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const panes = config.panes
  const save = (updates: Partial<PaneSettings>) =>
    void saveSettings({ panes: { ...panes, ...updates } })
  // A value set by hand in the config file still shows as itself.
  const dimOptions: number[] = INACTIVE_PANE_DIM_OPTIONS.includes(
    panes.dim_inactive_percent as (typeof INACTIVE_PANE_DIM_OPTIONS)[number]
  )
    ? [...INACTIVE_PANE_DIM_OPTIONS]
    : [...INACTIVE_PANE_DIM_OPTIONS, panes.dim_inactive_percent].sort((a, b) => a - b)

  return (
    <SettingsSection
      icon={<PanelsTopLeft size={16} />}
      title={t("settings.panes")}
      description={t("settings.panesDesc")}
    >
      <SettingsRow
        title={t("settings.paneHeaders")}
        description={t("settings.paneHeadersDesc")}
        action={
          <Switch
            checked={panes.show_headers}
            aria-label={t("settings.paneHeaders")}
            onCheckedChange={(checked) => save({ show_headers: checked })}
          />
        }
      />
      <SettingsRow
        title={t("settings.dimInactivePanes")}
        description={t("settings.dimInactivePanesDesc")}
        action={
          <Select
            className="w-28"
            aria-label={t("settings.dimInactivePanes")}
            value={String(panes.dim_inactive_percent)}
            onChange={(event) => save({ dim_inactive_percent: Number(event.target.value) })}
          >
            {dimOptions.map((percent) => (
              <option key={percent} value={percent}>
                {percent === 0 ? t("settings.dimInactivePanesOff") : `${percent}%`}
              </option>
            ))}
          </Select>
        }
      />
    </SettingsSection>
  )
}
