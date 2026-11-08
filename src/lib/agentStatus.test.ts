import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  AgentTracker,
  agentDisplayName,
  parseAgentReport,
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

  it("distinguishes direct sessions from broadcasts and rejects unknown scope", () => {
    for (const scope of ["direct", "broadcast"]) {
      expect(parseAgentReport(`tterm-agent;codex;done;abc-123;${scope}`)).toEqual({
        agent: "codex",
        state: "done",
        session: "abc-123",
        scope,
      })
    }
    expect(parseAgentReport("tterm-agent;codex;done;abc-123;other")).toBeNull()
  })

  it("leaves other OSC 777 commands alone", () => {
    expect(parseAgentReport("notify;tterm-agent;done")).toBeNull()
    expect(parseAgentReport("precmd")).toBeNull()
  })

  it("decodes hook cwd without allowing protocol delimiters or control sequences", () => {
    expect(
      parseAgentReport(
        "tterm-agent;claude;processing;abc-123;direct;C%3A%5CJohn%E2%80%99s%20files%3Bx"
      )?.cwd
    ).toBe("C:\\John’s files;x")
    for (const cwd of ["%0A", "%0D", "%09", "%00", "%1B", "%C2%85", "%FF", "%"]) {
      expect(parseAgentReport(`tterm-agent;codex;done;abc-123;direct;${cwd}`)).toBeNull()
    }
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
  let agents: Array<string | null>
  let announcements: AgentAnnouncement[]
  let tracker: AgentTracker

  beforeEach(() => {
    vi.useFakeTimers()
    statuses = []
    agents = []
    announcements = []
    tracker = new AgentTracker(
      {
        onStatus: (status) => statuses.push(status),
        onAgent: (agent) => agents.push(agent),
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

  it("leaves stops to an agent that reports them", () => {
    // Codex's Esc may only close a popup, and its Ctrl+C clear a draft.
    tracker.report({ agent: "codex", state: "processing" })
    tracker.interrupt()
    expect(statuses).toEqual([{ agent: "codex", activity: "processing" }])
  })

  it("works on once a wait is answered", () => {
    tracker.report({ agent: "codex", state: "waiting" })
    tracker.answer()
    vi.advanceTimersByTime(5000)
    expect(statuses[statuses.length - 1]).toEqual({ agent: "codex", activity: "processing" })
    expect(announcements).toEqual([])
    // A refusal is reported.
    tracker.report({ agent: "codex", state: "idle" })
    expect(statuses[statuses.length - 1]).toBeNull()
  })

  it("keeps waiting where an answer may be a refusal nobody reports", () => {
    tracker.report({ agent: "claude", state: "waiting" })
    tracker.answer()
    expect(statuses).toEqual([{ agent: "claude", activity: "waiting" }])
    tracker.report({ agent: "codex", state: "processing" })
    tracker.answer()
    expect(statuses[statuses.length - 1]).toEqual({ agent: "codex", activity: "processing" })
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

  it("names the agent running from its first report until it ends", () => {
    tracker.report({ agent: "claude", state: "processing" })
    tracker.report({ agent: "claude", state: "done" })
    // Idle between turns, but still running.
    tracker.report({ agent: "claude", state: "idle" })
    expect(agents).toEqual(["claude"])

    tracker.report({ agent: "claude", state: "ended" })
    expect(agents).toEqual(["claude", null])

    tracker.report({ agent: "pi", state: "waiting" })
    tracker.reset()
    expect(agents).toEqual(["claude", null, "pi", null])
  })
})

describe("direct and broadcast session ownership", () => {
  const create = () =>
    new AgentTracker({ onStatus: vi.fn(), onAgent: vi.fn(), onAnnounce: vi.fn() })

  it("records a direct session without Enter and never adopts a broadcast", () => {
    const a = create()
    const b = create()
    const direct = {
      agent: "codex",
      state: "processing",
      session: "own-1",
      scope: "direct",
    } as const
    expect(a.report(direct)).toBe(true)
    expect(a.resumeSession(direct)).toEqual({ agent: "codex", sessionId: "own-1" })
    const broadcast = { ...direct, scope: "broadcast" } as const
    expect(a.report(broadcast)).toBe(true)
    expect(b.report(broadcast)).toBe(false)
    expect(b.resumeSession(broadcast)).toBeNull()
    a.dispose()
    b.dispose()
  })

  it("only updates the exact session already restored in this tab", () => {
    const tracker = new AgentTracker({
      initialSession: { agent: "codex", sessionId: "restored" },
      onStatus: vi.fn(),
      onAgent: vi.fn(),
      onAnnounce: vi.fn(),
    })
    expect(
      tracker.report({
        agent: "codex",
        state: "processing",
        session: "restored",
        scope: "broadcast",
      })
    ).toBe(true)
    expect(
      tracker.report({ agent: "codex", state: "processing", session: "other", scope: "broadcast" })
    ).toBe(false)
    expect(
      tracker.report({
        agent: "claude",
        state: "processing",
        session: "restored",
        scope: "broadcast",
      })
    ).toBe(false)
    tracker.dispose()
  })

  it("uses shell cwd at tool start, falls back to hook cwd, and freezes it for that session", () => {
    for (const shell of [undefined, "/shell start"]) {
      let current = shell
      const tracker = new AgentTracker({
        shellCwd: () => current,
        onStatus: vi.fn(),
        onAgent: vi.fn(),
        onAnnounce: vi.fn(),
      })
      const report = {
        agent: "claude",
        state: "processing",
        session: "start-1",
        scope: "direct",
        cwd: "/hook start",
      } as const
      tracker.report(report)
      expect(tracker.resumeSession(report)?.cwd).toBe(shell ?? "/hook start")
      current = "/changed after start"
      tracker.report({ ...report, cwd: "/hook later" })
      expect(tracker.resumeSession(report)?.cwd).toBe(shell ?? "/hook start")
      tracker.dispose()
    }
  })

  it("two tabs in the same directory ignore each other's broadcast even after typing", () => {
    const a = create(),
      b = create()
    for (const [tracker, session] of [
      [a, "session-a"],
      [b, "session-b"],
    ] as const) {
      tracker.report({ agent: "codex", state: "processing", session, scope: "direct" })
      tracker.answer()
    }
    for (const scope of ["broadcast", undefined] as const) {
      const report = { agent: "codex", state: "done", session: "session-a", scope } as const
      expect(a.report(report)).toBe(true)
      expect(b.report(report)).toBe(false)
      expect(b.resumeSession(report)).toBeNull()
      expect(b.resumeSession({ ...report, session: "session-b" })?.sessionId).toBe("session-b")
    }
    a.dispose()
    b.dispose()
  })

  it.each(["claude", "codex"])(
    "direct %s session replaces the old record; ended, idle and reset clear it",
    (agent) => {
      const tracker = create()
      const direct = { agent, state: "processing", session: "first", scope: "direct" } as const
      tracker.report(direct)
      tracker.report({ ...direct, session: "second" })
      expect(tracker.report({ ...direct, scope: "broadcast" })).toBe(false)
      for (const state of ["ended", "idle"] as const) {
        tracker.report(direct)
        tracker.report({ ...direct, state, scope: "broadcast" })
        expect(tracker.resumeSession(direct)).toBeNull()
        expect(tracker.report({ ...direct, scope: "broadcast" })).toBe(false)
      }
      tracker.report(direct)
      tracker.reset()
      expect(tracker.report({ ...direct, scope: "broadcast" })).toBe(false)
      tracker.dispose()
    }
  )
})
