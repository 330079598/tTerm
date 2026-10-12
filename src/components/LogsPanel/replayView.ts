import { clampReplayFontSize } from "@/lib/terminalLogReplay"

/** How the replay is drawn; remembered across sessions. */
export interface ReplayViewPreferences {
  /** The replay's own font size, or null to use the terminal's. */
  fontSize: number | null
  /** Draw at the largest font size that fits the panel. */
  fitPanel: boolean
}

export const DEFAULT_REPLAY_VIEW_PREFERENCES: ReplayViewPreferences = {
  fontSize: null,
  fitPanel: false,
}

export const REPLAY_VIEW_STORAGE_KEY = "tterm.logs.replayView"

/** The font size one step smaller or larger, in whole points. */
export function stepReplayFontSize(fontSize: number, direction: -1 | 1): number {
  const whole = direction > 0 ? Math.floor(fontSize) : Math.ceil(fontSize)
  return clampReplayFontSize(whole + direction)
}

export function loadReplayViewPreferences(
  storage: Pick<Storage, "getItem"> = window.localStorage
): ReplayViewPreferences {
  try {
    const stored: unknown = JSON.parse(storage.getItem(REPLAY_VIEW_STORAGE_KEY) ?? "null")
    if (!stored || typeof stored !== "object") return DEFAULT_REPLAY_VIEW_PREFERENCES
    const { fontSize, fitPanel } = stored as Partial<ReplayViewPreferences>
    return {
      fontSize:
        typeof fontSize === "number" && Number.isFinite(fontSize)
          ? clampReplayFontSize(fontSize)
          : null,
      fitPanel: typeof fitPanel === "boolean" ? fitPanel : false,
    }
  } catch {
    return DEFAULT_REPLAY_VIEW_PREFERENCES
  }
}

export function saveReplayViewPreferences(
  preferences: ReplayViewPreferences,
  storage: Pick<Storage, "setItem"> = window.localStorage
) {
  try {
    storage.setItem(REPLAY_VIEW_STORAGE_KEY, JSON.stringify(preferences))
  } catch (error) {
    console.error("Failed to persist replay view preferences:", error)
  }
}
