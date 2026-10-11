import type { Tab } from "@/types/tab"

function connectionHostLabel(tab: Tab): string | undefined {
  const host = tab.remoteFile?.host?.trim() || tab.connection?.host?.trim()
  if (!host) {
    return undefined
  }

  const port = tab.connection?.port
  return port && port !== 22 ? `${host}:${port}` : host
}

/**
 * What the overflow menu shows under a tab's title: the host it reaches, or
 * for a remote file the connection it was opened through and that host.
 */
export function getOverflowConnectionMeta(tab: Tab): { primary?: string; secondary?: string } {
  const hostLabel = connectionHostLabel(tab)

  if (tab.type !== "remote-file-editor") {
    return { primary: hostLabel }
  }

  const savedName =
    tab.remoteFile?.profileName?.trim() ||
    tab.connection?.profileName?.trim() ||
    tab.remoteFile?.connectionLabel?.trim()

  if (!savedName) {
    return { primary: hostLabel }
  }

  return {
    primary: savedName,
    secondary: hostLabel && hostLabel !== savedName ? hostLabel : undefined,
  }
}
