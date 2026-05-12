import React from "react"
import { Pin, PinOff, RefreshCcw, Globe } from "lucide-react"
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

interface ConnectionHeaderProps {
  connection?: TerminalTabProps["connection"]
  connectionHeaderPinned: boolean
  connectionState: ConnectionState
  connectionProgress?: SshConnectionProgress | null
  onBackgroundMouseDown?: React.MouseEventHandler<HTMLDivElement>
  onPinConnectionHeader?: () => void
  onReconnect: () => void
  onToggleSftpDrawer: () => void
  onUnpinConnectionHeader?: () => void
}

export const ConnectionHeader: React.FC<ConnectionHeaderProps> = ({
  connection,
  connectionHeaderPinned,
  connectionState,
  connectionProgress,
  onBackgroundMouseDown,
  onPinConnectionHeader,
  onReconnect,
  onToggleSftpDrawer,
  onUnpinConnectionHeader,
}) => {
  const { t } = useTranslation()
  const showConnectionHeader = connection?.type === "ssh" && connectionHeaderPinned
  const showPinnedToggle = connection?.type === "ssh" && !connectionHeaderPinned
  const progressLabel =
    connectionState === "connecting" && connectionProgress
      ? getSshConnectionProgressLabel(connectionProgress)
      : null

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
                  aria-label={getConnectionStateLabel(connectionState, t)}
                >
                  <span className="connection-status-dot" />
                  <span className="sr-only">{getConnectionStateLabel(connectionState, t)}</span>
                </span>
              </TooltipTrigger>
              <TooltipContent>{getConnectionStateLabel(connectionState, t)}</TooltipContent>
            </Tooltip>
            <div className="connection-meta">
              <div className="connection-primary">{getConnectionDisplay(connection)}</div>
              {progressLabel && <div className="connection-progress-text">{progressLabel}</div>}
            </div>
          </div>

          <div className="connection-header-actions">
            <button type="button" className="connection-action" onClick={onReconnect}>
              <RefreshCcw size={14} />
              <span>{t("sessionHeader.reconnect", { defaultValue: "Reconnect" })}</span>
            </button>
            <button type="button" className="connection-action" onClick={onToggleSftpDrawer}>
              <Globe size={14} />
              <span>{t("sessionHeader.sftp", { defaultValue: "SFTP" })}</span>
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
