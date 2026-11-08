import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { AgentTracker, AGENT_CLAIM_WINDOW_MS } from "@/lib/agentStatus"

describe("broadcast resume claims preserve dev status routing", () => {
  const trackers: AgentTracker[] = []
  const make = () => {
    const status = vi.fn()
    const tracker = new AgentTracker({ onStatus: status, onAgent: vi.fn(), onAnnounce: vi.fn() })
    trackers.push(tracker)
    return { tracker, status }
  }
  const report = {
    agent: "codex",
    session: "shared-session",
    state: "processing",
    scope: "broadcast",
    cwd: "/same directory",
  } as const

  beforeEach(() => vi.useFakeTimers())
  afterEach(() => {
    for (const tracker of trackers.splice(0)) tracker.dispose()
    vi.useRealTimers()
  })

  it("records a single recent claimant and keeps later broadcasts and its start cwd", () => {
    const a = make(),
      b = make()
    a.tracker.submit()
    expect(b.tracker.report(report)).toBe(false)
    expect(a.tracker.report(report)).toBe(true)
    expect(a.status).toHaveBeenCalledWith({ agent: "codex", activity: "processing" })
    expect(b.status).not.toHaveBeenCalled()
    expect(a.tracker.resumeSession(report)).toEqual({
      agent: "codex",
      sessionId: report.session,
      cwd: report.cwd,
    })
    vi.advanceTimersByTime(AGENT_CLAIM_WINDOW_MS + 1)
    expect(a.tracker.report({ ...report, cwd: "/later" })).toBe(true)
    expect(a.tracker.resumeSession(report)?.cwd).toBe(report.cwd)
  })

  it("keeps dev first receiver status routing but never records overlapping claimants", () => {
    const a = make(),
      b = make()
    a.tracker.submit()
    b.tracker.submit()
    expect(a.tracker.report(report)).toBe(true)
    expect(b.tracker.report(report)).toBe(false)
    expect(a.status).toHaveBeenCalledWith({ agent: "codex", activity: "processing" })
    expect(b.status).not.toHaveBeenCalled()
    expect(a.tracker.resumeSession(report)).toBeNull()
    expect(b.tracker.resumeSession(report)).toBeNull()
    vi.advanceTimersByTime(AGENT_CLAIM_WINDOW_MS + 1)
    a.tracker.submit()
    a.tracker.report(report)
    expect(a.tracker.resumeSession(report)).toBeNull()
  })

  it("never records or displays another tab's claimed session even after Enter", () => {
    const a = make(),
      b = make()
    a.tracker.submit()
    a.tracker.report(report)
    b.tracker.submit()
    expect(b.tracker.report(report)).toBe(false)
    expect(b.tracker.report({ ...report, scope: "direct" })).toBe(false)
    expect(b.tracker.resumeSession(report)).toBeNull()
    expect(b.status).not.toHaveBeenCalled()
    expect(a.tracker.report({ ...report, state: "waiting" })).toBe(true)
    expect(a.status).toHaveBeenLastCalledWith({ agent: "codex", activity: "waiting" })
  })

  it("does not forget a competing Enter consumed by another session's status claim", () => {
    const a = make(),
      b = make()
    a.tracker.submit()
    b.tracker.submit()
    a.tracker.report({ ...report, session: "first-session" })
    b.tracker.report(report)
    expect(a.tracker.resumeSession({ ...report, session: "first-session" })).toBeNull()
    expect(b.tracker.resumeSession(report)).toBeNull()
    expect(b.status).toHaveBeenCalledWith({ agent: "codex", activity: "processing" })
  })
})
