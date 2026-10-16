// A program that turns mouse reporting on and is killed before turning it off
// (Ctrl+C while it starts, a crash) leaves the terminal sending every mouse
// move to the shell, whose line editor shows the reports as text such as
// `35;74;19M35;73;18M`. Shells never ask for mouse reports, so once the shell
// is back at its prompt the guard turns reporting off.
import type { IDisposable, Terminal } from "@xterm/xterm"

import { COMMAND_MARK_OSC_CODES, parseCommandMark } from "@/components/TerminalTab/commandMarks"
import { MOUSE_REPORTING_OFF } from "@/components/TerminalTab/terminalReset"

/** Output has gone quiet this long: a program that exits leaves the shell redrawing its prompt. */
export const SETTLE_DELAY_MS = 200

interface MouseReportingGuardOptions {
  /**
   * Whether no program is running in the terminal, asked once output settles
   * while mouse reporting is on. Resolves false when it cannot be told (SSH).
   */
  isShellInForeground: () => Promise<boolean>
}

/**
 * Turns mouse reporting off when a shell prompt appears with it still on:
 * at once on a shell integration prompt mark (OSC 133 / 633 A), or, for
 * shells without marks, once output settles and the shell is found to be in
 * the foreground again.
 */
export function watchMouseReporting(
  term: Terminal,
  { isShellInForeground }: MouseReportingGuardOptions
): IDisposable {
  const reportingOn = () => term.modes.mouseTrackingMode !== "none"
  const turnOff = () => term.write(MOUSE_REPORTING_OFF)

  const oscDisposables = COMMAND_MARK_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      if (parseCommandMark(data)?.kind === "prompt" && reportingOn()) turnOff()
      return false
    })
  )

  let timer: ReturnType<typeof setTimeout> | null = null
  // Counts output parsed, so an answer that arrives after more output is
  // dropped: a program started meanwhile may have just asked for reports.
  let outputSeen = 0
  let disposed = false

  const check = async () => {
    timer = null
    if (!reportingOn()) return
    const seenBefore = outputSeen
    let inForeground: boolean
    try {
      inForeground = await isShellInForeground()
    } catch (error) {
      console.error("Failed to check the shell's foreground state:", error)
      return
    }
    if (!disposed && inForeground && outputSeen === seenBefore && reportingOn()) turnOff()
  }

  const writeDisposable = term.onWriteParsed(() => {
    outputSeen += 1
    if (!reportingOn()) return
    if (timer !== null) clearTimeout(timer)
    timer = setTimeout(() => void check(), SETTLE_DELAY_MS)
  })

  return {
    dispose() {
      disposed = true
      if (timer !== null) clearTimeout(timer)
      writeDisposable.dispose()
      for (const disposable of oscDisposables) disposable.dispose()
    },
  }
}
