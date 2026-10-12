import type { LogsTabRequest } from "@/lib/appNavigation"

/** A logged session, as `list_terminal_logs` reports it. */
export interface LogSession {
  id: string
  sessionType: string
  profile: string
  host: string
  port: number
  username: string
  /** The tab the session ran in; empty for plain-only logs from older versions. */
  tabId: string
  startedAtMs: number
  modifiedAtMs: number
  rawBytes: number
  plainBytes: number
  hasRaw: boolean
  hasPlain: boolean
  recording: boolean
}

/** What the list calls a session: its connection, or `localLabel` for a local shell. */
export function logSessionTitle(session: LogSession, localLabel: string): string {
  if (session.sessionType !== "ssh") return localLabel
  return session.profile || session.host || session.id
}

/** Where an SSH session went; nothing for a local shell. */
export function logSessionTarget(session: LogSession): string | null {
  if (session.sessionType !== "ssh") return null
  const host =
    session.port && session.port !== 22 ? `${session.host}:${session.port}` : session.host
  return session.username ? `${session.username}@${host}` : host
}

/**
 * The newest log of a tab: one it wrote since tTerm started, else, for a
 * tab restored from an earlier run, one with its id and connection. Tab ids
 * are reused across runs, so a local tab only matches its own run's logs.
 */
export function findTabLog(
  sessions: readonly LogSession[],
  request: LogsTabRequest,
  loggedIds: readonly string[]
): LogSession | undefined {
  for (let index = loggedIds.length - 1; index >= 0; index--) {
    const logged = sessions.find((session) => session.id === loggedIds[index])
    if (logged) return logged
  }
  if (request.sessionType !== "ssh") return undefined
  return sessions.find(
    (session) =>
      session.tabId === request.tabId &&
      session.sessionType === "ssh" &&
      session.host === request.host &&
      session.port === (request.port ?? 22) &&
      session.username === (request.username ?? "")
  )
}

/** The folders a session's logs are in below the log directory, or "" at the top. */
export function logFolder(id: string): string {
  const slash = id.lastIndexOf("/")
  return slash < 0 ? "" : id.slice(0, slash)
}

/** A session's base file name, without its folders. */
export function logBaseName(id: string): string {
  return id.slice(id.lastIndexOf("/") + 1)
}

/**
 * The ids from `anchor` to `target` in `ids`, both included, for a shift-click;
 * just `target` when the anchor is not in the list.
 */
export function idRange(ids: readonly string[], anchor: string | null, target: string): string[] {
  const from = anchor === null ? -1 : ids.indexOf(anchor)
  const to = ids.indexOf(target)
  if (from < 0 || to < 0) return [target]
  return ids.slice(Math.min(from, to), Math.max(from, to) + 1)
}

export function matchesLogFilter(session: LogSession, filter: string): boolean {
  const query = filter.trim().toLowerCase()
  if (!query) return true
  return [session.profile, session.host, session.username, session.id].some((value) =>
    value.toLowerCase().includes(query)
  )
}
