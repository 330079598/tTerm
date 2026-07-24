import { RadioTower, RefreshCw, Send, Square, X } from "lucide-react"
import { useCallback, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"

import type { Tab } from "@/types/tab"
import type {
  BroadcastMode,
  LiveBroadcastState,
  PtyWriteResult,
  TerminalRuntimeState,
} from "@/types/broadcast"
import { useConfig } from "@/contexts/ConfigContext"

type BroadcastManagerProps = {
  activeTabId: string | null
  mode: BroadcastMode
  liveState: LiveBroadcastState
  preparing: boolean
  latestResults: Record<string, PtyWriteResult>
  unavailableTabIds: string[]
  runtimeStates: Record<string, TerminalRuntimeState>
  selectedTabIds: string[]
  sourceTabId: string | null
  tabs: Tab[]
  onClear: () => void
  onModeChange: (mode: BroadcastMode) => void
  onReconnectTargets: () => Promise<void>
  onSelectAll: () => void
  onSelectVisible: () => void
  onSendCommand: (command: string) => Promise<boolean>
  onStartLive: (sourceTabId: string) => Promise<boolean>
  onStopLive: () => void
  onToggleTarget: (tabId: string) => void
}

export function BroadcastManager({
  activeTabId,
  mode,
  liveState,
  preparing,
  latestResults,
  unavailableTabIds,
  runtimeStates,
  selectedTabIds,
  sourceTabId,
  tabs,
  onClear,
  onModeChange,
  onReconnectTargets,
  onSelectAll,
  onSelectVisible,
  onSendCommand,
  onStartLive,
  onStopLive,
  onToggleTarget,
}: BroadcastManagerProps) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const [open, setOpen] = useState(false)
  const [command, setCommand] = useState("")
  const panelRef = useRef<HTMLDivElement>(null)
  const terminalTabs = tabs.filter((tab) => tab.type === "terminal" || tab.type === "ssh")
  const selected = new Set(selectedTabIds)
  const unavailable = new Set(unavailableTabIds)
  const activeTab = terminalTabs.find((tab) => tab.id === activeTabId)
  const isLiveRunning = liveState !== "idle"
  const canStartLive =
    !!activeTab &&
    runtimeStates[activeTab.id]?.connectionState === "connected" &&
    runtimeStates[activeTab.id]?.sessionNonce === (activeTab.sessionNonce ?? 0)
  const liveTargetCount = selectedTabIds.filter((tabId) => tabId !== activeTab?.id).length

  const close = useCallback(() => setOpen(false), [])

  useEffect(() => {
    if (!open) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (
        e.key === "Escape" &&
        !document.querySelector('[data-slot="dialog-content"][data-state="open"]')
      ) {
        close()
      }
    }
    document.addEventListener("keydown", onKeyDown)
    return () => document.removeEventListener("keydown", onKeyDown)
  }, [open, close])

  const handleOverlayClick = useCallback(
    (e: React.MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        close()
      }
    },
    [close]
  )

  const submitCommand = async () => {
    if (!command.trim() || preparing) return
    if (await onSendCommand(command)) {
      setCommand("")
    }
  }

  return (
    <div className="broadcast-manager">
      <button
        type="button"
        className={`tab-action broadcast-trigger ${isLiveRunning ? "is-live" : open ? "is-open" : ""}`}
        aria-label={t("broadcast.title")}
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <RadioTower size={16} />
        {isLiveRunning ? (
          <span className="broadcast-live-badge">LIVE</span>
        ) : selectedTabIds.length > 0 ? (
          <span className="broadcast-count-badge">{selectedTabIds.length}</span>
        ) : null}
      </button>

      {open &&
        createPortal(
          <div className="broadcast-overlay" onMouseDown={handleOverlayClick}>
            <div
              className="broadcast-panel"
              ref={panelRef}
              role="dialog"
              aria-label={t("broadcast.title")}
            >
              <div className="broadcast-panel-header">
                <div>
                  <strong>{t("broadcast.title")}</strong>
                  <span>{t("broadcast.selected", { count: selectedTabIds.length })}</span>
                </div>
                <div className="broadcast-header-right">
                  {isLiveRunning && <span className="broadcast-live-label">LIVE</span>}
                  <button
                    type="button"
                    className="broadcast-close-btn"
                    aria-label={t("broadcast.close", { defaultValue: "Close" })}
                    onClick={close}
                  >
                    <X size={14} />
                  </button>
                </div>
              </div>

              <div className="broadcast-mode-tabs">
                <button
                  type="button"
                  className={mode === "command" ? "active" : ""}
                  onClick={() => onModeChange("command")}
                  disabled={isLiveRunning || preparing}
                >
                  {t("broadcast.commandMode")}
                </button>
                <button
                  type="button"
                  className={mode === "live" ? "active live" : ""}
                  onClick={() => onModeChange("live")}
                  disabled={isLiveRunning || preparing}
                >
                  {t("broadcast.liveMode")}
                </button>
              </div>

              <div className="broadcast-target-actions">
                <button type="button" onClick={onSelectVisible} disabled={preparing}>
                  {t("broadcast.selectVisible")}
                </button>
                <button type="button" onClick={onSelectAll} disabled={preparing}>
                  {t("broadcast.selectAll")}
                </button>
                <button type="button" onClick={onClear} disabled={preparing}>
                  {t("broadcast.clear")}
                </button>
                <button
                  type="button"
                  className="broadcast-reconnect-button"
                  disabled={terminalTabs.length === 0 || preparing}
                  onClick={() => void onReconnectTargets()}
                >
                  <RefreshCw size={12} aria-hidden="true" />
                  {selectedTabIds.length > 0
                    ? t("broadcast.reconnectSelected", { count: selectedTabIds.length })
                    : t("broadcast.reconnectAll", { count: terminalTabs.length })}
                </button>
              </div>

              <div className="broadcast-target-list">
                {terminalTabs.length === 0 ? (
                  <div className="broadcast-empty">{t("broadcast.noTerminals")}</div>
                ) : (
                  terminalTabs.map((tab) => {
                    const runtime = runtimeStates[tab.id]
                    const writable =
                      !unavailable.has(tab.id) &&
                      runtime?.connectionState === "connected" &&
                      runtime.sessionNonce === (tab.sessionNonce ?? 0)
                    const isLiveSource = isLiveRunning && sourceTabId === tab.id
                    return (
                      <label key={tab.id} className={isLiveSource ? "disabled" : ""}>
                        <input
                          type="checkbox"
                          checked={selected.has(tab.id)}
                          disabled={isLiveSource || preparing}
                          onChange={() => onToggleTarget(tab.id)}
                        />
                        <span className="broadcast-target-name">{tab.title}</span>
                        <span className="broadcast-target-kind">
                          {tab.type === "ssh" ? "SSH" : t("broadcast.local")}
                        </span>
                        <span className={`broadcast-target-status ${writable ? "connected" : ""}`}>
                          {latestResults[tab.id]
                            ? t(`broadcast.status.${latestResults[tab.id].status}`)
                            : writable
                              ? t("broadcast.connected")
                              : runtime?.connectionState === "connecting"
                                ? t("broadcast.connecting")
                                : t("broadcast.unavailable")}
                        </span>
                      </label>
                    )
                  })
                )}
              </div>

              {mode === "live" ? (
                <div className="broadcast-live-controls">
                  {isLiveRunning ? (
                    <>
                      <p>
                        {t("broadcast.liveSource", {
                          name: terminalTabs.find((tab) => tab.id === sourceTabId)?.title ?? "-",
                        })}
                      </p>
                      <button type="button" onClick={onStopLive}>
                        <Square size={13} aria-hidden="true" />
                        {t("broadcast.stopLive")}
                      </button>
                    </>
                  ) : (
                    <>
                      <p>{t("broadcast.liveReadyDescription")}</p>
                      <button
                        type="button"
                        disabled={!canStartLive || liveTargetCount === 0 || preparing}
                        onClick={() => {
                          if (!activeTab) return
                          close()
                          void onStartLive(activeTab.id)
                        }}
                      >
                        <RadioTower size={13} aria-hidden="true" />
                        {preparing
                          ? t("broadcast.waitingForConnections")
                          : t("broadcast.startLive")}
                      </button>
                    </>
                  )}
                </div>
              ) : (
                <div className="broadcast-command-editor">
                  <textarea
                    value={command}
                    disabled={preparing}
                    placeholder={t("broadcast.commandPlaceholder")}
                    style={{ fontFamily: config.font_family }}
                    onChange={(event) => setCommand(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && !event.shiftKey) {
                        event.preventDefault()
                        void submitCommand()
                      }
                    }}
                  />
                  <button
                    type="button"
                    disabled={!command.trim() || selectedTabIds.length === 0 || preparing}
                    onClick={() => void submitCommand()}
                  >
                    <Send size={14} />
                    {preparing
                      ? t("broadcast.waitingForConnections")
                      : t("broadcast.send", { count: selectedTabIds.length })}
                  </button>
                </div>
              )}
            </div>
          </div>,
          document.body
        )}
    </div>
  )
}
