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
let pendingStartupError: string | null = null
let pendingWriteError: string | null = null
let writeFailureReported = false
let writeQueue: Promise<void> = Promise.resolve()
const writeErrorListeners = new Set<(message: string) => void>()
const reloadListeners = new Set<() => void>()
let legacyMirror: ((key: FrontendStateKey, value: unknown) => void) | null = null
let degradedReload: ((importedState: unknown) => Promise<unknown>) | null = null

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
 * Installs the writer that keeps the 0.8.x localStorage copy current. The
 * migration module owns the old key names and string formats.
 */
export function registerFrontendStateLegacyMirror(
  mirror: (key: FrontendStateKey, value: unknown) => void
): void {
  legacyMirror = mirror
}

/**
 * Installs the retry used when an import finishes during a session whose
 * startup could not reach the backend. The migration module owns the legacy
 * key names and the retry command.
 */
export function registerFrontendStateDegradedReload(
  reload: (importedState: unknown) => Promise<unknown>
): void {
  degradedReload = reload
}

function mirrorValueToLegacy(key: FrontendStateKey, value: unknown): boolean {
  if (!legacyMirror) return false
  try {
    legacyMirror(key, value)
    return true
  } catch (error) {
    console.error(`Failed to write the legacy localStorage copy of '${key}':`, error)
    return false
  }
}

/** Refreshes every loaded value's old-format copy, for a rollback to 0.8.x. */
export function mirrorFrontendStateToLegacy(): FrontendStateKey[] {
  const mirrored: FrontendStateKey[] = []
  for (const key of FRONTEND_STATE_KEYS) {
    if (key in snapshot && mirrorValueToLegacy(key, snapshot[key])) mirrored.push(key)
  }
  return mirrored
}

/**
 * Leaves this session on localStorage after startup could not reach the
 * backend. The values are not emptied into the backend on the next write.
 */
export function resetFrontendStateToLegacyForThisSession(): void {
  snapshot = {}
  hydrated = false
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
      writeFailureReported = false
    } catch (error) {
      const message = toErrorMessage(error)
      if (!writeFailureReported) {
        writeFailureReported = true
        pendingWriteError = message
        for (const listener of writeErrorListeners) listener(message)
      }
      return
    }
    if (!mirrorValueToLegacy(key, value)) return
    try {
      await invoke("acknowledge_frontend_state_sync", { key, value })
    } catch (error) {
      console.error(`Failed to record the localStorage copy of '${key}':`, error)
    }
  })
}

/**
 * Loads every stored value again, after an import changed them. A session
 * that failed to load at startup retries the migration instead, so imported
 * data and this session's localStorage writes are reconciled together. If
 * that retry fails, the session stays on localStorage until the next start.
 */
export async function reloadFrontendState(importedState?: unknown): Promise<void> {
  const state = hydrated
    ? await invoke<unknown>("load_frontend_state")
    : await retryDegradedReload(importedState)
  hydrateFrontendState(state)
  const mirrored = mirrorFrontendStateToLegacy()
  for (const listener of reloadListeners) listener()
  for (const key of mirrored) {
    try {
      await invoke("acknowledge_frontend_state_sync", { key, value: snapshot[key] })
    } catch (error) {
      console.error(`Failed to record the localStorage copy of '${key}':`, error)
    }
  }
}

async function retryDegradedReload(importedState: unknown): Promise<unknown> {
  if (!degradedReload) {
    throw new Error("Frontend state migration is not available.")
  }
  return degradedReload(importedState)
}

/** Runs after an import replaced the cached values. */
export function onFrontendStateReloaded(listener: () => void): () => void {
  reloadListeners.add(listener)
  return () => {
    reloadListeners.delete(listener)
  }
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

/** The startup failure, shown once after React mounts. */
export function reportFrontendStateStartupError(message: string): void {
  pendingStartupError ??= message
}

export function takePendingFrontendStateStartupError(): string | null {
  const error = pendingStartupError
  pendingStartupError = null
  return error
}

/** Clears module state between tests. */
export function resetFrontendStateCacheForTests(): void {
  snapshot = {}
  hydrated = false
  pendingStartupError = null
  pendingWriteError = null
  writeFailureReported = false
  writeQueue = Promise.resolve()
  writeErrorListeners.clear()
  reloadListeners.clear()
}
