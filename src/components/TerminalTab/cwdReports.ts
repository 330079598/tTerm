import { listen } from "@tauri-apps/api/event"
import type { IDisposable, Terminal } from "@xterm/xterm"

import { CWD_REPORT_OSC_CODES, parseCwdReport } from "@/lib/terminalCwd"

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
}

/**
 * Follows the shell's working directory. Windows shells report it
 * themselves (OSC 7 / OSC 9;9); on macOS and Linux the backend reads it from
 * the shell process. A remote shell reports it with OSC 7 when it runs
 * tTerm's shell integration; those paths are on the host, so they never
 * reach what `onCwd` does with local ones.
 */
export function watchCwdReports(
  term: Terminal,
  { tabId, sessionNonce, isRemote, onCwd, onRemoteCwd }: CwdReportOptions
): IDisposable {
  let lastReportedCwd: string | null = null
  const reportCwd = (cwd: string) => {
    if (cwd === lastReportedCwd) return
    lastReportedCwd = cwd
    onCwd(cwd)
  }
  let lastRemoteCwd: string | null = null
  const oscDisposables = CWD_REPORT_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      const cwd = parseCwdReport(osc, data)
      if (!cwd) return false
      if (!isRemote()) {
        reportCwd(cwd)
      } else if (osc === 7 && cwd !== lastRemoteCwd) {
        // OSC 9;9 comes from Windows shells only, which are local.
        lastRemoteCwd = cwd
        onRemoteCwd(cwd)
      }
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
    },
  }
}
