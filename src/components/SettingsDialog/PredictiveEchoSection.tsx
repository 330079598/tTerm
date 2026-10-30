import { Gauge, Keyboard } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useSyncedState } from "@/hooks/useSyncedState"
import {
  PREDICTIVE_ECHO_THRESHOLD_MAX_MS,
  PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
  type AppConfig,
} from "@/lib/appConfig"
import { isImeKeyEvent } from "@/lib/ime"

const inRange = (value: number) =>
  value >= PREDICTIVE_ECHO_THRESHOLD_MIN_MS && value <= PREDICTIVE_ECHO_THRESHOLD_MAX_MS

/** Chooses when typing at a remote shell shows before its echo comes back. */
export function PredictiveEchoSection() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const threshold = config.predictive_echo_threshold_ms
  const [thresholdDraft, setThresholdDraft] = useSyncedState(String(threshold))

  const saveThreshold = (value: number) => {
    if (value !== threshold) void saveSettings({ predictive_echo_threshold_ms: value })
  }

  // A value out of range or not a number goes back to the nearest one allowed.
  const commitThreshold = () => {
    const value = parseInt(thresholdDraft, 10)
    const next = Number.isNaN(value)
      ? threshold
      : Math.min(
          Math.max(value, PREDICTIVE_ECHO_THRESHOLD_MIN_MS),
          PREDICTIVE_ECHO_THRESHOLD_MAX_MS
        )
    setThresholdDraft(String(next))
    saveThreshold(next)
  }

  return (
    <SettingsSection
      icon={<Keyboard size={16} />}
      title={t("settings.typing")}
      description={t("settings.typingDesc")}
    >
      <SettingsRow
        icon={<Keyboard size={16} />}
        title={t("settings.predictiveEcho")}
        description={t("settings.predictiveEchoDesc")}
        action={
          <Select
            className="w-32"
            aria-label={t("settings.predictiveEcho")}
            value={config.predictive_echo}
            onChange={(event) =>
              void saveSettings({
                predictive_echo: event.target.value as AppConfig["predictive_echo"],
              })
            }
          >
            <option value="auto">{t("settings.predictiveEchoAuto")}</option>
            <option value="always">{t("settings.predictiveEchoAlways")}</option>
            <option value="off">{t("settings.predictiveEchoOff")}</option>
          </Select>
        }
      />
      <SettingsRow
        icon={<Gauge size={16} />}
        title={t("settings.predictiveEchoThreshold")}
        description={t("settings.predictiveEchoThresholdDesc")}
      >
        <div className="flex max-w-xs items-end gap-3">
          <div className="min-w-0 flex-1 space-y-1.5">
            <Label htmlFor="predictive-echo-threshold" className="text-muted-foreground text-xs">
              {t("settings.predictiveEchoThresholdMs")}
            </Label>
            <Input
              id="predictive-echo-threshold"
              type="number"
              min={PREDICTIVE_ECHO_THRESHOLD_MIN_MS}
              max={PREDICTIVE_ECHO_THRESHOLD_MAX_MS}
              step={5}
              disabled={config.predictive_echo !== "auto"}
              value={thresholdDraft}
              onChange={(event) => {
                setThresholdDraft(event.target.value)
                const value = parseInt(event.target.value, 10)
                if (!Number.isNaN(value) && inRange(value)) saveThreshold(value)
              }}
              onBlur={commitThreshold}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                  event.currentTarget.blur()
                }
              }}
              className="h-8"
            />
          </div>
          <div className="text-muted-foreground pb-2 text-xs">
            {t("settings.predictiveEchoThresholdRange", {
              min: PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
              max: PREDICTIVE_ECHO_THRESHOLD_MAX_MS,
            })}
          </div>
        </div>
      </SettingsRow>
    </SettingsSection>
  )
}
