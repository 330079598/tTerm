import { Server, Terminal } from "lucide-react"
import React from "react"
import { useTranslation } from "react-i18next"

import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { Button } from "@/components/ui/button"
import { Tab } from "@/types/tab"

interface EmptyStateProps {
  handleConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  handleNewTab: () => void
  onEditProfile: (profile: SavedProfile) => void
  refreshKey?: number
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  handleConnect,
  handleNewTab,
  onEditProfile,
  refreshKey,
}) => {
  const { t } = useTranslation()

  return (
    <div className="terminal-placeholder">
      <div className="terminal-placeholder-hero">
        <div className="terminal-placeholder-heading">
          <h3>{t("welcome.title")}</h3>
          <p>{t("welcome.description")}</p>
        </div>
        <div className="terminal-placeholder-actions">
          <Button
            type="button"
            onClick={() =>
              handleConnect({
                title: t("profiles.localTerminal", { defaultValue: "Local terminal" }),
                type: "terminal",
                isModified: false,
                connection: { type: "terminal" },
              })
            }
          >
            <Terminal className="size-4" />
            {t("welcome.localTerminal", { defaultValue: "Local terminal" })}
          </Button>
          <Button type="button" variant="outline" onClick={handleNewTab}>
            <Server className="size-4" />
            {t("welcome.sshConnection", { defaultValue: "SSH connection" })}
          </Button>
        </div>
      </div>

      <div className="terminal-placeholder-panel">
        <ProfilesPanel
          refreshKey={refreshKey}
          className="h-full"
          onConnect={(connection) => {
            handleConnect(connection)
          }}
          onEdit={onEditProfile}
        />
      </div>
    </div>
  )
}
