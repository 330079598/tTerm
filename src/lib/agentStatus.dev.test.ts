import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import {
  AgentTracker,
  AGENT_CLAIM_WINDOW_MS,
  type AgentStatus,
  type AgentAnnouncement,
  type AgentReportState,
} from "@/lib/agentStatus"

describe("AgentTracker sessions told to every terminal", () => {
  let statuses: Array<AgentStatus | null>[]
  let announcements: AgentAnnouncement[][]
  let trackers: AgentTracker[]

  const tracker = (index: number) => {
    statuses[index] = []
    announcements[index] = []
    return new AgentTracker({
      onStatus: (status) => statuses[index].push(status),
      onAgent: () => {},
      onAnnounce: (announcement) => announcements[index].push(announcement),
    })
  }
  // A report as the shared server's hook sends it, to every terminal.
  const broadcast = (state: AgentReportState, session: string) => {
    for (const each of trackers) each.report({ agent: "codex", state, session })
  }

  beforeEach(() => {
    vi.useFakeTimers()
    statuses = []
    announcements = []
    trackers = [tracker(0), tracker(1)]
  })

  afterEach(() => {
    for (const each of trackers) each.dispose()
    vi.useRealTimers()
  })

  it("belongs to the terminal it was submitted from", () => {
    trackers[1].submit()
    vi.advanceTimersByTime(1500)
    broadcast("processing", "s1")
    expect(statuses[0]).toEqual([])
    expect(statuses[1]).toEqual([{ agent: "codex", activity: "processing" }])

    // Typing in the other terminal does not take it over.
    trackers[0].submit()
    broadcast("done", "s1")
    expect(announcements[0]).toEqual([])
    expect(announcements[1]).toEqual([{ agent: "codex", state: "done" }])
  })

  it("is nobody's without a recent Enter", () => {
    trackers[0].submit()
    vi.advanceTimersByTime(AGENT_CLAIM_WINDOW_MS + 1)
    broadcast("processing", "s1")
    expect(statuses).toEqual([[], []])
  })

  it("takes one session per Enter", () => {
    trackers[0].submit()
    broadcast("processing", "s1")
    broadcast("processing", "s2")
    expect(statuses[0]).toEqual([{ agent: "codex", activity: "processing" }])
    expect(statuses[1]).toEqual([])
  })

  it("is free again once it ends or its terminal resets", () => {
    trackers[0].submit()
    broadcast("processing", "s1")
    broadcast("ended", "s1")
    trackers[1].submit()
    broadcast("processing", "s1")
    expect(statuses[1]).toEqual([{ agent: "codex", activity: "processing" }])

    trackers[1].reset()
    trackers[0].submit()
    broadcast("done", "s1")
    expect(announcements[0]).toEqual([{ agent: "codex", state: "done" }])
  })
})
