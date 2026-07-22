import { RadioTower, Send, Square } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import type { Tab } from "@/types/tab"
import type { BroadcastMode, PtyWriteResult, TerminalRuntimeState } from "@/types/broadcast"

type BroadcastManagerProps = {
  activeTabId: string | null
  mode: BroadcastMode
  latestResults: Record<string, PtyWriteResult>
  unavailableTabIds: string[]
  runtimeStates: Record<string, TerminalRuntimeState>
  selectedTabIds: string[]
  sourceTabId: string | null
  tabs: Tab[]
  onClear: () => void
  onModeChange: (mode: BroadcastMode) => void
  onSelectAll: () => void
  onSelectVisible: () => void
  onSendCommand: (command: string) => Promise<boolean>
  onStartLive: (sourceTabId: string) => void
  onStopLive: () => void
  onToggleTarget: (tabId: string) => void
}

export function BroadcastManager({
  activeTabId,
  mode,
  latestResults,
  unavailableTabIds,
  runtimeStates,
  selectedTabIds,
  sourceTabId,
  tabs,
  onClear,
  onModeChange,
  onSelectAll,
  onSelectVisible,
  onSendCommand,
  onStartLive,
  onStopLive,
  onToggleTarget,
}: BroadcastManagerProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [command, setCommand] = useState("")
  const terminalTabs = tabs.filter((tab) => tab.type === "terminal" || tab.type === "ssh")
  const selected = new Set(selectedTabIds)
  const unavailable = new Set(unavailableTabIds)
  const writableCount = selectedTabIds.filter((tabId) => {
    const tab = terminalTabs.find((candidate) => candidate.id === tabId)
    return (
      !!tab &&
      !unavailable.has(tabId) &&
      runtimeStates[tabId]?.connectionState === "connected" &&
      runtimeStates[tabId]?.sessionNonce === (tab.sessionNonce ?? 0)
    )
  }).length
  const activeTab = terminalTabs.find((tab) => tab.id === activeTabId)
  const canStartLive =
    !!activeTab &&
    runtimeStates[activeTab.id]?.connectionState === "connected" &&
    runtimeStates[activeTab.id]?.sessionNonce === (activeTab.sessionNonce ?? 0)

  const submitCommand = async () => {
    if (!command.trim()) return
    if (await onSendCommand(command)) {
      setCommand("")
    }
  }

  return (
    <div className="broadcast-manager">
      <button
        type="button"
        className={`tab-action broadcast-trigger ${mode === "live" ? "is-live" : open ? "is-open" : ""}`}
        aria-label={t("broadcast.title")}
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <RadioTower size={16} />
        {mode === "live" ? (
          <span className="broadcast-live-badge">LIVE</span>
        ) : selectedTabIds.length > 0 ? (
          <span className="broadcast-count-badge">{selectedTabIds.length}</span>
        ) : null}
      </button>

      {open && (
        <div className="broadcast-panel" role="dialog" aria-label={t("broadcast.title")}>
          <div className="broadcast-panel-header">
            <div>
              <strong>{t("broadcast.title")}</strong>
              <span>{t("broadcast.selected", { count: selectedTabIds.length })}</span>
            </div>
            {mode === "live" && <span className="broadcast-live-label">LIVE</span>}
          </div>

          <div className="broadcast-mode-tabs">
            <button
              type="button"
              className={mode === "command" ? "active" : ""}
              onClick={() => mode !== "live" && onModeChange("command")}
              disabled={mode === "live"}
            >
              {t("broadcast.commandMode")}
            </button>
            <button
              type="button"
              className={mode === "live" ? "active live" : ""}
              onClick={() => activeTab && onStartLive(activeTab.id)}
              disabled={!canStartLive && mode !== "live"}
            >
              {t("broadcast.liveMode")}
            </button>
          </div>

          <div className="broadcast-target-actions">
            <button type="button" onClick={onSelectVisible}>
              {t("broadcast.selectVisible")}
            </button>
            <button type="button" onClick={onSelectAll}>
              {t("broadcast.selectAll")}
            </button>
            <button type="button" onClick={onClear}>
              {t("broadcast.clear")}
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
                const isLiveSource = mode === "live" && sourceTabId === tab.id
                return (
                  <label key={tab.id} className={!writable || isLiveSource ? "disabled" : ""}>
                    <input
                      type="checkbox"
                      checked={selected.has(tab.id)}
                      disabled={!writable || isLiveSource}
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
              <p>
                {t("broadcast.liveSource", {
                  name: terminalTabs.find((tab) => tab.id === sourceTabId)?.title ?? "-",
                })}
              </p>
              <button type="button" onClick={onStopLive}>
                <Square size={13} />
                {t("broadcast.stopLive")}
              </button>
            </div>
          ) : (
            <div className="broadcast-command-editor">
              <textarea
                value={command}
                placeholder={t("broadcast.commandPlaceholder")}
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
                disabled={!command.trim() || writableCount === 0}
                onClick={() => void submitCommand()}
              >
                <Send size={14} />
                {t("broadcast.send", { count: writableCount })}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
