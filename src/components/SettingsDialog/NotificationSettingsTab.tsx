import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Bell, BellRing, CircleAlert, MessageSquareText, Send, Timer, Volume2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { type BellStyle, NOTIFY_COMMAND_MIN_SECS_RANGE, useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useSyncedState } from "@/hooks/useSyncedState"
import { showSystemNotification } from "@/hooks/useTerminalNotifications"
import { isImeKeyEvent } from "@/lib/ime"
import { toErrorMessage } from "@/lib/utils"

/** `notification_permission` (src-tauri/src/notification). */
type NotificationPermission = "granted" | "denied" | "notDetermined" | "fallback"

export const NotificationSettingsTab: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [permission, setPermission] = useState<NotificationPermission | null>(null)
  const [minSecsDraft, setMinSecsDraft] = useSyncedState(String(config.notify_command_min_secs))

  const refreshPermission = useCallback(() => {
    invoke<NotificationPermission>("notification_permission")
      .then(setPermission)
      .catch((error: unknown) => console.error("Failed to read notification permission:", error))
  }, [])

  useEffect(() => {
    refreshPermission()
    // Changed in System Settings while this page is open.
    window.addEventListener("focus", refreshPermission)
    return () => window.removeEventListener("focus", refreshPermission)
  }, [refreshPermission])

  const sendTestNotification = async () => {
    try {
      await showSystemNotification({
        title: t("notifications.testTitle"),
        body: t("notifications.testBody"),
        sound: config.notify_sound,
      })
      // macOS asks for permission on the first one.
      window.setTimeout(refreshPermission, 1500)
    } catch (error) {
      toast({
        title: t("notifications.testFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const commitMinSecs = () => {
    const value = Number.parseInt(minSecsDraft, 10)
    if (
      Number.isNaN(value) ||
      value < NOTIFY_COMMAND_MIN_SECS_RANGE.min ||
      value > NOTIFY_COMMAND_MIN_SECS_RANGE.max
    ) {
      setMinSecsDraft(String(config.notify_command_min_secs))
      return
    }
    if (value !== config.notify_command_min_secs) {
      void saveSettings({ notify_command_min_secs: value })
    }
  }

  const announcementsOff = !config.notifications_enabled

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Bell size={16} />}
          title={t("notifications.title")}
          description={t("notifications.description")}
        >
          <SettingsRow
            icon={<BellRing size={16} />}
            title={t("notifications.enabled")}
            description={t("notifications.enabledDesc")}
            action={
              <Switch
                checked={config.notifications_enabled}
                onCheckedChange={(checked) => void saveSettings({ notifications_enabled: checked })}
                aria-label={t("notifications.enabled")}
              />
            }
          />
          <SettingsRow
            icon={<Volume2 size={16} />}
            title={t("notifications.sound")}
            description={t("notifications.soundDesc")}
            action={
              <Switch
                checked={config.notify_sound}
                disabled={announcementsOff}
                onCheckedChange={(checked) => void saveSettings({ notify_sound: checked })}
                aria-label={t("notifications.sound")}
              />
            }
          />
          <SettingsRow
            icon={<Send size={16} />}
            title={t("notifications.test")}
            description={
              permission === "fallback"
                ? t("notifications.testDescFallback")
                : t("notifications.testDesc")
            }
            action={
              <Button type="button" variant="outline" onClick={() => void sendTestNotification()}>
                {t("notifications.testButton")}
              </Button>
            }
          />
          {permission === "denied" && (
            <Alert className="border-amber-500/40 bg-amber-500/5">
              <CircleAlert className="absolute top-3.5 left-4 size-4 text-amber-500" />
              <div className="pl-6">
                <AlertTitle>{t("notifications.deniedTitle")}</AlertTitle>
                <AlertDescription>{t("notifications.deniedDesc")}</AlertDescription>
              </div>
            </Alert>
          )}
        </SettingsSection>

        <SettingsSection
          icon={<MessageSquareText size={16} />}
          title={t("notifications.events")}
          description={t("notifications.eventsDesc")}
        >
          <SettingsRow
            icon={<Timer size={16} />}
            title={t("notifications.commandFinishedSetting")}
            description={t("notifications.commandFinishedSettingDesc")}
            action={
              <Switch
                checked={config.notify_command_finished}
                onCheckedChange={(checked) =>
                  void saveSettings({ notify_command_finished: checked })
                }
                aria-label={t("notifications.commandFinishedSetting")}
              />
            }
          >
            <div className="flex max-w-xs items-end gap-3">
              <div className="min-w-0 flex-1 space-y-1.5">
                <Label htmlFor="notify-command-min-secs" className="text-muted-foreground text-xs">
                  {t("notifications.commandMinSecs")}
                </Label>
                <Input
                  id="notify-command-min-secs"
                  type="number"
                  min={NOTIFY_COMMAND_MIN_SECS_RANGE.min}
                  max={NOTIFY_COMMAND_MIN_SECS_RANGE.max}
                  step={1}
                  value={minSecsDraft}
                  disabled={!config.notify_command_finished}
                  onChange={(event) => setMinSecsDraft(event.target.value)}
                  onBlur={commitMinSecs}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                      event.currentTarget.blur()
                    }
                  }}
                  className="h-8"
                />
              </div>
              <div className="text-muted-foreground pb-2 text-xs">
                {t("notifications.commandMinSecsRange")}
              </div>
            </div>
          </SettingsRow>
          <SettingsRow
            icon={<MessageSquareText size={16} />}
            title={t("notifications.terminalRequests")}
            description={t("notifications.terminalRequestsDesc")}
            action={
              <Switch
                checked={config.notify_terminal_requests}
                onCheckedChange={(checked) =>
                  void saveSettings({ notify_terminal_requests: checked })
                }
                aria-label={t("notifications.terminalRequests")}
              />
            }
          />
        </SettingsSection>

        <SettingsSection icon={<BellRing size={16} />} title={t("notifications.bellSection")}>
          <SettingsRow
            icon={<Bell size={16} />}
            title={t("notifications.bellStyle")}
            description={t("notifications.bellStyleDesc")}
            action={
              <Select
                aria-label={t("notifications.bellStyle")}
                value={config.bell_style}
                onChange={(event) =>
                  void saveSettings({ bell_style: event.target.value as BellStyle })
                }
                className="w-36"
              >
                <option value="visual">{t("notifications.bellStyles.visual")}</option>
                <option value="sound">{t("notifications.bellStyles.sound")}</option>
                <option value="none">{t("notifications.bellStyles.none")}</option>
              </Select>
            }
          />
          <SettingsRow
            icon={<BellRing size={16} />}
            title={t("notifications.bellNotify")}
            description={t("notifications.bellNotifyDesc")}
            action={
              <Switch
                checked={config.bell_notify}
                disabled={config.bell_style === "none"}
                onCheckedChange={(checked) => void saveSettings({ bell_notify: checked })}
                aria-label={t("notifications.bellNotify")}
              />
            }
          />
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
