import { describe, expect, it } from "vitest"

import { isTabLogRecording, setRecordingTabIds } from "@/lib/terminalLogRecording"

describe("terminalLogRecording", () => {
  it("tracks the tabs the backend reports as recording", () => {
    setRecordingTabIds(["a", "b"])
    expect(isTabLogRecording("a")).toBe(true)
    expect(isTabLogRecording("c")).toBe(false)

    setRecordingTabIds(["c"])
    expect(isTabLogRecording("a")).toBe(false)
    expect(isTabLogRecording("c")).toBe(true)
  })
})
