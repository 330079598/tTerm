import { invoke } from "@tauri-apps/api/core"

import { readBackupFrontendState } from "@/lib/backupFrontendState"

/** Collections the backend syncs; `changed` lists the ones written locally. */
export type SyncCategory =
  | "profiles"
  | "profileGroups"
  | "tunnels"
  | "secrets"
  | "commands"
  | "knownHosts"
  | "settings"
  | "themes"

export interface SyncOutcome {
  state: "disabled" | "locked" | "busy" | "unchanged" | "synced"
  changed: SyncCategory[]
  uploaded: boolean
  customThemes: unknown[] | null
  recoveryBackup: string | null
}

export interface SyncSelection {
  profiles: boolean
  secrets: boolean
  commands: boolean
  knownHosts: boolean
  settings: boolean
  themes: boolean
}

export interface SyncSettings {
  enabled: boolean
  selection: SyncSelection
  /** How often local changes are looked for and uploaded. */
  pushIntervalSecs: number
  /** How often other devices' changes are downloaded. */
  pullIntervalMins: number
}

export const SYNC_PUSH_INTERVALS_SECS = [30, 60, 300, 900] as const
export const SYNC_PULL_INTERVALS_MINS = [5, 15, 30, 60] as const

/** Dispatched on `window` after the sync settings were saved. */
export const SYNC_SETTINGS_EVENT = "tterm:sync-settings"
/** Dispatched on `window` after a sync wrote local data; `detail` lists the categories. */
export const SYNC_APPLIED_EVENT = "tterm:sync-applied"
/** Dispatched on `window` after every sync attempt, for status displays. */
export const SYNC_STATUS_EVENT = "tterm:sync-status"

const CUSTOM_THEMES_STORAGE_KEY = "custom-themes"

/**
 * Runs one sync. Without `checkRemote` the backend returns early unless this
 * device has changes. Views reload through {@link onSyncApplied}.
 */
export async function runSync(checkRemote: boolean): Promise<SyncOutcome> {
  try {
    const outcome = await invoke<SyncOutcome>("run_sync", {
      frontendState: readBackupFrontendState(),
      checkRemote,
    })
    if (outcome.customThemes) {
      localStorage.setItem(CUSTOM_THEMES_STORAGE_KEY, JSON.stringify(outcome.customThemes))
    }
    if (outcome.changed.length > 0) {
      window.dispatchEvent(
        new CustomEvent<SyncCategory[]>(SYNC_APPLIED_EVENT, { detail: outcome.changed })
      )
    }
    return outcome
  } finally {
    window.dispatchEvent(new Event(SYNC_STATUS_EVENT))
  }
}

/** Calls `handler` when a sync changed any of `categories`; returns the unsubscribe. */
export function onSyncApplied(categories: SyncCategory[], handler: () => void): () => void {
  const listener = (event: Event) => {
    const changed = (event as CustomEvent<SyncCategory[]>).detail ?? []
    if (changed.some((category) => categories.includes(category))) handler()
  }
  window.addEventListener(SYNC_APPLIED_EVENT, listener)
  return () => window.removeEventListener(SYNC_APPLIED_EVENT, listener)
}
