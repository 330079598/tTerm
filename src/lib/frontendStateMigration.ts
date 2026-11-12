import { invoke } from "@tauri-apps/api/core"

import { REPLAY_VIEW_STORAGE_KEY } from "@/components/LogsPanel/replayView"
import {
  SFTP_COLUMN_WIDTH_STORAGE_KEY,
  SFTP_VIEW_STORAGE_KEY,
} from "@/components/SftpDrawer/sftpView"
import {
  COMMAND_COMPOSER_LINE_BY_LINE_STORAGE_KEY,
  COMMAND_COMPOSER_OPEN_STORAGE_KEY,
} from "@/lib/commandComposer"
import {
  FRONTEND_STATE_KEYS,
  acknowledgeFrontendStateMirror,
  hydrateFrontendState,
  mirrorFrontendStateToLegacy,
  registerFrontendStateDegradedReload,
  registerFrontendStateLegacyMirror,
  reportFrontendStateStartupError,
  resetFrontendStateToLegacyForThisSession,
  type FrontendStateKey,
} from "@/lib/frontendStateCache"
import { PROFILE_RECENTS_STORAGE_KEY } from "@/lib/profileRecents"
import { QUICK_CONNECT_RECENTS_STORAGE_KEY } from "@/lib/quickConnect"
import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"
import { SERIAL_SEND_ECHO_STORAGE_KEY } from "@/lib/serialSendStore"

const LEGACY_STORAGE_KEYS: Record<FrontendStateKey, string> = {
  recentCommands: RECENT_COMMANDS_STORAGE_KEY,
  recentQuickConnections: QUICK_CONNECT_RECENTS_STORAGE_KEY,
  recentProfiles: PROFILE_RECENTS_STORAGE_KEY,
  sftpColumnWidths: SFTP_COLUMN_WIDTH_STORAGE_KEY,
  sftpView: SFTP_VIEW_STORAGE_KEY,
  serialSendEcho: SERIAL_SEND_ECHO_STORAGE_KEY,
  commandComposerOpen: COMMAND_COMPOSER_OPEN_STORAGE_KEY,
  commandComposerLineByLine: COMMAND_COMPOSER_LINE_BY_LINE_STORAGE_KEY,
  replayView: REPLAY_VIEW_STORAGE_KEY,
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

const VALID_SHAPES: Record<FrontendStateKey, (value: unknown) => boolean> = {
  recentCommands: Array.isArray,
  recentQuickConnections: Array.isArray,
  recentProfiles: Array.isArray,
  sftpColumnWidths: Array.isArray,
  sftpView: isObject,
  serialSendEcho: Array.isArray,
  commandComposerOpen: (value) => typeof value === "boolean",
  commandComposerLineByLine: (value) => typeof value === "boolean",
  replayView: isObject,
}

function legacyValueText(key: FrontendStateKey, value: unknown): string {
  if (key === "commandComposerOpen" || key === "commandComposerLineByLine") {
    return value === true ? "1" : "0"
  }
  return JSON.stringify(value)
}

registerFrontendStateLegacyMirror((key, value) => {
  window.localStorage.setItem(LEGACY_STORAGE_KEYS[key], legacyValueText(key, value))
})

/**
 * The values worth handing to the backend, as JSON text. They stay in
 * localStorage through 0.9.0 so an older Tauri version still works; 0.10.0
 * can remove them once the transition build has had a release cycle.
 */
export function readLegacyFrontendState(storage: Pick<Storage, "getItem">): Record<string, string> {
  const legacy: Record<string, string> = {}
  for (const key of FRONTEND_STATE_KEYS) {
    let raw: string | null
    try {
      raw = storage.getItem(LEGACY_STORAGE_KEYS[key])
    } catch {
      continue
    }
    if (raw === null) continue

    if (key === "commandComposerOpen" || key === "commandComposerLineByLine") {
      if (raw !== "1" && raw !== "0") continue
      legacy[key] = raw === "1" ? "true" : "false"
      continue
    }

    let value: unknown
    try {
      value = JSON.parse(raw) as unknown
    } catch {
      continue
    }
    if (!VALID_SHAPES[key](value)) continue
    if (key === "serialSendEcho") {
      // The old reader dropped non-string connection keys instead of the
      // whole list, so keep that behavior while moving it.
      legacy[key] = JSON.stringify((value as unknown[]).filter((item) => typeof item === "string"))
    } else {
      legacy[key] = raw
    }
  }
  return legacy
}

function readStorage(storage: Storage): Record<string, string> {
  try {
    return readLegacyFrontendState(storage)
  } catch {
    return {}
  }
}

/**
 * Puts the fields an import wrote into the backend back into the old-format
 * localStorage values first. The retry then compares this session's local
 * writes with the imported rows instead of overwriting either side blindly.
 */
function mirrorImportedFrontendStateToLegacy(state: unknown): void {
  if (!isObject(state)) return
  for (const key of FRONTEND_STATE_KEYS) {
    if (!Object.prototype.hasOwnProperty.call(state, key)) continue
    const value = state[key]
    if (value === undefined || value === null || !VALID_SHAPES[key](value)) continue
    try {
      window.localStorage.setItem(LEGACY_STORAGE_KEYS[key], legacyValueText(key, value))
    } catch (error) {
      console.error(`Failed to write the imported legacy value of '${key}':`, error)
    }
  }
}

async function retryMigrationForImport(importedState: unknown): Promise<unknown> {
  mirrorImportedFrontendStateToLegacy(importedState)
  return invoke<unknown>("migrate_frontend_state", {
    legacy: readStorage(window.localStorage),
  })
}

registerFrontendStateDegradedReload(retryMigrationForImport)

/**
 * Loads every value before React mounts. The first start hands the
 * localStorage values over; later starts compare them with the backend. If
 * the backend cannot be reached, this session keeps reading and writing
 * localStorage and the next start tries the migration again.
 */
export async function initializeFrontendState(): Promise<void> {
  const legacy = readStorage(window.localStorage)
  try {
    const state = await invoke<unknown>("migrate_frontend_state", { legacy })
    hydrateFrontendState(state)
    const mirrored = mirrorFrontendStateToLegacy()
    await acknowledgeFrontendStateMirror(mirrored)
  } catch (error) {
    console.error("Failed to load frontend state from the backend:", error)
    resetFrontendStateToLegacyForThisSession()
    reportFrontendStateStartupError(error instanceof Error ? error.message : String(error))
  }
}
