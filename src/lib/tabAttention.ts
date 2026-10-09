import { useSyncExternalStore } from "react"

import { strongerAttention, type TabAttentionLevel } from "@/lib/terminalNotifications"

// Marks on tabs the user is not looking at: new output, a bell, a
// notification. Kept apart from the tabs themselves so a mark neither
// re-renders the workspace nor is saved with the session.
let marks = new Map<string, TabAttentionLevel>()
const listeners = new Set<() => void>()

function emit() {
  for (const listener of listeners) listener()
}

export function markTabAttention(tabId: string, level: TabAttentionLevel) {
  const current = marks.get(tabId)
  const next = strongerAttention(current, level)
  if (next === current) return
  marks = new Map(marks).set(tabId, next)
  emit()
}

export function clearTabAttention(tabId: string) {
  if (!marks.has(tabId)) return
  marks = new Map(marks)
  marks.delete(tabId)
  emit()
}

export function getTabAttention(tabId: string): TabAttentionLevel | undefined {
  return marks.get(tabId)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useTabAttention(tabId: string): TabAttentionLevel | undefined {
  return useSyncExternalStore(subscribe, () => marks.get(tabId))
}
