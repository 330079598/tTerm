// Running the compose bar's lines one at a time: a line goes to the shell
// only once the one before it has ended and the shell is back at its prompt,
// and a failed line holds the rest until the user says what to do.

/** `unfinished`: sent, but the run was stopped before it ended. */
export type RunLineStatus = "pending" | "running" | "succeeded" | "failed" | "unfinished"

export interface RunLine {
  text: string
  status: RunLineStatus
  exitCode?: number
}

export interface ComposerRun {
  lines: RunLine[]
  /**
   * `running`: a line runs; `failed`: a line failed and the rest wait;
   * `done`: every line ran; `stopped`: the user or the session ended it.
   */
  phase: "running" | "failed" | "done" | "stopped"
  /** The line that runs, or the one that failed. */
  current: number
}

/** The commands in `text`: its lines, less the blank ones, which would run nothing. */
export function runLines(text: string): string[] {
  return text.split(/\r\n?|\n/).filter((line) => line.trim() !== "")
}

function withLine(run: ComposerRun, index: number, line: Partial<RunLine>): RunLine[] {
  return run.lines.map((current, at) => (at === index ? { ...current, ...line } : current))
}

/** Runs line `index` next, or ends the run when there is none. */
function runFrom(lines: RunLine[], index: number): ComposerRun {
  if (index >= lines.length) return { lines, phase: "done", current: lines.length - 1 }
  return {
    lines: lines.map((line, at) => (at === index ? { text: line.text, status: "running" } : line)),
    phase: "running",
    current: index,
  }
}

export function startRun(lines: string[]): ComposerRun {
  const pending = lines.map((text): RunLine => ({ text, status: "pending" }))
  return runFrom(pending, 0)
}

/**
 * The running line ended with `exitCode`, which the shell may not tell (a
 * comment runs nothing); only a non-zero status counts as failing.
 */
export function finishLine(run: ComposerRun, exitCode: number | undefined): ComposerRun {
  if (run.phase !== "running") return run
  const failed = exitCode !== undefined && exitCode !== 0
  const lines = withLine(run, run.current, {
    status: failed ? "failed" : "succeeded",
    exitCode,
  })
  if (failed) return { ...run, lines, phase: "failed" }
  return runFrom(lines, run.current + 1)
}

/** Runs the failed line again. */
export function retryLine(run: ComposerRun): ComposerRun {
  if (run.phase !== "failed") return run
  return runFrom(run.lines, run.current)
}

/** Leaves the failed line as it is and goes on with the next one. */
export function continueRun(run: ComposerRun): ComposerRun {
  if (run.phase !== "failed") return run
  return runFrom(run.lines, run.current + 1)
}

/** Sends no more lines; one still running carries on in the shell. */
export function stopRun(run: ComposerRun): ComposerRun {
  if (run.phase !== "running" && run.phase !== "failed") return run
  const lines =
    run.phase === "running" ? withLine(run, run.current, { status: "unfinished" }) : run.lines
  return { ...run, lines, phase: "stopped" }
}

/** The run holds the bar: lines still wait to go. */
export function isRunActive(run: ComposerRun | undefined) {
  return run?.phase === "running" || run?.phase === "failed"
}
