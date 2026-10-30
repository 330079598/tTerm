import React from "react"
import { ClipboardPaste, FolderOpen, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { BandwidthLimitInput } from "@/components/SettingsDialog/BandwidthLimitInput"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import type { AppConfig } from "@/lib/appConfig"
import { pickEditorProgram } from "@/lib/externalEditor"
import { normalizeSftpStartDirectory, SFTP_START_DIRECTORIES } from "@/lib/sftpStartDirectory"

/** The SFTP drawer's settings, in the General tab. */
export const SftpSettingsSection: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const sftpStartDirectory = config.sftp_start_directory
  const sftpPasteUploadEnabled = config.sftp_paste_upload_enabled
  const sftpTransferParallelism = config.sftp_transfer_parallelism
  const sftpUploadLimitKib = config.sftp_upload_limit_kib
  const sftpDownloadLimitKib = config.sftp_download_limit_kib

  const handleSftpStartDirectoryChange = (choice: AppConfig["sftp_start_directory"]) =>
    saveSettings({ sftp_start_directory: choice })
  const handleSftpPasteUploadEnabledChange = (checked: boolean) =>
    saveSettings({ sftp_paste_upload_enabled: checked })
  const handleSftpTransferParallelismChange = (value: number) => {
    if (!Number.isFinite(value)) return
    void saveSettings({ sftp_transfer_parallelism: Math.min(Math.max(Math.round(value), 1), 16) })
  }
  const handleSftpUploadLimitChange = async (kib: number) => {
    await saveSettings({ sftp_upload_limit_kib: kib })
  }
  const handleSftpDownloadLimitChange = async (kib: number) => {
    await saveSettings({ sftp_download_limit_kib: kib })
  }
  const chooseExternalEditor = async () => {
    const program = await pickEditorProgram(t("settings.sftpExternalEditorChoose"))
    if (program) await saveSettings({ sftp_external_editor: program })
  }

  return (
    <SettingsSection
      icon={<ClipboardPaste size={16} />}
      title={t("settings.sftp", { defaultValue: "SFTP" })}
      description={t("settings.sftpDesc", {
        defaultValue: "Configure file manager behavior for remote directories.",
      })}
    >
      <SettingsRow
        title={t("settings.sftpStartDirectory")}
        description={t("settings.sftpStartDirectoryDesc")}
        action={
          <Select
            aria-label={t("settings.sftpStartDirectory")}
            value={sftpStartDirectory}
            onChange={(event) => {
              const choice = normalizeSftpStartDirectory(event.target.value)
              if (choice) void handleSftpStartDirectoryChange(choice)
            }}
            className="w-44"
          >
            {SFTP_START_DIRECTORIES.map((choice) => (
              <option key={choice} value={choice}>
                {t(`sftp.startDirectory.${choice}`)}
              </option>
            ))}
          </Select>
        }
      />
      <SettingsRow
        title={t("settings.sftpPasteUpload")}
        description={t("settings.sftpPasteUploadDesc")}
        action={
          <Switch
            checked={sftpPasteUploadEnabled}
            onCheckedChange={handleSftpPasteUploadEnabledChange}
          />
        }
      />
      <SettingsRow
        title={t("settings.sftpTransferParallelism", {
          defaultValue: "Parallel transfer channels",
        })}
        description={t("settings.sftpTransferParallelismDesc", {
          defaultValue:
            "Number of SFTP channels used per resumable transfer (1-16, default 4). Set to 1 for single-stream sequential transfers, which is the compatible choice for servers whose storage stalls under concurrent writes.",
        })}
        action={
          <Input
            type="number"
            min={1}
            max={16}
            value={sftpTransferParallelism}
            onChange={(event) => handleSftpTransferParallelismChange(Number(event.target.value))}
            className="w-20"
          />
        }
      />
      <SettingsRow
        title={t("settings.sftpUploadLimit", { defaultValue: "Upload speed limit" })}
        description={t("settings.sftpUploadLimitDesc", {
          defaultValue:
            "Caps the combined speed of all SFTP uploads, so large transfers leave room for interactive sessions. 0 means unlimited. Changes apply to running transfers immediately.",
        })}
        action={
          <BandwidthLimitInput
            aria-label={t("settings.sftpUploadLimit", { defaultValue: "Upload speed limit" })}
            value={sftpUploadLimitKib}
            onChange={handleSftpUploadLimitChange}
          />
        }
      />
      <SettingsRow
        title={t("settings.sftpDownloadLimit", { defaultValue: "Download speed limit" })}
        description={t("settings.sftpDownloadLimitDesc", {
          defaultValue:
            "Caps the combined speed of all SFTP downloads. 0 means unlimited. Changes apply to running transfers immediately.",
        })}
        action={
          <BandwidthLimitInput
            aria-label={t("settings.sftpDownloadLimit", {
              defaultValue: "Download speed limit",
            })}
            value={sftpDownloadLimitKib}
            onChange={handleSftpDownloadLimitChange}
          />
        }
      />
      <SettingsRow
        title={t("settings.sftpExternalEditor")}
        description={t("settings.sftpExternalEditorDesc")}
      >
        <div className="flex min-w-0 gap-2">
          <Input
            value={config.sftp_external_editor}
            readOnly
            placeholder={t("settings.sftpExternalEditorPlaceholder")}
            aria-label={t("settings.sftpExternalEditor")}
            className="min-w-0 flex-1 font-mono text-xs"
          />
          <Button type="button" variant="outline" onClick={() => void chooseExternalEditor()}>
            <FolderOpen />
            {t("terminalLogging.choose")}
          </Button>
          {config.sftp_external_editor && (
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={t("settings.sftpExternalEditorClear")}
              title={t("settings.sftpExternalEditorClear")}
              onClick={() => void saveSettings({ sftp_external_editor: "" })}
            >
              <X />
            </Button>
          )}
        </div>
      </SettingsRow>
      <SettingsRow
        title={t("settings.sftpExternalUpload")}
        description={t("settings.sftpExternalUploadDesc")}
        action={
          <Select
            aria-label={t("settings.sftpExternalUpload")}
            value={config.sftp_external_upload}
            onChange={(event) =>
              void saveSettings({
                sftp_external_upload: event.target.value === "auto" ? "auto" : "ask",
              })
            }
            className="w-44"
          >
            <option value="ask">{t("settings.sftpExternalUploadAsk")}</option>
            <option value="auto">{t("settings.sftpExternalUploadAuto")}</option>
          </Select>
        }
      />
    </SettingsSection>
  )
}
