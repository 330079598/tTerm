import { useSyncExternalStore } from "react"

import type { AgentStatus } from "@/lib/agentStatus"
import { strongerAttention, type TabAttentionLevel } from "@/lib/terminalNotifications"

// Marks on tabs the user is not looking at: new output, a bell, a
// notification; how many system notifications a tab raised unseen; which AI
// agent runs in a tab and what it is doing; the command a tab's shell runs;
// and which tabs wait at a password prompt. Kept apart from the tabs
// themselves so a mark neither re-renders the workspace nor is saved with the
// session.
let marks = new Map<string, TabAttentionLevel>()
let unseenNotifications = new Map<string, number>()
let agentStatuses = new Map<string, AgentStatus>()
let runningAgents = new Map<string, string>()
let runningCommands = new Map<string, string>()
let passwordPrompts = new Set<string>()
const runningCommandTimers = new Map<string, ReturnType<typeof setTimeout>>()
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

/** Looking at the tab answers its mark and its unseen notifications. */
export function clearTabAttention(tabId: string) {
  if (!marks.has(tabId) && !unseenNotifications.has(tabId)) return
  marks = new Map(marks)
  marks.delete(tabId)
  unseenNotifications = new Map(unseenNotifications)
  unseenNotifications.delete(tabId)
  emit()
}

/** Counts a system notification the tab raised while the user looked away. */
export function countTabNotification(tabId: string) {
  unseenNotifications = new Map(unseenNotifications).set(
    tabId,
    (unseenNotifications.get(tabId) ?? 0) + 1
  )
  emit()
}

function unseenNotificationTotal(): number {
  let total = 0
  for (const count of unseenNotifications.values()) total += count
  return total
}

/**
 * Hears the number of system notifications no one has looked at yet, now and
 * whenever it changes. A plain subscription rather than a hook: the app icon
 * badge it feeds needs no render.
 */
export function watchUnseenNotificationTotal(listener: (total: number) => void): () => void {
  let last = unseenNotificationTotal()
  listener(last)
  const onChange = () => {
    const total = unseenNotificationTotal()
    if (total === last) return
    last = total
    listener(total)
  }
  listeners.add(onChange)
  return () => {
    listeners.delete(onChange)
  }
}

export function getTabAttention(tabId: string): TabAttentionLevel | undefined {
  return marks.get(tabId)
}

export function setTabAgentStatus(tabId: string, status: AgentStatus | null) {
  if (status) {
    agentStatuses = new Map(agentStatuses).set(tabId, status)
  } else {
    if (!agentStatuses.has(tabId)) return
    agentStatuses = new Map(agentStatuses)
    agentStatuses.delete(tabId)
  }
  emit()
}

/** The AI agent running in a tab, as its reports name it, or null once it ends. */
export function setTabRunningAgent(tabId: string, agent: string | null) {
  if ((runningAgents.get(tabId) ?? null) === agent) return
  runningAgents = new Map(runningAgents)
  if (agent) runningAgents.set(tabId, agent)
  else runningAgents.delete(tabId)
  emit()
}

/**
 * A command shows on its tab once it has run for a moment: one that ends at
 * once (`cd`, `ls`) would only make the title flicker.
 */
export const RUNNING_COMMAND_DELAY_MS = 1000

/** The command a tab's shell started, or null once its prompt is back. */
export function setTabRunningCommand(tabId: string, command: string | null) {
  clearTimeout(runningCommandTimers.get(tabId))
  runningCommandTimers.delete(tabId)
  if (command) {
    const timer = setTimeout(() => {
      runningCommandTimers.delete(tabId)
      runningCommands = new Map(runningCommands).set(tabId, command)
      emit()
    }, RUNNING_COMMAND_DELAY_MS)
    runningCommandTimers.set(tabId, timer)
  }
  if (!runningCommands.has(tabId)) return
  runningCommands = new Map(runningCommands)
  runningCommands.delete(tabId)
  emit()
}

/** Whether the tab's session waits at a password prompt, until the answer is submitted. */
export function setTabPasswordPrompt(tabId: string, waiting: boolean) {
  if (passwordPrompts.has(tabId) === waiting) return
  passwordPrompts = new Set(passwordPrompts)
  if (waiting) passwordPrompts.add(tabId)
  else passwordPrompts.delete(tabId)
  emit()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The strongest mark on any of the tabs, as the tab of the bar holding them as panes shows it. */
export function useTabAttention(tabIds: readonly string[]): TabAttentionLevel | undefined {
  return useSyncExternalStore(subscribe, () =>
    tabIds.reduce<TabAttentionLevel | undefined>((level, tabId) => {
      const mark = marks.get(tabId)
      return mark ? strongerAttention(level, mark) : level
    }, undefined)
  )
}

/** What an AI agent does in the first of the tabs that has one. */
export function useTabAgentStatus(tabIds: readonly string[]): AgentStatus | undefined {
  return useSyncExternalStore(subscribe, () => {
    for (const tabId of tabIds) {
      const status = agentStatuses.get(tabId)
      if (status) return status
    }
    return undefined
  })
}

/** The AI agent running in the first of the tabs that has one. */
export function useTabRunningAgent(tabIds: readonly string[]): string | undefined {
  return useSyncExternalStore(subscribe, () => {
    for (const tabId of tabIds) {
      const agent = runningAgents.get(tabId)
      if (agent) return agent
    }
    return undefined
  })
}

/** Whether any of the tabs waits at a password prompt. */
export function useTabPasswordPrompt(tabIds: readonly string[]): boolean {
  return useSyncExternalStore(subscribe, () => tabIds.some((tabId) => passwordPrompts.has(tabId)))
}

/** The command each tab's shell runs, by tab id. */
export function useRunningCommands(): ReadonlyMap<string, string> {
  return useSyncExternalStore(subscribe, () => runningCommands)
}
