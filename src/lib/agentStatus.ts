// What AI coding agents report through the hooks tTerm installs into them
// (src-tauri/src/agent_integration): OSC 777 ; tterm-agent ; <agent> ; <state>,
// Reports with sessions add ; <session> ; direct|broadcast ; <encoded-cwd>.
// Legacy session reports
// are broadcasts: they must never establish a resume session.

/** `idle`: stopped by the user, nothing to announce. `ended`: the agent quit. */
const REPORT_STATES = ["processing", "waiting", "done", "error", "idle", "ended"] as const

export type AgentReportState = (typeof REPORT_STATES)[number]

export interface AgentReport {
  agent: string
  state: AgentReportState
  /** Only direct reports can establish the terminal's resume session. */
  session?: string
  scope?: "direct" | "broadcast"
  cwd?: string
}

export interface AgentResumeSession {
  agent: "claudeCode" | "codex"
  sessionId: string
  /** Directory at agent start; older records can fall back to the local shell directory. */
  cwd?: string
}

/** The payload of an OSC 777, which also carries `notify` and other extensions. */
export function parseAgentReport(data: string): AgentReport | null {
  const [command, agent, state, session, scope, encodedCwd, ...rest] = data.split(";")
  if (command !== "tterm-agent" || rest.length > 0) return null
  if (!agent || !/^[a-z0-9][a-z0-9-]{0,31}$/.test(agent)) return null
  if (!(REPORT_STATES as readonly string[]).includes(state)) return null
  if (session === undefined) return { agent, state: state as AgentReportState }
  if (!/^[A-Za-z0-9-]{1,64}$/.test(session)) return null
  if (scope !== undefined && scope !== "direct" && scope !== "broadcast") return null
  let cwd: string | undefined
  if (encodedCwd) {
    try {
      cwd = decodeURIComponent(encodedCwd)
      if (cwd.length > 16384 || /[\p{Cc}]/u.test(cwd)) return null
    } catch {
      return null
    }
  }
  return {
    agent,
    state: state as AgentReportState,
    session,
    ...(scope ? { scope } : {}),
    ...(cwd ? { cwd } : {}),
  }
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

/**
 * How long after Enter a terminal may take a session it is told about as its
 * own: the hook runs once the prompt is submitted, after the login shell
 * Codex starts it with.
 */
export const AGENT_CLAIM_WINDOW_MS = 10_000

/**
 * Agents that report it themselves when the user stops them or refuses what
 * they ask, which Claude Code does not. Their keys are no sign of a stop:
 * Codex's Esc closes a popup first, and its Ctrl+C clears a draft. A guess
 * that an answer resumed their work is put right by the report if it did not.
 */
const AGENTS_REPORTING_STOPS: ReadonlySet<string> = new Set(["codex", "opencode", "pi"])

/** The terminal each addressed session was submitted from, across tabs. */
const sessionOwners = new Map<string, AgentTracker>()

interface AgentTrackerCallbacks {
  /** A restored record is already owned by this tab; broadcasts cannot create it. */
  initialSession?: AgentResumeSession
  /** The directory reported by the live shell at tool start, before hook fallback. */
  shellCwd?: () => string | undefined
  onStatus: (status: AgentStatus | null) => void
  /** The agent running in the terminal, from its first report until it ends; null for none. */
  onAgent: (agent: string | null) => void
  onAnnounce: (announcement: AgentAnnouncement) => void
}

/** Follows the reports of the agent running in one terminal. */
export class AgentTracker {
  private status: AgentStatus | null = null
  private agent: string | null = null
  private lastReportAt: number | null = null
  private waitTimer: ReturnType<typeof setTimeout> | null = null
  private resume: AgentResumeSession | null = null
  private submittedAt: number | null = null
  private readonly sessions = new Set<string>()

  constructor(
    private readonly callbacks: AgentTrackerCallbacks,
    private readonly waitDelayMs = AGENT_WAIT_ANNOUNCE_DELAY_MS
  ) {
    this.resume = callbacks.initialSession ?? null
  }

  /**
   * An agent is at work, waiting, or has just reported: notifications it
   * sends itself repeat its reports. Not until it ends, as Claude Code often
   * exits before writing its `SessionEnd` report.
   */
  get speaksForAgent() {
    if (this.status) return true
    return this.lastReportAt !== null && Date.now() - this.lastReportAt < AGENT_ECHO_WINDOW_MS
  }

  /** Enter typed in the terminal: it may take the next new session it hears of. */
  submit() {
    this.submittedAt = Date.now()
  }

  report({ agent, state, session, scope, cwd }: AgentReport): boolean {
    if (session !== undefined) {
      if (scope === "direct") {
        if (agent === "claude" || agent === "codex") {
          const same =
            this.resume?.sessionId === session &&
            this.resume.agent === (agent === "claude" ? "claudeCode" : agent)
          const startCwd = same ? this.resume?.cwd : (this.callbacks.shellCwd?.() ?? cwd)
          this.resume = {
            agent: agent === "claude" ? "claudeCode" : "codex",
            sessionId: session,
            ...(startCwd ? { cwd: startCwd } : {}),
          }
        }
      } else {
        const restored =
          this.resume?.sessionId === session &&
          this.resume.agent === (agent === "claude" ? "claudeCode" : agent)
        if (!restored && !this.claim(session, state)) return false
        if (this.resume?.sessionId === session && !restored) return false
      }
    }
    if (state === "idle") this.resume = null
    this.cancelWait()
    if (state === "ended") {
      this.reset()
      return true
    }
    this.lastReportAt = Date.now()
    this.setAgent(agent)
    if (state === "idle") {
      this.setStatus(null)
      return true
    }
    if (state === "processing" || state === "waiting") {
      this.setStatus({ agent, activity: state })
      if (state === "waiting") {
        this.waitTimer = setTimeout(() => {
          this.waitTimer = null
          this.callbacks.onAnnounce({ agent, state })
        }, this.waitDelayMs)
      }
      return true
    }
    this.setStatus(null)
    this.callbacks.onAnnounce({ agent, state })
    return true
  }

  resumeSession({ agent, state, session }: AgentReport): AgentResumeSession | null {
    if (!session || state === "ended" || state === "idle") return null
    const saved = this.resume
    if (!saved || saved.sessionId !== session) return null
    if (saved.agent !== (agent === "claude" ? "claudeCode" : agent)) return null
    // Even a recently submitted Enter cannot make a broadcast a new resume record.
    return saved
  }

  /**
   * Esc or Ctrl+C typed in the terminal: an agent at work that does not
   * report its stops stops and waits for the user, without a hook to say so.
   */
  interrupt() {
    if (!this.status || AGENTS_REPORTING_STOPS.has(this.status.agent)) return
    this.cancelWait()
    this.setStatus(null)
  }

  /**
   * Typed in the terminal, not by the terminal itself: an agent waiting for
   * the user has its answer and works on. Its hooks would say so only once
   * the approved tool has finished, which may take minutes.
   */
  answer() {
    const status = this.status
    if (status?.activity !== "waiting" || !AGENTS_REPORTING_STOPS.has(status.agent)) return
    this.cancelWait()
    this.setStatus({ agent: status.agent, activity: "processing" })
  }

  /** The shell's prompt is back or the session closed: no agent runs anymore. */
  reset() {
    this.cancelWait()
    this.releaseSessions()
    this.resume = null
    this.lastReportAt = null
    this.setStatus(null)
    this.setAgent(null)
  }

  dispose() {
    this.cancelWait()
    this.releaseSessions()
  }

  /**
   * Codex's shared background server tells every terminal running Codex
   * about each of its sessions. A session belongs to the terminal Enter was
   * typed in shortly before it was first heard of, and each Enter submits one
   * session; the others hear of it and ignore it.
   */
  private claim(session: string, state: AgentReportState) {
    const owner = sessionOwners.get(session)
    if (owner === this) {
      if (state === "ended") this.releaseSession(session)
      return true
    }
    if (owner || state === "ended") return false
    if (this.submittedAt === null || Date.now() - this.submittedAt > AGENT_CLAIM_WINDOW_MS) {
      return false
    }
    this.submittedAt = null
    sessionOwners.set(session, this)
    this.sessions.add(session)
    return true
  }

  private releaseSession(session: string) {
    if (sessionOwners.get(session) === this) sessionOwners.delete(session)
    this.sessions.delete(session)
  }

  private releaseSessions() {
    for (const session of this.sessions) this.releaseSession(session)
  }

  private cancelWait() {
    if (this.waitTimer === null) return
    clearTimeout(this.waitTimer)
    this.waitTimer = null
  }

  private setAgent(agent: string | null) {
    if (this.agent === agent) return
    this.agent = agent
    this.callbacks.onAgent(agent)
  }

  private setStatus(status: AgentStatus | null) {
    const current = this.status
    if (current?.agent === status?.agent && current?.activity === status?.activity) return
    this.status = status
    this.callbacks.onStatus(status)
  }
}
