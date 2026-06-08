import React from "react"
import { Activity, Route } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"

interface ConnectionSettingsTabProps {
  handleShowJumpHostConnectionInfoChange: (checked: boolean) => Promise<void>
  handleMonitorRefreshIntervalChange: (seconds: number) => Promise<void>
  monitorRefreshIntervalSecs: number
  showJumpHostConnectionInfo: boolean
}

export const ConnectionSettingsTab: React.FC<ConnectionSettingsTabProps> = ({
  handleShowJumpHostConnectionInfoChange,
  handleMonitorRefreshIntervalChange,
  monitorRefreshIntervalSecs,
  showJumpHostConnectionInfo,
}) => {
  const { t } = useTranslation()
  const [monitorRefreshDraft, setMonitorRefreshDraft] = React.useState(
    String(monitorRefreshIntervalSecs)
  )

  React.useEffect(() => {
    setMonitorRefreshDraft(String(monitorRefreshIntervalSecs))
  }, [monitorRefreshIntervalSecs])

  const commitMonitorRefreshInterval = React.useCallback(() => {
    const value = parseInt(monitorRefreshDraft, 10)
    if (Number.isNaN(value)) {
      setMonitorRefreshDraft(String(monitorRefreshIntervalSecs))
      return
    }

    const normalizedValue = Math.min(Math.max(value, 1), 60)
    setMonitorRefreshDraft(String(normalizedValue))
    if (normalizedValue !== monitorRefreshIntervalSecs) {
      void handleMonitorRefreshIntervalChange(normalizedValue)
    }
  }, [handleMonitorRefreshIntervalChange, monitorRefreshDraft, monitorRefreshIntervalSecs])

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Route size={16} />}
          title={t("settings.jumpHost", { defaultValue: "Jump hosts" })}
          description={t("settings.connectionDesc", {
            defaultValue: "Control connection-related prompts and route details.",
          })}
        >
          <SettingsRow
            icon={<Route size={16} />}
            title={t("settings.showJumpHostConnectionInfo")}
            description={t("settings.showJumpHostConnectionInfoDesc")}
            action={
              <Switch
                checked={showJumpHostConnectionInfo}
                onCheckedChange={handleShowJumpHostConnectionInfoChange}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Activity size={16} />}
          title={t("settings.monitor", { defaultValue: "Monitor" })}
          description={t("settings.monitorDesc", {
            defaultValue: "Tune how often SSH server metrics refresh while the monitor is visible.",
          })}
        >
          <SettingsRow
            icon={<Activity size={16} />}
            title={t("settings.monitorRefreshInterval", {
              defaultValue: "Server monitor refresh interval",
            })}
            description={t("settings.monitorRefreshIntervalDesc", {
              defaultValue:
                "Choose a compact cadence for CPU, memory, IP and disk metrics. Shorter intervals feel more live but run more SSH queries.",
            })}
          >
            <div className="flex max-w-xs items-end gap-3">
              <div className="min-w-0 flex-1 space-y-1.5">
                <Label htmlFor="monitor-refresh-interval" className="text-muted-foreground text-xs">
                  {t("settings.monitorRefreshIntervalSeconds", { defaultValue: "Seconds" })}
                </Label>
                <Input
                  id="monitor-refresh-interval"
                  type="number"
                  min={1}
                  max={60}
                  step={1}
                  value={monitorRefreshDraft}
                  onChange={(event) => {
                    const nextValue = event.target.value
                    setMonitorRefreshDraft(nextValue)

                    const value = parseInt(nextValue, 10)
                    if (!Number.isNaN(value) && value >= 1 && value <= 60) {
                      void handleMonitorRefreshIntervalChange(value)
                    }
                  }}
                  onBlur={commitMonitorRefreshInterval}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.currentTarget.blur()
                    }
                  }}
                  className="h-8"
                />
              </div>
              <div className="text-muted-foreground pb-2 text-xs">
                {t("settings.monitorRefreshIntervalRange", { defaultValue: "1-60s" })}
              </div>
            </div>
          </SettingsRow>
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
