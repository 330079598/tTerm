import { SerializeAddon } from "@xterm/addon-serialize"
import type { IDisposable, Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import type { TFunction } from "i18next"

import { restoredContentText, serializeTerminal } from "@/components/TerminalTab/terminalSnapshot"

/**
 * Keeps what each terminal tab shows on disk, when "Restore terminal
 * content" is on, so that the next launch shows it again above the tab's new
 * session. Terminals that changed are saved every few seconds, and all of
 * them once more when the app quits; the backend waits for that save.
 */

/** How often the terminals that changed are saved. */
export const SAVE_INTERVAL_MS = 10_000
const WILL_QUIT_EVENT = "app-will-quit"

/** A tab's content as the last launch saved it. */
export interface SavedTerminalContent {
  tabId: string
  content: string
  /** Milliseconds since the Unix epoch. */
  savedAt: number
}

interface TrackedTerminal {
  term: Terminal
  serializer: SerializeAddon
  /** Whether it changed since it was last saved. */
  dirty: boolean
}

const tracked = new Map<string, TrackedTerminal>()
const restored = new Map<string, SavedTerminalContent>()
let enabled = false
let suspended = false
let saveTimer: ReturnType<typeof setInterval> | null = null
let stopQuitListener: UnlistenFn | null = null
/** Bumped on each change of `enabled`, so a listener that comes late knows it is stale. */
let generation = 0
let pendingSave: Promise<void> = Promise.resolve()

/** Whether "Restore terminal content" is on. */
export function isTerminalContentRestoreEnabled(): boolean {
  return enabled
}

/** Saves the terminals that changed since they were last saved. */
export function saveTerminalContent(): Promise<void> {
  // Saves run one after another, so an older one never lands last.
  pendingSave = pendingSave.then(saveChanged).catch((error) => {
    console.error("Failed to save terminal content:", error)
  })
  return pendingSave
}

async function saveChanged(): Promise<void> {
  if (!enabled || suspended) return
  const snapshots: { tabId: string; content: string }[] = []
  for (const [tabId, entry] of tracked) {
    if (!entry.dirty) continue
    entry.dirty = false
    snapshots.push({ tabId, content: serializeTerminal(entry.term, entry.serializer) })
  }
  if (snapshots.length === 0) return
  try {
    await invoke("save_scrollback_snapshots", { snapshots })
  } catch (error) {
    console.error("Failed to save terminal content:", error)
    // The next save tries them again.
    for (const { tabId } of snapshots) {
      const entry = tracked.get(tabId)
      if (entry) entry.dirty = true
    }
  }
}

function answerQuit() {
  void saveTerminalContent().finally(() => {
    invoke("ready_to_quit").catch((error) => {
      console.error("Failed to tell the app it may quit:", error)
    })
  })
}

/**
 * Turns the saving on or off. Turning it off deletes what was saved, and so
 * does a launch with it off, since files of an earlier launch may remain.
 */
export function setTerminalContentRestore(next: boolean): void {
  if (!next) {
    if (enabled) {
      enabled = false
      generation += 1
      setSaveBeforeQuit(false)
      stopSaving()
      restored.clear()
    }
    void clearSavedContent()
    return
  }
  if (enabled) return
  enabled = true
  generation += 1
  setSaveBeforeQuit(true)
  // What the terminals show from before it was turned on is saved too.
  for (const entry of tracked.values()) entry.dirty = true
  saveTimer = setInterval(() => void saveTerminalContent(), SAVE_INTERVAL_MS)
  const current = generation
  listen(WILL_QUIT_EVENT, answerQuit)
    .then((unlisten) => {
      if (current === generation) stopQuitListener = unlisten
      else unlisten()
    })
    .catch((error) => {
      console.error("Failed to listen for the app quitting:", error)
    })
}

function setSaveBeforeQuit(value: boolean) {
  invoke("set_save_before_quit", { enabled: value }).catch((error) => {
    console.error("Failed to set up saving terminal content on quit:", error)
  })
}

function stopSaving() {
  if (saveTimer !== null) clearInterval(saveTimer)
  saveTimer = null
  stopQuitListener?.()
  stopQuitListener = null
}

function clearSavedContent(): Promise<void> {
  return invoke<void>("clear_scrollback_snapshots").catch((error) => {
    console.error("Failed to delete saved terminal content:", error)
  })
}

/**
 * Stops saving until the next launch. An imported session reuses the ids of
 * the tabs open now, and must not get their content.
 */
export function suspendTerminalContentSaving(): void {
  suspended = true
  restored.clear()
  if (saveTimer !== null) clearInterval(saveTimer)
  saveTimer = null
  // A quit no longer has anything to wait for.
  setSaveBeforeQuit(false)
}

/** Reads what the last launch saved of `tabIds`, for their terminals to show. */
export async function loadSavedTerminalContent(tabIds: string[]): Promise<void> {
  try {
    const saved = await invoke<SavedTerminalContent[]>("load_scrollback_snapshots", { tabIds })
    for (const content of saved) restored.set(content.tabId, content)
  } catch (error) {
    console.error("Failed to load saved terminal content:", error)
  }
}

function restoredBanner(savedAt: number, t: TFunction): string {
  const time = new Date(savedAt).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  })
  return t("sessionHeader.contentRestored", { time, defaultValue: "Restored · saved {{time}}" })
}

/**
 * Shows the tab's saved content in its new terminal, once per launch, and
 * keeps track of the terminal for the next save. Call it before the session
 * writes anything.
 */
export function restoreTerminalContent(tabId: string, term: Terminal, t: TFunction): IDisposable {
  const saved = restored.get(tabId)
  restored.delete(tabId)
  if (saved) term.write(restoredContentText(saved.content, restoredBanner(saved.savedAt, t)))

  // Disposed with the terminal.
  const serializer = new SerializeAddon()
  term.loadAddon(serializer)
  const entry: TrackedTerminal = { term, serializer, dirty: false }
  const writes = term.onWriteParsed(() => {
    entry.dirty = true
  })
  tracked.set(tabId, entry)
  return {
    dispose() {
      writes.dispose()
      if (tracked.get(tabId) === entry) tracked.delete(tabId)
    },
  }
}
