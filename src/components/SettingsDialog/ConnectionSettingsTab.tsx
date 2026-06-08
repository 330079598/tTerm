import React from "react"
import { Route } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"

interface ConnectionSettingsTabProps {
  handleShowJumpHostConnectionInfoChange: (checked: boolean) => Promise<void>
  showJumpHostConnectionInfo: boolean
}

export const ConnectionSettingsTab: React.FC<ConnectionSettingsTabProps> = ({
  handleShowJumpHostConnectionInfoChange,
  showJumpHostConnectionInfo,
}) => {
  const { t } = useTranslation()

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Route size={16} />}
          title={t("settings.jumpHost", { defaultValue: "Jump hosts" })}
          description={t("settings.connectionDesc", {
            defaultValue: "Control connection-related prompts and route details.",
          })}
        >
          <SettingsRow
            icon={<Route size={16} />}
            title={t("settings.showJumpHostConnectionInfo")}
            description={t("settings.showJumpHostConnectionInfoDesc")}
            action={
              <Switch
                checked={showJumpHostConnectionInfo}
                onCheckedChange={handleShowJumpHostConnectionInfoChange}
              />
            }
          />
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
