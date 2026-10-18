import React, { useEffect, useState } from "react"
import {
  Binary,
  Clock,
  MessageSquareText,
  RefreshCcw,
  RotateCcw,
  SendHorizontal,
  ToggleLeft,
  ToggleRight,
  Zap,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { clearSerialDisplay, useSerialDisplay } from "@/components/TerminalTab/serialDisplay"
import { SerialSendBar } from "@/components/TerminalTab/SerialSendBar"
import {
  getConnectionStateLabel,
  getSshConnectionProgressLabel,
} from "@/components/TerminalTab/terminalTabUtils"
import type {
  ConnectionState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import { useSerialControls } from "@/components/TerminalTab/useSerialControls"
import { formatUsbId, normalizeSerialSettings, serialSummary } from "@/lib/serial"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"

interface SerialControlBarProps {
  tabId: string
  sessionNonce: number
  connection: NonNullable<TerminalTabProps["connection"]>
  connectionState: ConnectionState
  connectionProgress?: SshConnectionProgress | null
  onBackgroundMouseDown?: React.MouseEventHandler<HTMLDivElement>
  onReconnect: () => void
}

interface BarButtonProps {
  icon: React.ReactNode
  label: string
  hint: string
  pressed?: boolean
  onClick: () => void
}

const BarButton: React.FC<BarButtonProps> = ({ icon, label, hint, pressed, onClick }) => (
  <Tooltip>
    <TooltipTrigger asChild>
      <button
        type="button"
        className={`connection-action ${pressed ? "is-active" : ""}`}
        onMouseDown={keepFocusOnMouseDown}
        onClick={onClick}
        aria-pressed={pressed}
      >
        {icon}
        <span>{label}</span>
      </button>
    </TooltipTrigger>
    <TooltipContent>{hint}</TooltipContent>
  </Tooltip>
)

/** A serial tab's header: its port and settings, and the controls of its lines. */
export const SerialControlBar: React.FC<SerialControlBarProps> = ({
  tabId,
  sessionNonce,
  connection,
  connectionState,
  connectionProgress,
  onBackgroundMouseDown,
  onReconnect,
}) => {
  const { t } = useTranslation()
  const settings = normalizeSerialSettings(connection.serial)
  const [display, setDisplay] = useSerialDisplay(tabId, settings)
  const { dtr, rts, toggleLine, reset, sendBreak, setHexView } = useSerialControls(
    tabId,
    sessionNonce,
    connectionProgress
  )
  const [showSendBar, setShowSendBar] = useState(false)
  useEffect(() => () => clearSerialDisplay(tabId), [tabId])

  const stateLabel = getConnectionStateLabel(connectionState, t)
  const progressLabel =
    connectionState === "reconnecting" && connectionProgress
      ? getSshConnectionProgressLabel(connectionProgress, t)
      : null
  const details = [serialSummary(settings)]
  if (connection.encoding) details.push(t(`connection.encodingOptions.${connection.encoding}`))
  if (settings.usb) details.push(`USB ${formatUsbId(settings.usb)}`)
  const lineLabel = (name: string, level: boolean | null) =>
    level === null ? name : `${name} ${level ? t("serialControls.on") : t("serialControls.off")}`

  return (
    <>
      <div className="connection-header" onMouseDown={onBackgroundMouseDown}>
        <div className="connection-header-main">
          <span className={`connection-status-pill is-${connectionState}`} aria-label={stateLabel}>
            <span className="connection-status-dot" />
            <span className="sr-only">{stateLabel}</span>
          </span>
          <div className="connection-meta">
            <div className="connection-primary-row">
              <span className="connection-kind">{t("serialControls.kind")}</span>
              <span className="connection-primary">{settings.port}</span>
              <span className={`connection-state-label is-${connectionState}`}>{stateLabel}</span>
            </div>
            <div className="connection-secondary-row">
              {progressLabel ? (
                <span className="connection-progress-text">{progressLabel}</span>
              ) : (
                details.map((item) => (
                  <span key={item} className="connection-detail-item">
                    {item}
                  </span>
                ))
              )}
            </div>
          </div>
        </div>

        <div className="connection-header-actions">
          <BarButton
            icon={dtr ? <ToggleRight size={14} /> : <ToggleLeft size={14} />}
            label={lineLabel("DTR", dtr)}
            hint={t("serialControls.dtrHint")}
            pressed={dtr === true}
            onClick={() => toggleLine("dtr")}
          />
          <BarButton
            icon={rts ? <ToggleRight size={14} /> : <ToggleLeft size={14} />}
            label={lineLabel("RTS", rts)}
            hint={t("serialControls.rtsHint")}
            pressed={rts === true}
            onClick={() => toggleLine("rts")}
          />
          <BarButton
            icon={<RotateCcw size={14} />}
            label={t("serialControls.reset")}
            hint={t("serialControls.resetHint")}
            onClick={reset}
          />
          <BarButton
            icon={<Zap size={14} />}
            label={t("serialControls.break")}
            hint={t("serialControls.breakHint")}
            onClick={sendBreak}
          />
          <BarButton
            icon={<Binary size={14} />}
            label="HEX"
            hint={t("connection.serial.hexViewDesc")}
            pressed={display.hex}
            onClick={() => {
              const hex = !display.hex
              void setHexView(hex).then((ok) => {
                if (ok) setDisplay({ hex })
              })
            }}
          />
          <BarButton
            icon={<Clock size={14} />}
            label={t("serialControls.timestamps")}
            hint={t("connection.serial.timestampsDesc")}
            pressed={display.timestamps}
            onClick={() => setDisplay({ timestamps: !display.timestamps })}
          />
          <BarButton
            icon={<MessageSquareText size={14} />}
            label={t("serialControls.localEcho")}
            hint={t("connection.serial.localEchoDesc")}
            pressed={display.localEcho}
            onClick={() => setDisplay({ localEcho: !display.localEcho })}
          />
          <BarButton
            icon={<SendHorizontal size={14} />}
            label={t("serialSend.send")}
            hint={t("serialControls.sendHint")}
            pressed={showSendBar}
            onClick={() => setShowSendBar((shown) => !shown)}
          />
          <BarButton
            icon={<RefreshCcw size={14} />}
            label={t("sessionHeader.reconnect", { defaultValue: "Reconnect" })}
            hint={t("serialControls.reconnectHint")}
            onClick={onReconnect}
          />
        </div>
      </div>
      {showSendBar && (
        <SerialSendBar tabId={tabId} sessionNonce={sessionNonce} lineEnding={settings.lineEnding} />
      )}
    </>
  )
}
