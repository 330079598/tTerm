import { listen } from "@tauri-apps/api/event"
import type { IDisposable, Terminal } from "@xterm/xterm"

import { CWD_REPORT_OSC_CODES, parseCwdReport } from "@/lib/terminalCwd"

interface CwdReportOptions {
  tabId: string
  sessionNonce: number
  /** Over SSH the reports come from the remote host, so they are ignored. */
  isRemote: () => boolean
  /** Called with each new directory, not again for a repeat of the last one. */
  onCwd: (cwd: string) => void
}

/**
 * Follows a local shell's working directory. Windows shells report it
 * themselves (OSC 7 / OSC 9;9); on macOS and Linux the backend reads it from
 * the shell process.
 */
export function watchCwdReports(
  term: Terminal,
  { tabId, sessionNonce, isRemote, onCwd }: CwdReportOptions
): IDisposable {
  let lastReportedCwd: string | null = null
  const reportCwd = (cwd: string) => {
    if (cwd === lastReportedCwd) return
    lastReportedCwd = cwd
    onCwd(cwd)
  }
  const oscDisposables = CWD_REPORT_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      if (isRemote()) return false
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
    },
  }
}
