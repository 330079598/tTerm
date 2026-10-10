import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  AgentTracker,
  agentDisplayName,
  parseAgentReport,
  AGENT_CLAIM_WINDOW_MS,
  type AgentAnnouncement,
  type AgentReportState,
  type AgentStatus,
} from "@/lib/agentStatus"

describe("parseAgentReport", () => {
  it("reads an agent and its state", () => {
    expect(parseAgentReport("tterm-agent;claude;processing")).toEqual({
      agent: "claude",
      state: "processing",
    })
    expect(parseAgentReport("tterm-agent;my-agent2;ended")).toEqual({
      agent: "my-agent2",
      state: "ended",
    })
  })

  it("reads the session a report names", () => {
    expect(parseAgentReport("tterm-agent;codex;done;01a12490-4b68-76d0")).toEqual({
      agent: "codex",
      state: "done",
      session: "01a12490-4b68-76d0",
    })
  })

  it("leaves other OSC 777 commands alone", () => {
    expect(parseAgentReport("notify;tterm-agent;done")).toBeNull()
    expect(parseAgentReport("precmd")).toBeNull()
  })

  it("rejects unknown states, odd names and extra fields", () => {
    expect(parseAgentReport("tterm-agent;claude;sleeping")).toBeNull()
    expect(parseAgentReport("tterm-agent;Claude;done")).toBeNull()
    expect(parseAgentReport("tterm-agent;;done")).toBeNull()
    expect(parseAgentReport("tterm-agent;claude;done;x;y")).toBeNull()
    expect(parseAgentReport("tterm-agent;claude;done;")).toBeNull()
    expect(parseAgentReport("tterm-agent;claude;done;a b")).toBeNull()
    expect(parseAgentReport("tterm-agent;claude")).toBeNull()
  })
})

describe("agentDisplayName", () => {
  it("names known agents and keeps others as they are", () => {
    expect(agentDisplayName("claude")).toBe("Claude Code")
    expect(agentDisplayName("aider")).toBe("aider")
  })
})

describe("AgentTracker", () => {
  let statuses: Array<AgentStatus | null>
  let announcements: AgentAnnouncement[]
  let tracker: AgentTracker

  beforeEach(() => {
    vi.useFakeTimers()
    statuses = []
    announcements = []
    tracker = new AgentTracker(
      {
        onStatus: (status) => statuses.push(status),
        onAnnounce: (announcement) => announcements.push(announcement),
      },
      3000
    )
  })

  afterEach(() => {
    tracker.dispose()
    vi.useRealTimers()
  })

  it("shows work and announces the end of a turn", () => {
    tracker.report({ agent: "claude", state: "processing" })
    tracker.report({ agent: "claude", state: "processing" })
    expect(statuses).toEqual([{ agent: "claude", activity: "processing" }])
    expect(tracker.speaksForAgent).toBe(true)

    tracker.report({ agent: "claude", state: "done" })
    expect(statuses[statuses.length - 1]).toBeNull()
    expect(announcements).toEqual([{ agent: "claude", state: "done" }])
    // Its own notification about the same turn is a repeat for a while.
    expect(tracker.speaksForAgent).toBe(true)
    vi.advanceTimersByTime(10_000)
    expect(tracker.speaksForAgent).toBe(false)
  })

  it("announces a wait only once it lasts", () => {
    tracker.report({ agent: "claude", state: "waiting" })
    expect(statuses).toEqual([{ agent: "claude", activity: "waiting" }])
    vi.advanceTimersByTime(2999)
    expect(announcements).toEqual([])
    vi.advanceTimersByTime(1)
    expect(announcements).toEqual([{ agent: "claude", state: "waiting" }])
  })

  it("forgets a wait answered in time", () => {
    tracker.report({ agent: "claude", state: "waiting" })
    vi.advanceTimersByTime(1000)
    tracker.report({ agent: "claude", state: "processing" })
    vi.advanceTimersByTime(5000)
    expect(announcements).toEqual([])
    expect(statuses[statuses.length - 1]).toEqual({ agent: "claude", activity: "processing" })
  })

  it("speaks for an agent at work however long it takes", () => {
    tracker.report({ agent: "claude", state: "processing" })
    vi.advanceTimersByTime(600_000)
    expect(tracker.speaksForAgent).toBe(true)
  })

  it("clears the status on an interrupt", () => {
    tracker.interrupt()
    expect(statuses).toEqual([])
    tracker.report({ agent: "claude", state: "waiting" })
    tracker.interrupt()
    vi.advanceTimersByTime(5000)
    expect(statuses[statuses.length - 1]).toBeNull()
    expect(announcements).toEqual([])
  })

  it("goes quiet when the agent reports it was stopped", () => {
    tracker.report({ agent: "codex", state: "waiting" })
    tracker.report({ agent: "codex", state: "idle" })
    vi.advanceTimersByTime(5000)
    expect(statuses[statuses.length - 1]).toBeNull()
    expect(announcements).toEqual([])
    // Still the agent's own turn of events for a while.
    expect(tracker.speaksForAgent).toBe(true)
  })

  it("stops on the end of the session or a reset", () => {
    tracker.report({ agent: "claude", state: "waiting" })
    tracker.report({ agent: "claude", state: "ended" })
    vi.advanceTimersByTime(5000)
    expect(statuses[statuses.length - 1]).toBeNull()
    expect(announcements).toEqual([])
    expect(tracker.speaksForAgent).toBe(false)

    tracker.report({ agent: "claude", state: "processing" })
    tracker.reset()
    expect(statuses[statuses.length - 1]).toBeNull()
    expect(tracker.speaksForAgent).toBe(false)
  })
})

describe("AgentTracker sessions told to every terminal", () => {
  let statuses: Array<AgentStatus | null>[]
  let announcements: AgentAnnouncement[][]
  let trackers: AgentTracker[]

  const tracker = (index: number) => {
    statuses[index] = []
    announcements[index] = []
    return new AgentTracker({
      onStatus: (status) => statuses[index].push(status),
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
