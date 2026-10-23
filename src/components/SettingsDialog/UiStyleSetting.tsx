import { useTranslation } from "react-i18next"

import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import type { UiStyle } from "@/lib/appConfig"

/** Chooses how the window lays out its surfaces: classic, or as islands. */
export function UiStyleSetting() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()

  return (
    <SettingsRow
      title={t("settings.uiStyle")}
      description={t("settings.uiStyleDesc")}
      action={
        <Select
          className="w-32"
          aria-label={t("settings.uiStyle")}
          value={config.ui_style}
          onChange={(event) => void saveSettings({ ui_style: event.target.value as UiStyle })}
        >
          <option value="classic">{t("settings.uiStyleClassic")}</option>
          <option value="islands">{t("settings.uiStyleIslands")}</option>
        </Select>
      }
    />
  )
}
