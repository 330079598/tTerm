import React from "react"
import { Check, ClipboardPaste, Info, Languages, PlugZap, Trash2, Wrench } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
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
  i18nLanguage: string
  languages: LanguageOption[]
  restoreAllSessionConnections: boolean
  sftpPasteUploadEnabled: boolean
}

export const GeneralSettingsTab: React.FC<GeneralSettingsTabProps> = ({
  handleAbout,
  handleClearSession,
  handleLanguageChange,
  handleRestoreAllSessionConnectionsChange,
  handleSftpPasteUploadEnabledChange,
  i18nLanguage,
  languages,
  restoreAllSessionConnections,
  sftpPasteUploadEnabled,
}) => {
  const { t } = useTranslation()

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
          <div className="grid gap-2">
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
                    "h-auto w-full justify-between rounded-lg border px-4 py-3 text-left",
                    isActive
                      ? "border-primary bg-accent ring-primary ring-1 ring-inset"
                      : "border-transparent"
                  )}
                >
                  <div>
                    <div className="text-sm font-semibold">{lang.nativeLabel}</div>
                    <div className="text-muted-foreground text-xs">{lang.label}</div>
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
            icon={<ClipboardPaste size={16} />}
            title={t("settings.sftpPasteUpload")}
            description={t("settings.sftpPasteUploadDesc")}
            action={
              <Switch
                checked={sftpPasteUploadEnabled}
                onCheckedChange={handleSftpPasteUploadEnabledChange}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<PlugZap size={16} />}
          title={t("settings.startup", { defaultValue: "Startup" })}
        >
          <SettingsRow
            icon={<PlugZap size={16} />}
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
          icon={<Wrench size={16} />}
          title={t("settings.maintenance", { defaultValue: "Maintenance" })}
        >
          <SettingsRow
            icon={<Trash2 size={16} className="text-destructive" />}
            title={t("settings.clearSession")}
            description={t("settings.clearSessionDesc")}
            action={
              <Button type="button" variant="outline" onClick={handleClearSession}>
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
