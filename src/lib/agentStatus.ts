// What AI coding agents report through the hooks tTerm installs into them
// (src-tauri/src/agent_integration): OSC 777 ; tterm-agent ; <agent> ; <state>.

/** `idle`: stopped by the user, nothing to announce. `ended`: the agent quit. */
const REPORT_STATES = ["processing", "waiting", "done", "error", "idle", "ended"] as const

export type AgentReportState = (typeof REPORT_STATES)[number]

export interface AgentReport {
  agent: string
  state: AgentReportState
}

/** The payload of an OSC 777, which also carries `notify` and other extensions. */
export function parseAgentReport(data: string): AgentReport | null {
  const [command, agent, state, ...rest] = data.split(";")
  if (command !== "tterm-agent" || rest.length > 0) return null
  if (!agent || !/^[a-z0-9][a-z0-9-]{0,31}$/.test(agent)) return null
  if (!(REPORT_STATES as readonly string[]).includes(state)) return null
  return { agent, state: state as AgentReportState }
}

const DISPLAY_NAMES: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  opencode: "OpenCode",
  pi: "pi",
}

export function agentDisplayName(agent: string): string {
  return DISPLAY_NAMES[agent] ?? agent
}

/** What a tab shows while an agent in it is busy or asking. */
export interface AgentStatus {
  agent: string
  activity: "processing" | "waiting"
}

/** Worth telling the user: a wait that lasted, a finished or failed turn. */
export interface AgentAnnouncement {
  agent: string
  state: "waiting" | "done" | "error"
}

/**
 * A wait is announced only once it lasts this long: auto mode may approve the
 * request, and a background subagent's is denied at once.
 */
export const AGENT_WAIT_ANNOUNCE_DELAY_MS = 3000

/** How long after its last report an agent's own notifications count as repeats. */
const AGENT_ECHO_WINDOW_MS = 10_000

interface AgentTrackerCallbacks {
  onStatus: (status: AgentStatus | null) => void
  onAnnounce: (announcement: AgentAnnouncement) => void
}

/** Follows the reports of the agent running in one terminal. */
export class AgentTracker {
  private status: AgentStatus | null = null
  private lastReportAt: number | null = null
  private waitTimer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly callbacks: AgentTrackerCallbacks,
    private readonly waitDelayMs = AGENT_WAIT_ANNOUNCE_DELAY_MS
  ) {}

  /**
   * An agent is at work, waiting, or has just reported: notifications it
   * sends itself repeat its reports. Not until it ends, as Claude Code often
   * exits before writing its `SessionEnd` report.
   */
  get speaksForAgent() {
    if (this.status) return true
    return this.lastReportAt !== null && Date.now() - this.lastReportAt < AGENT_ECHO_WINDOW_MS
  }

  report({ agent, state }: AgentReport) {
    this.cancelWait()
    if (state === "ended") {
      this.reset()
      return
    }
    this.lastReportAt = Date.now()
    if (state === "idle") {
      this.setStatus(null)
      return
    }
    if (state === "processing" || state === "waiting") {
      this.setStatus({ agent, activity: state })
      if (state === "waiting") {
        this.waitTimer = setTimeout(() => {
          this.waitTimer = null
          this.callbacks.onAnnounce({ agent, state })
        }, this.waitDelayMs)
      }
      return
    }
    this.setStatus(null)
    this.callbacks.onAnnounce({ agent, state })
  }

  /**
   * Esc or Ctrl+C typed in the terminal: an agent at work stops and waits for
   * the user, without a hook to say so.
   */
  interrupt() {
    if (!this.status) return
    this.cancelWait()
    this.setStatus(null)
  }

  /** The shell's prompt is back or the session closed: no agent runs anymore. */
  reset() {
    this.cancelWait()
    this.lastReportAt = null
    this.setStatus(null)
  }

  dispose() {
    this.cancelWait()
  }

  private cancelWait() {
    if (this.waitTimer === null) return
    clearTimeout(this.waitTimer)
    this.waitTimer = null
  }

  private setStatus(status: AgentStatus | null) {
    const current = this.status
    if (current?.agent === status?.agent && current?.activity === status?.activity) return
    this.status = status
    this.callbacks.onStatus(status)
  }
}
