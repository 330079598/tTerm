import React from "react"
import {
  ArrowLeftRight,
  Bug,
  Check,
  ClipboardPaste,
  FolderOpen,
  Info,
  Languages,
  PlugZap,
  Trash2,
  Wrench,
} from "lucide-react"
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { cn } from "@/lib/utils"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"

interface LanguageOption {
  code: string
  label: string
  nativeLabel: string
}

interface GeneralSettingsTabProps {
  handleAbout: () => void
  handleClearSession: () => Promise<void>
  handleLanguageChange: (langCode: string) => Promise<void>
  handleRestoreAllSessionConnectionsChange: (checked: boolean) => Promise<void>
  handleSftpPasteUploadEnabledChange: (checked: boolean) => Promise<void>
  handleSftpTransferParallelismChange: (value: number) => Promise<void>
  handleEnableDevtoolsChange: () => Promise<void>
  handleZmodemAutoDetectEnabledChange: (checked: boolean) => Promise<void>
  handleZmodemDownloadDirectoryChange: (directory: string) => Promise<void>
  i18nLanguage: string
  languages: LanguageOption[]
  restoreAllSessionConnections: boolean
  sftpPasteUploadEnabled: boolean
  sftpTransferParallelism: number
  zmodemAutoDetectEnabled: boolean
  zmodemDownloadDirectory: string
}

export const GeneralSettingsTab: React.FC<GeneralSettingsTabProps> = ({
  handleAbout,
  handleClearSession,
  handleLanguageChange,
  handleRestoreAllSessionConnectionsChange,
  handleSftpPasteUploadEnabledChange,
  handleSftpTransferParallelismChange,
  handleEnableDevtoolsChange,
  handleZmodemAutoDetectEnabledChange,
  handleZmodemDownloadDirectoryChange,
  i18nLanguage,
  languages,
  restoreAllSessionConnections,
  sftpPasteUploadEnabled,
  sftpTransferParallelism,
  zmodemAutoDetectEnabled,
  zmodemDownloadDirectory,
}) => {
  const { t } = useTranslation()

  const chooseZmodemDownloadDirectory = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      defaultPath: zmodemDownloadDirectory || undefined,
    }).catch(() => null)
    if (typeof selected !== "string") return
    await handleZmodemDownloadDirectoryChange(selected)
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Languages size={16} />}
          title={t("language.title")}
          description={t("language.description", {
            defaultValue: "Set the display language for tTerm.",
          })}
        >
          <div className="grid max-w-lg grid-cols-2 gap-2">
            {languages.map((lang) => {
              const isActive = i18nLanguage === lang.code
              return (
                <Button
                  key={lang.code}
                  type="button"
                  variant="ghost"
                  aria-pressed={isActive}
                  onClick={() => handleLanguageChange(lang.code)}
                  className={cn(
                    "h-10 w-full justify-start rounded-md border px-3 text-left",
                    isActive
                      ? "border-primary bg-accent ring-primary ring-1 ring-inset"
                      : "border-transparent"
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{lang.nativeLabel}</div>
                    {lang.nativeLabel !== lang.label && (
                      <div className="text-muted-foreground truncate text-[11px]">{lang.label}</div>
                    )}
                  </div>
                  {isActive && <Check size={16} className="text-primary ml-3 shrink-0" />}
                </Button>
              )
            })}
          </div>
        </SettingsSection>

        <SettingsSection
          icon={<ClipboardPaste size={16} />}
          title={t("settings.sftp", { defaultValue: "SFTP" })}
          description={t("settings.sftpDesc", {
            defaultValue: "Configure file manager behavior for remote directories.",
          })}
        >
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
                onChange={(event) =>
                  handleSftpTransferParallelismChange(Number(event.target.value))
                }
                className="w-20"
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<ArrowLeftRight size={16} />}
          title={t("settings.zmodem", { defaultValue: "ZMODEM" })}
          description={t("settings.zmodemDesc", {
            defaultValue:
              "Detect rz/sz file transfers inside an active shell session, the way SecureCRT/Xshell do.",
          })}
        >
          <SettingsRow
            title={t("settings.zmodemAutoDetect", { defaultValue: "Auto-detect ZMODEM transfers" })}
            description={t("settings.zmodemAutoDetectDesc", {
              defaultValue:
                "Watch terminal output for rz/sz and start the transfer automatically. Turn off if you never use ZMODEM and prefer not to scan output at all.",
            })}
            action={
              <Switch
                checked={zmodemAutoDetectEnabled}
                onCheckedChange={handleZmodemAutoDetectEnabledChange}
              />
            }
          />
          <SettingsRow
            title={t("settings.zmodemDownloadDirectory", { defaultValue: "Download directory" })}
            description={t("settings.zmodemDownloadDirectoryDesc", {
              defaultValue: "Where auto-detected ZMODEM downloads are saved. Defaults to Downloads.",
            })}
          >
            <div className="flex min-w-0 gap-2">
              <Input
                value={zmodemDownloadDirectory}
                readOnly
                placeholder={t("settings.zmodemDownloadDirectoryPlaceholder", {
                  defaultValue: "Platform Downloads folder",
                })}
                aria-label={t("settings.zmodemDownloadDirectory", { defaultValue: "Download directory" })}
                className="min-w-0 flex-1 font-mono text-xs"
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => void chooseZmodemDownloadDirectory()}
              >
                <FolderOpen />
                {t("terminalLogging.choose")}
              </Button>
            </div>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection
          icon={<PlugZap size={16} />}
          title={t("settings.startup", { defaultValue: "Startup" })}
        >
          <SettingsRow
            title={t("settings.restoreAllSessionConnections")}
            description={t("settings.restoreAllSessionConnectionsDesc")}
            action={
              <Switch
                checked={restoreAllSessionConnections}
                onCheckedChange={handleRestoreAllSessionConnectionsChange}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Bug size={16} />}
          title={t("settings.advanced", { defaultValue: "Advanced" })}
          description={t("settings.advancedDesc", {
            defaultValue: "Tools intended for diagnostics and development.",
          })}
        >
          <SettingsRow
            icon={<Bug size={16} />}
            title={t("settings.devtools")}
            description={t("settings.devtoolsDesc")}
            action={
              <Button type="button" variant="outline" onClick={handleEnableDevtoolsChange}>
                {t("settings.openDevtools", { defaultValue: "Open" })}
              </Button>
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Wrench size={16} />}
          title={t("settings.maintenance", { defaultValue: "Maintenance" })}
        >
          <SettingsRow
            icon={<Trash2 size={16} className="text-destructive" />}
            title={t("settings.clearSession")}
            description={t("settings.clearSessionDesc")}
            action={
              <Button type="button" variant="destructive" onClick={handleClearSession}>
                {t("settings.clearSession")}
              </Button>
            }
          />
          <SettingsRow
            icon={<Info size={16} />}
            title={t("settings.about")}
            description={t("app.subtitle")}
            action={
              <Button type="button" variant="outline" onClick={handleAbout}>
                {t("common.open", { defaultValue: "Open" })}
              </Button>
            }
          />
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
