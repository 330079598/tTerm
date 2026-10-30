import type { IDisposable, IMarker, Terminal } from "@xterm/xterm"

import { createPredictiveEchoOverlay } from "@/components/TerminalTab/predictiveEchoOverlay"
import { DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS, type AppConfig } from "@/lib/appConfig"
import { EchoPredictor, endsLine, type PredictionScreen } from "@/lib/predictiveEcho"

export type PredictiveEchoMode = AppConfig["predictive_echo"]

/** How often guesses are checked for an overdue echo while any wait for one. */
const TICK_MS = 100

interface PredictiveEchoOptions {
  modeRef?: React.RefObject<PredictiveEchoMode>
  /** In auto mode, guesses show once the round trip is over this. */
  thresholdMsRef?: React.RefObject<number>
  /**
   * What is typed goes to a remote line editor that echoes it: an SSH
   * session with no password prompt at the cursor.
   */
  canPredict: () => boolean
}

/**
 * Shows the echo of what is typed at a remote shell before it comes back,
 * when the link is slow (or always, as set), and takes it away once the real
 * echo arrives or turns out different.
 */
export function attachPredictiveEcho(
  term: Terminal,
  { modeRef, thresholdMsRef, canPredict }: PredictiveEchoOptions
) {
  const predictor = new EchoPredictor()
  const charAt = (row: number, col: number) =>
    term.buffer.active.getLine(row)?.getCell(col)?.getChars() || " "
  const screen: PredictionScreen = {
    get cursorRow() {
      const buffer = term.buffer.active
      return buffer.baseY + buffer.cursorY
    },
    get cursorCol() {
      return term.buffer.active.cursorX
    },
    get cols() {
      return term.cols
    },
    charAt,
  }
  const overlay = createPredictiveEchoOverlay(term, charAt)
  // Full-screen programs and prompts such as Claude Code's hide the cursor
  // and draw their own, so the cursor's cell is not where typing goes.
  let cursorVisible = true
  // Rows count from the top of the scrollback, which drops its oldest lines
  // once full; the marker tells when the row typed on has moved.
  let marker: IMarker | null = null
  let markerRow: number | null = null
  let timer: number | null = null
  // The row typed on while guessing was ruled out, as at a password prompt. A
  // prompt that echoes `*` for each key no longer looks like one after the
  // first, so the row stays ruled out until the line is ended.
  let promptRow: number | null = null

  const mode = () => modeRef?.current ?? "auto"

  const syncMarker = () => {
    if (marker && predictor.activeRow === markerRow && marker.line !== markerRow) predictor.reset()
    const row = predictor.activeRow
    if (row === markerRow && marker && !marker.isDisposed) return
    marker?.dispose()
    marker = row === null ? null : (term.registerMarker(row - screen.cursorRow) ?? null)
    markerRow = marker ? row : null
  }

  const render = () => {
    const shown =
      mode() === "always" ||
      (mode() === "auto" &&
        predictor.slowerThan(thresholdMsRef?.current ?? DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS))
    const view = shown ? predictor.view() : null
    const onRow = view !== null && screen.cursorRow === view.row
    overlay.draw(view, {
      col: screen.cursorCol,
      empty: onRow && charAt(screen.cursorRow, screen.cursorCol) === " ",
    })
  }

  const stopTimer = () => {
    if (timer === null) return
    window.clearInterval(timer)
    timer = null
  }

  const tick = () => {
    predictor.expire(screen)
    syncMarker()
    render()
    if (!predictor.busy) stopTimer()
  }

  const update = () => {
    syncMarker()
    render()
    if (timer === null && predictor.busy) timer = window.setInterval(tick, TICK_MS)
  }

  const reset = () => {
    predictor.reset()
    update()
  }

  const allowed = () =>
    mode() !== "off" &&
    cursorVisible &&
    term.buffer.active.type === "normal" &&
    term.modes.mouseTrackingMode === "none" &&
    canPredict()

  const trackCursorVisibility = (visible: boolean) => (params: (number | number[])[]) => {
    if (params.includes(25)) cursorVisible = visible
    // xterm still applies the mode itself.
    return false
  }

  const disposables: IDisposable[] = [
    term.parser.registerCsiHandler({ prefix: "?", final: "h" }, trackCursorVisibility(true)),
    term.parser.registerCsiHandler({ prefix: "?", final: "l" }, trackCursorVisibility(false)),
    term.onWriteParsed(() => {
      predictor.reconcile(screen)
      update()
    }),
    term.onResize(reset),
    term.buffer.onBufferChange(reset),
    term.onScroll(render),
  ]

  return {
    /** Notes what is typed, before it is sent. */
    noteInput(data: string) {
      if (endsLine(data)) promptRow = null
      else if (!canPredict()) promptRow = screen.cursorRow
      if (!allowed() || screen.cursorRow === promptRow) {
        if (predictor.activeRow !== null) reset()
        return
      }
      predictor.type(data, screen)
      update()
    },
    dispose() {
      stopTimer()
      for (const disposable of disposables) disposable.dispose()
      marker?.dispose()
      overlay.dispose()
    },
  }
}

export type PredictiveEcho = ReturnType<typeof attachPredictiveEcho>
