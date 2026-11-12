import { invoke } from "@tauri-apps/api/core"

import { toErrorMessage } from "@/lib/utils"

/** User data moved out of localStorage; each is one `app_meta` row. */
export const FRONTEND_STATE_KEYS = [
  "recentCommands",
  "recentQuickConnections",
  "recentProfiles",
  "sftpColumnWidths",
  "sftpView",
  "serialSendEcho",
  "commandComposerOpen",
  "commandComposerLineByLine",
  "replayView",
] as const

export type FrontendStateKey = (typeof FRONTEND_STATE_KEYS)[number]
export type FrontendStateSnapshot = Partial<Record<FrontendStateKey, unknown>>

let snapshot: FrontendStateSnapshot = {}
let hydrated = false
let pendingWriteError: string | null = null
let writeQueue: Promise<void> = Promise.resolve()
const writeErrorListeners = new Set<(message: string) => void>()

/** Whether startup finished loading the values from the backend. */
export function isFrontendStateHydrated(): boolean {
  return hydrated
}

/** The value loaded at startup; `undefined` when the key was never written. */
export function cachedFrontendState(key: FrontendStateKey): unknown {
  return snapshot[key]
}

function normalizeSnapshot(value: unknown): FrontendStateSnapshot {
  if (!value || typeof value !== "object") return {}
  const fields = value as Record<string, unknown>
  const next: FrontendStateSnapshot = {}
  for (const key of FRONTEND_STATE_KEYS) {
    if (key in fields) next[key] = fields[key]
  }
  return next
}

export function hydrateFrontendState(value: unknown): void {
  snapshot = normalizeSnapshot(value)
  hydrated = true
}

/**
 * Stores a value in the cache at once and writes it back in the background.
 * The caller has already checked {@link isFrontendStateHydrated}.
 */
export function writeFrontendStateValue(key: FrontendStateKey, value: unknown): void {
  snapshot[key] = value
  if (!hydrated) return
  // Invocations can finish out of order; one queue keeps the last value the
  // one that reaches the database last.
  writeQueue = writeQueue.then(async () => {
    try {
      await invoke("write_frontend_state", { key, value })
    } catch (error) {
      const message = toErrorMessage(error)
      pendingWriteError = message
      for (const listener of writeErrorListeners) listener(message)
    }
  })
}

/** Loads every stored value again, after an import changed them. */
export async function reloadFrontendState(): Promise<void> {
  const state = await invoke<unknown>("load_frontend_state")
  hydrateFrontendState(state)
}

/** A write failed after startup; the caller shows it to the user. */
export function onFrontendStateWriteError(listener: (message: string) => void): () => void {
  writeErrorListeners.add(listener)
  return () => {
    writeErrorListeners.delete(listener)
  }
}

/** An error that happened before the listener was mounted. */
export function takePendingFrontendStateWriteError(): string | null {
  const error = pendingWriteError
  pendingWriteError = null
  return error
}

/** Clears module state between tests. */
export function resetFrontendStateCacheForTests(): void {
  snapshot = {}
  hydrated = false
  pendingWriteError = null
  writeQueue = Promise.resolve()
  writeErrorListeners.clear()
}
