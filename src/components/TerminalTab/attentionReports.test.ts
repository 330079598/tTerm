import { describe, expect, it, vi } from "vitest"
import type { Terminal } from "@xterm/xterm"

import { watchAttentionReports } from "@/components/TerminalTab/attentionReports"
import { AgentTracker } from "@/lib/agentStatus"

function tab(cwd?: string) {
  let osc: (payload: string) => boolean = () => false
  const term = {
    parser: {
      registerOscHandler: (code: number, listener: typeof osc) => {
        if (code === 777) osc = listener
        return { dispose() {} }
      },
    },
    onBell: () => ({ dispose() {} }),
  } as unknown as Terminal
  const sessions = vi.fn()
  const tracker = new AgentTracker({
    shellCwd: () => cwd,
    onStatus: vi.fn(),
    onAgent: vi.fn(),
    onAnnounce: vi.fn(),
  })
  const reports = watchAttentionReports(term, {
    agentTracker: tracker,
    onAgentSession: sessions,
    onAttention: vi.fn(),
  })
  return {
    sessions,
    submit: () => tracker.submit(),
    receive: (payload: string) => osc(payload),
    dispose: () => {
      reports.dispose()
      tracker.dispose()
    },
  }
}

describe("resume metadata through terminal reports", () => {
  it.each(["claude", "codex"])(
    "direct %s reports persist in the receiving tab, broadcasts cannot adopt another session",
    (agent) => {
      const a = tab(),
        b = tab()
      a.receive(`tterm-agent;${agent};processing;session-a;direct;%2Fsame`)
      b.receive(`tterm-agent;${agent};processing;session-b;direct;%2Fsame`)
      const savedAgent = agent === "claude" ? "claudeCode" : "codex"
      expect(a.sessions).toHaveBeenLastCalledWith({
        agent: savedAgent,
        sessionId: "session-a",
        cwd: "/same",
      })
      expect(b.sessions).toHaveBeenLastCalledWith({
        agent: savedAgent,
        sessionId: "session-b",
        cwd: "/same",
      })
      b.sessions.mockClear()
      a.receive(`tterm-agent;${agent};done;session-a;broadcast;%2Fother`)
      b.receive(`tterm-agent;${agent};done;session-a;broadcast;%2Fother`)
      expect(a.sessions).toHaveBeenLastCalledWith({
        agent: savedAgent,
        sessionId: "session-a",
        cwd: "/same",
      })
      expect(b.sessions).not.toHaveBeenCalled()
      b.receive(`tterm-agent;${agent};ended;session-a;broadcast;`)
      expect(b.sessions).not.toHaveBeenCalled()
      a.receive(`tterm-agent;${agent};ended;session-a;broadcast;`)
      expect(a.sessions).toHaveBeenLastCalledWith(null)
      a.dispose()
      b.dispose()
    }
  )

  it("unowned broadcasts and malformed cwd never reach persistence", () => {
    const t = tab()
    t.receive("tterm-agent;codex;processing;unknown;broadcast;%2Ftmp")
    t.receive("tterm-agent;claude;processing;abc-123;direct;%0A")
    expect(t.sessions).not.toHaveBeenCalled()
    t.dispose()
  })

  it("persists a sole claimant but not competing claims through the terminal callback", () => {
    const a = tab("/tmp"),
      b = tab("/tmp")
    a.submit()
    a.receive("tterm-agent;codex;processing;sole;broadcast;%2Ftmp")
    b.receive("tterm-agent;codex;processing;sole;broadcast;%2Ftmp")
    expect(a.sessions).toHaveBeenLastCalledWith({ agent: "codex", sessionId: "sole", cwd: "/tmp" })
    expect(b.sessions).not.toHaveBeenCalled()
    a.sessions.mockClear()
    a.submit()
    b.submit()
    a.receive("tterm-agent;codex;processing;contested;broadcast;%2Ftmp")
    b.receive("tterm-agent;codex;processing;contested;broadcast;%2Ftmp")
    expect(a.sessions).not.toHaveBeenCalled()
    expect(b.sessions).not.toHaveBeenCalled()
    a.dispose()
    b.dispose()
  })
})
