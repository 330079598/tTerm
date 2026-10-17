import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { Play, Volume2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { NOTIFICATION_SOUND_NAMES, type NotificationSoundName } from "@/lib/appConfig"
import {
  type NotificationSoundSettings,
  importNotificationSound,
  notificationSoundSource,
  playNotificationSoundSource,
} from "@/lib/notificationSound"
import { toErrorMessage } from "@/lib/utils"

/** What the webviews decode: Chromium on Windows, WebKit on macOS and Linux,
 *  where Linux also needs the GStreamer plugins for the format. */
const SOUND_FILE_EXTENSIONS = ["mp3", "wav", "ogg", "oga", "opus", "flac", "m4a", "aac"]

interface NotificationSoundRowProps {
  /** Notifications are off, so no sound is made either. */
  disabled: boolean
}

/** Whether notifications make a sound, and which: the system's, a tone or a file. */
export const NotificationSoundRow: React.FC<NotificationSoundRowProps> = ({ disabled }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const choiceDisabled = disabled || !config.notify_sound
  const source = notificationSoundSource(config)

  const preview = async (settings: NotificationSoundSettings) => {
    try {
      await playNotificationSoundSource(notificationSoundSource(settings))
    } catch (error) {
      toast({
        title: t("notifications.soundFileFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const pickFile = async () => {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: t("notifications.soundFiles"), extensions: SOUND_FILE_EXTENSIONS }],
    }).catch(() => null)
    if (typeof selected !== "string") return
    let name: string
    try {
      name = await importNotificationSound(selected)
    } catch (error) {
      toast({
        title: t("notifications.soundFileFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
      return
    }
    void saveSettings({ notify_sound_name: "custom", notify_sound_file: name })
    void preview({ ...config, notify_sound_name: "custom", notify_sound_file: name })
  }

  const choose = (name: NotificationSoundName) => {
    void saveSettings({ notify_sound_name: name })
    if (name === "custom" && !config.notify_sound_file.trim()) void pickFile()
    else void preview({ ...config, notify_sound_name: name })
  }

  return (
    <SettingsRow
      icon={<Volume2 size={16} />}
      title={t("notifications.sound")}
      description={t("notifications.soundDesc")}
      action={
        <Switch
          checked={config.notify_sound}
          disabled={disabled}
          onCheckedChange={(checked) => void saveSettings({ notify_sound: checked })}
          aria-label={t("notifications.sound")}
        />
      }
    >
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Select
            aria-label={t("notifications.soundName")}
            value={config.notify_sound_name}
            disabled={choiceDisabled}
            onChange={(event) => choose(event.target.value as NotificationSoundName)}
            className="w-44"
          >
            {NOTIFICATION_SOUND_NAMES.map((name) => (
              <option key={name} value={name}>
                {t(`notifications.sounds.${name}`)}
              </option>
            ))}
          </Select>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={choiceDisabled || source.kind === "system"}
            onClick={() => void preview(config)}
            aria-label={t("notifications.soundPreview")}
            title={t("notifications.soundPreview")}
          >
            <Play />
          </Button>
        </div>
        {config.notify_sound_name === "custom" && (
          <div className="flex items-center gap-2">
            <span
              className="text-muted-foreground min-w-0 flex-1 truncate text-xs"
              title={config.notify_sound_file}
            >
              {config.notify_sound_file.trim()
                ? t("notifications.soundFileCopied", { name: config.notify_sound_file })
                : t("notifications.soundFileNone")}
            </span>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={choiceDisabled}
              onClick={() => void pickFile()}
            >
              {t("notifications.soundFileBrowse")}
            </Button>
          </div>
        )}
      </div>
    </SettingsRow>
  )
}
