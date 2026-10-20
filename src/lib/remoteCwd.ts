import { useSyncExternalStore } from "react"

// The directory each SSH tab's remote shell last reported (OSC 7), for the
// tab's SFTP drawer to follow. Kept apart from the tabs so a `cd` re-renders
// only the drawer and is not saved with the session.
let cwds = new Map<string, string>()
const listeners = new Set<() => void>()

/** The tab's shell reported `cwd`, or null once its session is gone. */
export function setTabRemoteCwd(tabId: string, cwd: string | null) {
  if (cwd === null) {
    if (!cwds.has(tabId)) return
    cwds = new Map(cwds)
    cwds.delete(tabId)
  } else {
    if (cwds.get(tabId) === cwd) return
    cwds = new Map(cwds).set(tabId, cwd)
  }
  for (const listener of listeners) listener()
}

export function getTabRemoteCwd(tabId: string): string | undefined {
  return cwds.get(tabId)
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useTabRemoteCwd(tabId: string): string | undefined {
  return useSyncExternalStore(subscribe, () => cwds.get(tabId))
}
