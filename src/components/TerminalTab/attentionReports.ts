import type { IDisposable, Terminal } from "@xterm/xterm"

import { type AgentTracker, parseAgentReport } from "@/lib/agentStatus"
import {
  KittyNotificationAssembler,
  NOTIFICATION_OSC_CODES,
  parseOsc777Notification,
  parseOsc9Notification,
  type TerminalAttentionEvent,
} from "@/lib/terminalNotifications"

interface AttentionReportOptions {
  /** Follows the agent in the terminal; hears its reports. */
  agentTracker: AgentTracker
  onAttention: (event: TerminalAttentionEvent) => void
  onAgentSession: (session: { agent: "claudeCode" | "codex"; sessionId: string } | null) => void
}

/**
 * Follows what programs in the terminal ask the user to notice: the reports
 * of AI agents through tTerm's hooks, notifications (OSC 9, 777 and 99) and
 * the bell.
 */
export function watchAttentionReports(
  term: Terminal,
  { agentTracker, onAttention, onAgentSession }: AttentionReportOptions
): IDisposable {
  // Returns false like the directory reports, so an OSC 9 reaches both
  // handlers.
  const kittyNotifications = new KittyNotificationAssembler()
  const oscDisposables = NOTIFICATION_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      const report = osc === 777 ? parseAgentReport(data) : null
      if (report) {
        const accepted = agentTracker.report(report)
        if (accepted) {
          const session = agentTracker.resumeSession(report)
          if (session) onAgentSession(session)
          else if (report.state === "ended" || report.state === "idle") onAgentSession(null)
        }
        return false
      }
      // An agent's own notifications repeat what its reports say.
      if (agentTracker.speaksForAgent) return false
      const request =
        osc === 9
          ? parseOsc9Notification(data)
          : osc === 777
            ? parseOsc777Notification(data)
            : kittyNotifications.handle(data)
      if (request) onAttention({ kind: "request", ...request })
      return false
    })
  )
  const bellDisposable = term.onBell(() => onAttention({ kind: "bell" }))

  return {
    dispose() {
      for (const disposable of oscDisposables) disposable.dispose()
      bellDisposable.dispose()
    },
  }
}
