import { useSyncExternalStore } from "react"
import type { useTranslation } from "react-i18next"

import type { Tab, TabContextMenuAction } from "@/types/tab"

export interface TerminalLogStatus {
  /** Sessions are logged unless their connection or tab says otherwise. */
  enabled: boolean
  directory: string
  activeSessions: number
  recordingTabIds: string[]
  totalSizeBytes: number
  lastError?: string | null
}

// Tabs whose session is being logged, as the backend last reported them.
// Kept apart from the tabs themselves, like tab attention marks, so a change
// neither re-renders the workspace nor is saved with the session.
let recording = new Set<string>()
const listeners = new Set<() => void>()

export function setRecordingTabIds(tabIds: readonly string[]) {
  if (tabIds.length === recording.size && tabIds.every((tabId) => recording.has(tabId))) return
  recording = new Set(tabIds)
  for (const listener of listeners) listener()
}

export function isTabLogRecording(tabId: string): boolean {
  return recording.has(tabId)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useTabLogRecording(tabId: string): boolean {
  return useSyncExternalStore(subscribe, () => recording.has(tabId))
}

/** The tab menu item that starts or stops logging a terminal tab's session. */
export function getTabLogMenuAction(
  tab: Pick<Tab, "id" | "type">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction | null {
  if (tab.type !== "terminal" && tab.type !== "ssh") return null
  return isTabLogRecording(tab.id)
    ? { label: t("contextMenu.stopLogging"), action: "log-stop", icon: "record-stop" }
    : { label: t("contextMenu.startLogging"), action: "log-start", icon: "record" }
}

/** A size in bytes as the logging pages show it. */
export function formatLogSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KiB", "MiB", "GiB", "TiB"]
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** unitIndex
  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`
}
