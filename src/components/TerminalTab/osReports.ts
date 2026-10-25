import type { IDisposable, Terminal } from "@xterm/xterm"

import { detectDistroId, parseOsReport, type DistroId } from "@/lib/distro"

const OS_REPORT_OSC = 777

interface OsReportOptions {
  /** Over SSH a report would be whatever the host prints, so it is ignored. */
  isRemote: () => boolean
  onDistro: (distroId: DistroId) => void
}

/**
 * Follows the distribution tTerm's WSL launcher reports as it starts the
 * shell, for the tab to show its logo. An SSH host's distribution is read on
 * a channel of its own instead.
 */
export function watchOsReports(
  term: Terminal,
  { isRemote, onDistro }: OsReportOptions
): IDisposable {
  // Returns false like the other OSC 777 handlers, so each one sees every report.
  return term.parser.registerOscHandler(OS_REPORT_OSC, (data) => {
    const info = parseOsReport(data)
    if (info && !isRemote()) onDistro(detectDistroId(info))
    return false
  })
}
