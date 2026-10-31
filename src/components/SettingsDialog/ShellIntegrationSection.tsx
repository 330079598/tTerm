import { SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"

/** Chooses whether SSH sessions start with tTerm's shell integration by default. */
export function ShellIntegrationSection() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()

  return (
    <SettingsSection
      icon={<SquareTerminal size={16} />}
      title={t("settings.shellIntegration")}
      description={t("settings.shellIntegrationDesc")}
    >
      <SettingsRow
        icon={<SquareTerminal size={16} />}
        title={t("settings.sshShellIntegration")}
        description={t("settings.sshShellIntegrationDesc")}
        action={
          <Switch
            aria-label={t("settings.sshShellIntegration")}
            checked={config.ssh_shell_integration}
            onCheckedChange={(checked) => void saveSettings({ ssh_shell_integration: checked })}
          />
        }
      />
    </SettingsSection>
  )
}
