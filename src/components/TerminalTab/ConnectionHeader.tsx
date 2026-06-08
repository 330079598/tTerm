import React from "react"
import { Activity, Globe, Pin, PinOff, RefreshCcw, Route } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import {
  getConnectionDisplay,
  getConnectionStateLabel,
  getSshConnectionProgressLabel,
} from "@/components/TerminalTab/terminalTabUtils"
import {
  ConnectionState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"

function getConnectionProtocolLabel(connection?: TerminalTabProps["connection"]) {
  if (!connection || connection.type === "terminal") {
    return "Local"
  }

  return "SSH"
}

function getConnectionDetailItems(connection?: TerminalTabProps["connection"]) {
  if (!connection || connection.type !== "ssh") {
    return []
  }

  const items = []
  if (connection.profileName) {
    items.push(connection.profileName)
  }
  if (connection.port) {
    items.push(`:${connection.port}`)
  }
  if (connection.jumpHosts?.length) {
    items.push(`${connection.jumpHosts.length} jump${connection.jumpHosts.length === 1 ? "" : "s"}`)
  }

  return items
}

interface ConnectionHeaderProps {
  connection?: TerminalTabProps["connection"]
  connectionHeaderPinned: boolean
  connectionState: ConnectionState
  connectionProgress?: SshConnectionProgress | null
  onBackgroundMouseDown?: React.MouseEventHandler<HTMLDivElement>
  onPinConnectionHeader?: () => void
  onReconnect: () => void
  onToggleServerMonitor: () => void
  onToggleSftpDrawer: () => void
  onUnpinConnectionHeader?: () => void
  serverMonitorVisible: boolean
}

export const ConnectionHeader: React.FC<ConnectionHeaderProps> = ({
  connection,
  connectionHeaderPinned,
  connectionState,
  connectionProgress,
  onBackgroundMouseDown,
  onPinConnectionHeader,
  onReconnect,
  onToggleServerMonitor,
  onToggleSftpDrawer,
  onUnpinConnectionHeader,
  serverMonitorVisible,
}) => {
  const { t } = useTranslation()
  const showConnectionHeader = connection?.type === "ssh" && connectionHeaderPinned
  const showPinnedToggle = connection?.type === "ssh" && !connectionHeaderPinned
  const progressLabel =
    connectionState === "connecting" && connectionProgress
      ? getSshConnectionProgressLabel(connectionProgress)
      : null
  const stateLabel = getConnectionStateLabel(connectionState, t)
  const detailItems = getConnectionDetailItems(connection)
  const jumpHostCount = connection?.jumpHosts?.length ?? 0

  return (
    <>
      {showPinnedToggle && (
        <button type="button" className="connection-header-restore" onClick={onPinConnectionHeader}>
          <Pin size={14} />
          <span>{t("sessionHeader.connectionInfo", { defaultValue: "Connection" })}</span>
        </button>
      )}

      {showConnectionHeader && (
        <div className="connection-header" onMouseDown={onBackgroundMouseDown}>
          <div className="connection-header-main">
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className={`connection-status-pill is-${connectionState}`}
                  aria-label={stateLabel}
                >
                  <span className="connection-status-dot" />
                  <span className="sr-only">{stateLabel}</span>
                </span>
              </TooltipTrigger>
              <TooltipContent>{stateLabel}</TooltipContent>
            </Tooltip>
            <div className="connection-meta">
              <div className="connection-primary-row">
                <span className="connection-kind">{getConnectionProtocolLabel(connection)}</span>
                <span className="connection-primary">{getConnectionDisplay(connection)}</span>
                <span className={`connection-state-label is-${connectionState}`}>{stateLabel}</span>
              </div>
              <div className="connection-secondary-row">
                {progressLabel ? (
                  <span className="connection-progress-text">{progressLabel}</span>
                ) : (
                  detailItems.map((item) => (
                    <span key={item} className="connection-detail-item">
                      {item}
                    </span>
                  ))
                )}
              </div>
            </div>
          </div>

          <div className="connection-header-actions">
            {jumpHostCount > 0 && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span
                    className="connection-route-indicator"
                    aria-label={`${jumpHostCount} jump host route`}
                  >
                    <Route size={13} />
                    <span>{jumpHostCount}</span>
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  {t("sessionHeader.jumpRoute", {
                    count: jumpHostCount,
                    defaultValue: "{{count}} jump host route",
                  })}
                </TooltipContent>
              </Tooltip>
            )}
            <button type="button" className="connection-action" onClick={onReconnect}>
              <RefreshCcw size={14} />
              <span>{t("sessionHeader.reconnect", { defaultValue: "Reconnect" })}</span>
            </button>
            <button type="button" className="connection-action" onClick={onToggleSftpDrawer}>
              <Globe size={14} />
              <span>{t("sessionHeader.sftp", { defaultValue: "SFTP" })}</span>
            </button>
            <button
              type="button"
              className={`connection-action ${serverMonitorVisible ? "is-active" : ""}`}
              onClick={onToggleServerMonitor}
              aria-pressed={serverMonitorVisible}
            >
              <Activity size={14} />
              <span>
                {serverMonitorVisible
                  ? t("sessionHeader.hideMonitor", { defaultValue: "Hide Monitor" })
                  : t("sessionHeader.showMonitor", { defaultValue: "Monitor" })}
              </span>
            </button>
            <button type="button" className="connection-action" onClick={onUnpinConnectionHeader}>
              <PinOff size={14} />
              <span>{t("sessionHeader.unpin", { defaultValue: "Unpin" })}</span>
            </button>
          </div>
        </div>
      )}
    </>
  )
}
