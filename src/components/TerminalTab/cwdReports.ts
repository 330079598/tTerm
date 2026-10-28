import { listen } from "@tauri-apps/api/event"
import type { IDisposable, Terminal } from "@xterm/xterm"

import {
  CWD_REPORT_OSC_CODES,
  isSameHost,
  isUnnamedHost,
  parseCwdReport,
  parseOsc7Report,
} from "@/lib/terminalCwd"

interface CwdReportOptions {
  tabId: string
  sessionNonce: number
  /** Over SSH the reports come from the remote host and go to `onRemoteCwd`. */
  isRemote: () => boolean
  /** Called with each new local directory, not again for a repeat of the last one. */
  onCwd: (cwd: string) => void
  /**
   * Called with each new directory of the remote shell, and with null once
   * the session is gone.
   */
  onRemoteCwd: (cwd: string | null) => void
  /** The connected host's name, when shell integration read it. */
  remoteHostname: () => string | null
  /**
   * Called with another host's name once the remote reports come from it (an
   * `ssh` from the connected host), and with null once they are back.
   */
  onRemoteAwayHost: (host: string | null) => void
}

/**
 * Follows the shell's working directory. Windows shells report it
 * themselves (OSC 7 / OSC 9;9); on macOS and Linux the backend reads it from
 * the shell process. A remote shell reports it with OSC 7 when it runs
 * tTerm's shell integration; those paths are on the host, so they never
 * reach what `onCwd` does with local ones. A remote report that names
 * another host comes from a shell reached from the connected host, and its
 * paths are not the connected host's either.
 */
export function watchCwdReports(term: Terminal, options: CwdReportOptions): IDisposable {
  const { tabId, sessionNonce, isRemote, onCwd, onRemoteCwd, onRemoteAwayHost } = options
  let lastReportedCwd: string | null = null
  const reportCwd = (cwd: string) => {
    if (cwd === lastReportedCwd) return
    lastReportedCwd = cwd
    onCwd(cwd)
  }
  let lastRemoteCwd: string | null = null
  let awayHost: string | null = null
  const setAwayHost = (host: string | null) => {
    if (host === awayHost) return
    awayHost = host
    onRemoteAwayHost(host)
  }
  // The first host a report named, standing in for the connected host's name
  // when shell integration did not read it.
  let firstNamedHost: string | null = null
  const reportRemote = (data: string) => {
    const report = parseOsc7Report(data)
    if (!report) return
    // tTerm's own scripts leave the host out, and they run on the connected
    // host only.
    if (!isUnnamedHost(report.host)) {
      firstNamedHost ??= report.host
      const hostname = options.remoteHostname() ?? firstNamedHost
      if (!isSameHost(report.host, hostname)) {
        setAwayHost(report.host)
        return
      }
    }
    setAwayHost(null)
    if (report.path === lastRemoteCwd) return
    lastRemoteCwd = report.path
    onRemoteCwd(report.path)
  }
  const oscDisposables = CWD_REPORT_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      if (isRemote()) {
        // OSC 9;9 comes from Windows shells only, which are local.
        if (osc === 7) reportRemote(data)
        return false
      }
      const cwd = parseCwdReport(osc, data)
      if (cwd) reportCwd(cwd)
      return false
    })
  )
  const unlisten = listen<{ sessionNonce: number; cwd: string }>(`pty-cwd-${tabId}`, (event) => {
    if (event.payload.sessionNonce === sessionNonce) reportCwd(event.payload.cwd)
  })

  return {
    dispose() {
      for (const disposable of oscDisposables) disposable.dispose()
      void unlisten.then((stop) => stop())
      if (lastRemoteCwd !== null) onRemoteCwd(null)
      setAwayHost(null)
    },
  }
}
