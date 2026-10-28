import { useSyncExternalStore } from "react"

// The directory each SSH tab's remote shell last reported (OSC 7), for the
// tab's SFTP drawer to follow. Kept apart from the tabs so a `cd` re-renders
// only the drawer and is not saved with the session.
let cwds = new Map<string, string>()
// The other host a tab's terminal reports from, after an `ssh` from the
// connected one, while its directories are not the connected host's.
let awayHosts = new Map<string, string>()
// The connected host's name, as shell integration read it; not rendered.
const hostnames = new Map<string, string>()
const listeners = new Set<() => void>()

function notify() {
  for (const listener of listeners) listener()
}

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
  notify()
}

export function getTabRemoteCwd(tabId: string): string | undefined {
  return cwds.get(tabId)
}

/**
 * The tab's terminal reports from `host`, not the host it connected to, or
 * null once it is back there or its session is gone.
 */
export function setTabRemoteAwayHost(tabId: string, host: string | null) {
  if (host === null) {
    if (!awayHosts.has(tabId)) return
    awayHosts = new Map(awayHosts)
    awayHosts.delete(tabId)
  } else {
    if (awayHosts.get(tabId) === host) return
    awayHosts = new Map(awayHosts).set(tabId, host)
  }
  notify()
}

export function getTabRemoteAwayHost(tabId: string): string | undefined {
  return awayHosts.get(tabId)
}

/** The name of the host the tab connected to, or null when it is not known. */
export function setTabRemoteHostname(tabId: string, hostname: string | null) {
  if (hostname === null) hostnames.delete(tabId)
  else hostnames.set(tabId, hostname)
}

export function getTabRemoteHostname(tabId: string): string | undefined {
  return hostnames.get(tabId)
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

export function useTabRemoteAwayHost(tabId: string): string | undefined {
  return useSyncExternalStore(subscribe, () => awayHosts.get(tabId))
}
