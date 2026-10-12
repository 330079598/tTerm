import { useTranslation } from "react-i18next"
import { Keyboard, Pause, Play, Square } from "lucide-react"

import type { LiveBroadcastState } from "@/types/broadcast"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

interface BroadcastControlsProps {
  liveBroadcastState: LiveBroadcastState
  fontFamily: string
  onPauseBroadcast: TerminalTabProps["onPauseBroadcast"]
  onResumeBroadcast: TerminalTabProps["onResumeBroadcast"]
  onStopBroadcast: TerminalTabProps["onStopBroadcast"]
}

/** The bar on a live broadcast's source terminal: its state, and pausing or stopping it. */
export function BroadcastControls({
  liveBroadcastState,
  fontFamily,
  onPauseBroadcast,
  onResumeBroadcast,
  onStopBroadcast,
}: BroadcastControlsProps) {
  const { t } = useTranslation()

  return (
    <div className={`terminal-broadcast-controls is-${liveBroadcastState}`} style={{ fontFamily }}>
      <span role="status">
        <Keyboard size={13} aria-hidden="true" />
        <strong>{t("broadcast.primaryInput")}</strong>
        <span aria-hidden="true">·</span>
        <span>
          {liveBroadcastState === "paused" ? t("broadcast.livePaused") : t("broadcast.liveActive")}
        </span>
      </span>
      <div>
        {liveBroadcastState === "paused" ? (
          <button type="button" onClick={onResumeBroadcast}>
            <Play size={13} aria-hidden="true" />
            <span>{t("broadcast.resumeLive")}</span>
          </button>
        ) : (
          <button type="button" onClick={onPauseBroadcast}>
            <Pause size={13} aria-hidden="true" />
            <span>{t("broadcast.pauseLive")}</span>
          </button>
        )}
        <button type="button" className="destructive" onClick={onStopBroadcast}>
          <Square size={13} fill="currentColor" aria-hidden="true" />
          <span>{t("broadcast.stopLive")}</span>
        </button>
      </div>
    </div>
  )
}
