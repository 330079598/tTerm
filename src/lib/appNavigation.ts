// Requests to open an app page from places that do not own the tabs, such
// as a settings panel. The app shell handles them.

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
