// Requests to open an app page from places that do not own the tabs, such
// as a settings panel or a tab menu. The app shell opens the page; the page
// picks up what it should show.

export type AppPage = "terminal-logs"

let handler: ((page: AppPage) => void) | null = null

/** Handles page requests; returns a function that stops handling them. */
export function handleAppPageRequests(next: (page: AppPage) => void): () => void {
  handler = next
  return () => {
    if (handler === next) handler = null
  }
}

export function requestAppPage(page: AppPage) {
  handler?.(page)
}

/** A tab whose logs the logs page should show, and what it connects to. */
export interface LogsTabRequest {
  tabId: string
  sessionType: "ssh" | "local"
  host?: string
  port?: number
  username?: string
}

let logsTabRequest: LogsTabRequest | null = null
const logsTabListeners = new Set<() => void>()

/** Opens the logs page on the newest session logged in a tab. */
export function showLogsForTab(request: LogsTabRequest) {
  logsTabRequest = request
  for (const listener of logsTabListeners) listener()
  requestAppPage("terminal-logs")
}

/** The tab asked for since the last call, once. */
export function takeLogsTabRequest(): LogsTabRequest | null {
  const request = logsTabRequest
  logsTabRequest = null
  return request
}

export function onLogsTabRequest(listener: () => void): () => void {
  logsTabListeners.add(listener)
  return () => {
    logsTabListeners.delete(listener)
  }
}
