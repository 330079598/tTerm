import React from "react"
import {
  ArrowLeftRight,
  Bug,
  Check,
  FolderOpen,
  Info,
  Languages,
  Moon,
  PlugZap,
  RotateCw,
  Trash2,
  Wrench,
} from "lucide-react"
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useAppActivity } from "@/contexts/AppActivityContext"
import type { BackgroundThrottling, CloseBehavior } from "@/lib/appConfig"
import { getDetectedPlatform } from "@/lib/uiPlatform"
import { getLaunchBackgroundThrottling } from "@/contexts/ConfigContext"
import { cn } from "@/lib/utils"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { SftpSettingsSection } from "@/components/SettingsDialog/SftpSettingsSection"
import { SystemIntegrationSection } from "@/components/SettingsDialog/SystemIntegrationSection"

interface LanguageOption {
  code: string
  label: string
  nativeLabel: string
}

interface GeneralSettingsTabProps {
  backgroundThrottling: BackgroundThrottling
  closeBehavior: CloseBehavior
  handleAbout: () => void
  handleBackgroundThrottlingChange: (policy: BackgroundThrottling) => Promise<void>
  handleCloseBehaviorChange: (behavior: CloseBehavior) => Promise<void>
  handleClearSession: () => Promise<void>
  handleLanguageChange: (langCode: string) => Promise<void>
  handleRestoreAllSessionConnectionsChange: (checked: boolean) => Promise<void>
  handleRestoreTerminalContentChange: (checked: boolean) => Promise<void>
  handleEnableDevtoolsChange: () => Promise<void>
  handleZmodemAutoDetectEnabledChange: (checked: boolean) => Promise<void>
  handleZmodemDownloadDirectoryChange: (directory: string) => Promise<void>
  i18nLanguage: string
  languages: LanguageOption[]
  restoreAllSessionConnections: boolean
  restoreTerminalContent: boolean
  zmodemAutoDetectEnabled: boolean
  zmodemDownloadDirectory: string
}

export const GeneralSettingsTab: React.FC<GeneralSettingsTabProps> = ({
  backgroundThrottling,
  closeBehavior,
  handleAbout,
  handleBackgroundThrottlingChange,
  handleCloseBehaviorChange,
  handleClearSession,
  handleLanguageChange,
  handleRestoreAllSessionConnectionsChange,
  handleRestoreTerminalContentChange,
  handleEnableDevtoolsChange,
  handleZmodemAutoDetectEnabledChange,
  handleZmodemDownloadDirectoryChange,
  i18nLanguage,
  languages,
  restoreAllSessionConnections,
  restoreTerminalContent,
  zmodemAutoDetectEnabled,
  zmodemDownloadDirectory,
}) => {
  const { t } = useTranslation()
  const { requestAppRestart } = useAppActivity()
  const detectedPlatform = getDetectedPlatform()
  const launchBackgroundThrottling = getLaunchBackgroundThrottling()
  const backgroundThrottlingNeedsRestart =
    launchBackgroundThrottling !== null && launchBackgroundThrottling !== backgroundThrottling

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
                    "h-10 w-full justify-start rounded-md px-3 text-left",
                    isActive
                      ? "bg-primary/10 hover:bg-primary/15 dark:hover:bg-primary/15"
                      : "hover:bg-muted dark:hover:bg-muted"
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

        <SftpSettingsSection />

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
              defaultValue:
                "Where auto-detected ZMODEM downloads are saved. Defaults to Downloads.",
            })}
          >
            <div className="flex min-w-0 gap-2">
              <Input
                value={zmodemDownloadDirectory}
                readOnly
                placeholder={t("settings.zmodemDownloadDirectoryPlaceholder", {
                  defaultValue: "Platform Downloads folder",
                })}
                aria-label={t("settings.zmodemDownloadDirectory", {
                  defaultValue: "Download directory",
                })}
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
          <SettingsRow
            title={t("settings.restoreTerminalContent")}
            description={t("settings.restoreTerminalContentDesc")}
            action={
              <Switch
                checked={restoreTerminalContent}
                onCheckedChange={handleRestoreTerminalContentChange}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Moon size={16} />}
          title={t("settings.background", { defaultValue: "Background" })}
        >
          <SettingsRow
            title={t("settings.closeBehavior", { defaultValue: "When closing the main window" })}
            description={
              t("settings.closeBehaviorDesc", {
                defaultValue:
                  "In the background, SSH sessions and port forwarding keep running. Bring the window back from the tray icon (the menu bar on macOS) or by launching tTerm again.",
              }) +
              (detectedPlatform === "linux"
                ? " " +
                  t("settings.closeBehaviorLinuxNote", {
                    defaultValue:
                      "Some Linux desktops, such as GNOME without an AppIndicator extension, do not show tray icons.",
                  })
                : "")
            }
            action={
              <Select
                aria-label={t("settings.closeBehavior", {
                  defaultValue: "When closing the main window",
                })}
                value={closeBehavior}
                onChange={(event) =>
                  void handleCloseBehaviorChange(event.target.value as CloseBehavior)
                }
                className="w-44"
              >
                <option value="ask">
                  {t("settings.closeBehaviorAsk", { defaultValue: "Ask every time" })}
                </option>
                <option value="tray">
                  {t("settings.closeBehaviorTray", { defaultValue: "Run in background" })}
                </option>
                <option value="quit">
                  {t("settings.closeBehaviorQuit", { defaultValue: "Quit tTerm" })}
                </option>
              </Select>
            }
          />
          {detectedPlatform === "macos" && (
            <SettingsRow
              title={t("settings.backgroundThrottling", {
                defaultValue: "While the window is hidden",
              })}
              description={t("settings.backgroundThrottlingDesc", {
                defaultValue:
                  "Throttle keeps terminals processing output at reduced priority. Suspend frees the most power but pauses the page after about 5 minutes, which delays redraw when you come back. Requires macOS 14 or later; applies after restart.",
              })}
              action={
                <Select
                  aria-label={t("settings.backgroundThrottling", {
                    defaultValue: "While the window is hidden",
                  })}
                  value={backgroundThrottling}
                  onChange={(event) =>
                    void handleBackgroundThrottlingChange(
                      event.target.value as BackgroundThrottling
                    )
                  }
                  className="w-44"
                >
                  <option value="throttle">
                    {t("settings.backgroundThrottlingThrottle", {
                      defaultValue: "Throttle (recommended)",
                    })}
                  </option>
                  <option value="disabled">
                    {t("settings.backgroundThrottlingDisabled", {
                      defaultValue: "Keep running",
                    })}
                  </option>
                  <option value="suspend">
                    {t("settings.backgroundThrottlingSuspend", {
                      defaultValue: "Suspend (saves power)",
                    })}
                  </option>
                </Select>
              }
            />
          )}
          {detectedPlatform === "macos" && backgroundThrottlingNeedsRestart && (
            <SettingsRow
              title={t("settings.restartToApply", { defaultValue: "Restart to apply" })}
              action={
                <Button type="button" variant="outline" onClick={() => void requestAppRestart()}>
                  <RotateCw />
                  {t("settings.restartNow", { defaultValue: "Restart now" })}
                </Button>
              }
            />
          )}
        </SettingsSection>

        <SystemIntegrationSection />

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
