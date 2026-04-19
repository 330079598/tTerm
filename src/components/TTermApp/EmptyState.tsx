import React from "react"
import { useTranslation } from "react-i18next"

import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { Tab } from "@/types/tab"

interface EmptyStateProps {
  handleConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  handleNewTab: () => void
  onEditProfile: (profile: SavedProfile) => void
  profilesRefreshKey: number
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  handleConnect,
  handleNewTab,
  onEditProfile,
  profilesRefreshKey,
}) => {
  const { t } = useTranslation()

  return (
    <div className="terminal-placeholder">
      <div className="terminal-placeholder-hero">
        <h3>{t("welcome.title")}</h3>
        <p>{t("welcome.description")}</p>
        <button onClick={handleNewTab} className="btn-primary" style={{ marginTop: "16px" }}>
          {t("welcome.newConnection")}
        </button>
      </div>

      <div className="terminal-placeholder-panel">
        <ProfilesPanel
          refreshKey={profilesRefreshKey}
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
