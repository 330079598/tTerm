import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Switch } from "@/components/ui/switch"
import { toast } from "@/hooks/use-toast"
import { getDetectedPlatform } from "@/lib/uiPlatform"
import { toErrorMessage } from "@/lib/utils"

/** Whether new shells run tTerm as `tterm`, as `shell_command_status` returns it. */
interface ShellCommandStatus {
  /** Whether tTerm can add the command on this system. */
  supported: boolean
  installed: boolean
}

/** Ties tTerm into the system; hidden where it has nothing to offer yet. */
export const SystemIntegrationSection: React.FC = () => {
  const { t } = useTranslation()
  const [status, setStatus] = useState<ShellCommandStatus | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    invoke<ShellCommandStatus>("shell_command_status")
      .then(setStatus)
      .catch((error: unknown) => console.error("Failed to read the tterm command:", error))
  }, [])

  if (!status?.supported) return null

  const change = async (installed: boolean) => {
    setBusy(true)
    try {
      setStatus(
        await invoke<ShellCommandStatus>("set_shell_command", {
          installed,
          adminPrompt: t("systemIntegration.adminPrompt"),
        })
      )
    } catch (error) {
      toast({
        title: t("systemIntegration.shellCommandFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  return (
    <SettingsSection
      icon={<SquareTerminal size={16} />}
      title={t("systemIntegration.title")}
      description={t("systemIntegration.description")}
    >
      <SettingsRow
        title={t("systemIntegration.shellCommand")}
        description={
          getDetectedPlatform() === "macos"
            ? t("systemIntegration.shellCommandDescMacos")
            : t("systemIntegration.shellCommandDesc")
        }
        action={
          <Switch
            checked={status.installed}
            disabled={busy}
            onCheckedChange={(checked) => void change(checked)}
            aria-label={t("systemIntegration.shellCommand")}
          />
        }
      />
    </SettingsSection>
  )
}
