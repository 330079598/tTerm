import { describe, expect, it } from "vitest"

import {
  DEFAULT_REPLAY_VIEW_PREFERENCES,
  loadReplayViewPreferences,
  REPLAY_VIEW_STORAGE_KEY,
  saveReplayViewPreferences,
  stepReplayFontSize,
} from "@/components/LogsPanel/replayView"

function memoryStorage(initial: Record<string, string> = {}) {
  const items = new Map(Object.entries(initial))
  return {
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => void items.set(key, value),
  }
}

describe("replayView", () => {
  it("steps the font size in whole points within range", () => {
    expect(stepReplayFontSize(13, 1)).toBe(14)
    expect(stepReplayFontSize(13, -1)).toBe(12)
    expect(stepReplayFontSize(13.75, 1)).toBe(14)
    expect(stepReplayFontSize(13.25, -1)).toBe(13)
    expect(stepReplayFontSize(6, -1)).toBe(6)
    expect(stepReplayFontSize(48, 1)).toBe(48)
  })

  it("round-trips the preferences", () => {
    const storage = memoryStorage()
    saveReplayViewPreferences({ fontSize: 16, fitPanel: true }, storage)
    expect(loadReplayViewPreferences(storage)).toEqual({ fontSize: 16, fitPanel: true })
  })

  it("falls back on missing or malformed preferences", () => {
    expect(loadReplayViewPreferences(memoryStorage())).toEqual(DEFAULT_REPLAY_VIEW_PREFERENCES)
    expect(
      loadReplayViewPreferences(memoryStorage({ [REPLAY_VIEW_STORAGE_KEY]: "{not json" }))
    ).toEqual(DEFAULT_REPLAY_VIEW_PREFERENCES)
    expect(
      loadReplayViewPreferences(
        memoryStorage({ [REPLAY_VIEW_STORAGE_KEY]: '{"fontSize":200,"fitPanel":"yes"}' })
      )
    ).toEqual({ fontSize: 48, fitPanel: false })
  })
})
