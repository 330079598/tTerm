import React from "react"
import { FolderOpen, SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import {
  useSystemIntegration,
  type IntegrationStatus,
} from "@/components/SettingsDialog/useSystemIntegration"
import { Switch } from "@/components/ui/switch"
import { getDetectedPlatform } from "@/lib/uiPlatform"

interface IntegrationRowProps {
  icon: React.ReactNode
  title: string
  description: string
  integration: {
    status: IntegrationStatus | null
    busy: boolean
    change: (installed: boolean) => Promise<void>
  }
}

const IntegrationRow: React.FC<IntegrationRowProps> = ({
  icon,
  title,
  description,
  integration: { status, busy, change },
}) =>
  status?.supported ? (
    <SettingsRow
      icon={icon}
      title={title}
      description={description}
      action={
        <Switch
          checked={status.installed}
          disabled={busy}
          onCheckedChange={(checked) => void change(checked)}
          aria-label={title}
        />
      }
    />
  ) : null

/** Ties tTerm into the system; hidden where it has nothing to offer yet. */
export const SystemIntegrationSection: React.FC = () => {
  const { t } = useTranslation()
  const macos = getDetectedPlatform() === "macos"
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

  if (!shellCommand.status?.supported && !folderMenu.status?.supported) return null

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
        integration={shellCommand}
      />
      <IntegrationRow
        icon={<FolderOpen size={16} />}
        title={t("systemIntegration.folderMenu")}
        description={
          macos ? t("systemIntegration.folderMenuDescMacos") : t("systemIntegration.folderMenuDesc")
        }
        integration={folderMenu}
      />
    </SettingsSection>
  )
}
