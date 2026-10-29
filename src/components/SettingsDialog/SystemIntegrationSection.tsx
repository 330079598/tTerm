import React from "react"
import { FolderOpen, SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useSystemIntegration } from "@/components/SettingsDialog/useSystemIntegration"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { getDetectedPlatform } from "@/lib/uiPlatform"

type Integration = ReturnType<typeof useSystemIntegration>

/**
 * Whether to show an integration's row: where the backend supports it, or,
 * until it has said, where tTerm offers it, so the section does not appear
 * only once the status is in. A status that could not be read shows too.
 */
function isShown({ status, loadError }: Integration, offered: boolean): boolean {
  return loadError !== null || (status?.supported ?? offered)
}

interface IntegrationRowProps {
  icon: React.ReactNode
  title: string
  description: string
  offered: boolean
  integration: Integration
}

const IntegrationRow: React.FC<IntegrationRowProps> = ({
  icon,
  title,
  description,
  offered,
  integration,
}) => {
  const { t } = useTranslation()
  const { status, loadError, busy, change, retry } = integration
  if (!isShown(integration, offered)) return null

  if (loadError !== null) {
    return (
      <SettingsRow
        icon={icon}
        title={title}
        description={
          <>
            {description}
            <span className="text-destructive mt-1 block">
              {t("systemIntegration.loadFailed", { error: loadError })}
            </span>
          </>
        }
        action={
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => void retry()}
          >
            {t("systemIntegration.retry")}
          </Button>
        }
      />
    )
  }

  return (
    <SettingsRow
      icon={icon}
      title={title}
      description={description}
      action={
        <Switch
          checked={status?.installed ?? false}
          disabled={busy || status === null}
          onCheckedChange={(checked) => void change(checked)}
          aria-label={title}
        />
      }
    />
  )
}

/** Ties tTerm into the system; hidden where it has nothing to offer yet. */
export const SystemIntegrationSection: React.FC = () => {
  const { t } = useTranslation()
  const platform = getDetectedPlatform()
  const macos = platform === "macos"
  const offered = macos || platform === "windows"
  const shellCommand = useSystemIntegration({
    status: "shell_command_status",
    set: "set_shell_command",
    args: { adminPrompt: t("systemIntegration.adminPrompt") },
    failedTitle: t("systemIntegration.shellCommandFailed"),
  })
  const folderMenu = useSystemIntegration({
    status: "folder_menu_status",
    set: "set_folder_menu",
    failedTitle: t("systemIntegration.folderMenuFailed"),
  })

  if (!isShown(shellCommand, offered) && !isShown(folderMenu, offered)) return null

  return (
    <SettingsSection
      icon={<SquareTerminal size={16} />}
      title={t("systemIntegration.title")}
      description={t("systemIntegration.description")}
    >
      <IntegrationRow
        icon={<SquareTerminal size={16} />}
        title={t("systemIntegration.shellCommand")}
        description={
          macos
            ? t("systemIntegration.shellCommandDescMacos")
            : t("systemIntegration.shellCommandDesc")
        }
        offered={offered}
        integration={shellCommand}
      />
      <IntegrationRow
        icon={<FolderOpen size={16} />}
        title={t("systemIntegration.folderMenu")}
        description={
          macos ? t("systemIntegration.folderMenuDescMacos") : t("systemIntegration.folderMenuDesc")
        }
        offered={offered}
        integration={folderMenu}
      />
    </SettingsSection>
  )
}
