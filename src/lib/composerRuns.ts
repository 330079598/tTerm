import { useSyncExternalStore } from "react"

import type { ShellPromptState } from "@/components/TerminalTab/commandMarks"
import { composerInput, getComposerTarget } from "@/lib/commandComposer"
import {
  continueRun,
  isRunActive,
  promptReached,
  retryLine,
  startRun,
  stopRun,
  type ComposerRun,
} from "@/lib/composerRun"

// Each tab's line-by-line run, kept here rather than in the bar so that it
// goes on while another pane has the bar, or while the bar is closed.
let runs = new Map<string, ComposerRun>()
const unsubscribers = new Map<string, () => void>()
const listeners = new Set<() => void>()

function stopFollowing(tabId: string) {
  unsubscribers.get(tabId)?.()
  unsubscribers.delete(tabId)
}

/** Keeps `run` as tab `tabId`'s, and sends its running line when it has just started one. */
function apply(tabId: string, previous: ComposerRun, run: ComposerRun) {
  if (run === previous) return
  const sendsLine =
    run.phase === "running" && (previous.phase !== "running" || previous.current !== run.current)
  if (sendsLine) {
    const target = getComposerTarget(tabId)
    if (!target) {
      apply(tabId, previous, stopRun(previous))
      return
    }
    target.input(composerInput(run.lines[run.current].text, false))
  }
  if (!isRunActive(run)) stopFollowing(tabId)
  runs = new Map(runs).set(tabId, run)
  for (const listener of listeners) listener()
}

function update(tabId: string, change: (run: ComposerRun) => ComposerRun) {
  const run = runs.get(tabId)
  if (run) apply(tabId, run, change(run))
}

/**
 * Runs `lines` in tab `tabId`'s shell one at a time. Says why it didn't start
 * when the shell marks no prompts, so a line's end can't be told, or is busy
 * with a command, or with another run, rather than at its prompt.
 */
export function startComposerRun(
  tabId: string,
  lines: string[]
): "started" | Exclude<ShellPromptState, "prompt"> {
  const target = getComposerTarget(tabId)
  const state = target?.promptState() ?? "unmarked"
  if (state !== "prompt") return state
  if (!target || isRunActive(runs.get(tabId)) || lines.length === 0) return "busy"
  stopFollowing(tabId)
  const run = startRun(lines)
  const idle: ComposerRun = { ...run, phase: "done" }
  unsubscribers.set(
    tabId,
    target.onPrompt((event) => update(tabId, (current) => promptReached(current, event)))
  )
  apply(tabId, idle, run)
  return "started"
}

export const retryComposerRun = (tabId: string) => update(tabId, retryLine)
export const continueComposerRun = (tabId: string) => update(tabId, continueRun)
export const stopComposerRun = (tabId: string) => update(tabId, stopRun)

/** Forgets a run that is over, taking its progress off the bar. */
export function dismissComposerRun(tabId: string) {
  const run = runs.get(tabId)
  if (!run || isRunActive(run)) return
  runs = new Map(runs)
  runs.delete(tabId)
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Tab `tabId`'s run, while it runs or until its progress is dismissed. */
export function useComposerRun(tabId: string | null) {
  return useSyncExternalStore(subscribe, () => (tabId ? runs.get(tabId) : undefined))
}
