import { describe, expect, it } from "vitest"

import {
  continueRun,
  finishLine,
  isRunActive,
  promptReached,
  retryLine,
  runLines,
  startRun,
  stopRun,
} from "@/lib/composerRun"

const statuses = (run: ReturnType<typeof startRun>) => run.lines.map((line) => line.status)

describe("runLines", () => {
  it("keeps each line as typed and drops the blank ones", () => {
    expect(runLines("cd /tmp\r\n\n  \n  ls -la\n")).toEqual(["cd /tmp", "  ls -la"])
  })
})

describe("composer runs", () => {
  it("runs the lines in turn and ends after the last", () => {
    let run = startRun(["a", "b"])
    expect(run.phase).toBe("running")
    expect(statuses(run)).toEqual(["running", "pending"])

    run = finishLine(run, 0)
    expect(run.current).toBe(1)
    expect(statuses(run)).toEqual(["succeeded", "running"])

    run = finishLine(run, undefined)
    expect(run.phase).toBe("done")
    expect(statuses(run)).toEqual(["succeeded", "succeeded"])
    expect(isRunActive(run)).toBe(false)
  })

  it("holds the rest when a line fails", () => {
    const run = finishLine(startRun(["a", "b"]), 2)
    expect(run.phase).toBe("failed")
    expect(run.current).toBe(0)
    expect(run.lines[0]).toEqual({ text: "a", status: "failed", exitCode: 2 })
    expect(isRunActive(run)).toBe(true)
    // A prompt that comes while the run waits on the user changes nothing.
    expect(finishLine(run, 0)).toBe(run)
  })

  it("runs a failed line again on retry", () => {
    const run = retryLine(finishLine(startRun(["a", "b"]), 1))
    expect(run.phase).toBe("running")
    expect(run.current).toBe(0)
    expect(run.lines[0]).toEqual({ text: "a", status: "running" })
  })

  it("goes on past a failed line, which stays failed", () => {
    const run = continueRun(finishLine(startRun(["a", "b"]), 1))
    expect(run.current).toBe(1)
    expect(statuses(run)).toEqual(["failed", "running"])
  })

  it("ends when the failed line was the last", () => {
    const run = continueRun(finishLine(startRun(["a"]), 1))
    expect(run.phase).toBe("done")
  })

  it("waits past a prompt at which the shell ran nothing", () => {
    // Enter pressed in the terminal while "a" ran gives a prompt before "b" is read.
    let run = promptReached(startRun(["a", "b", "c"]), {
      kind: "prompt",
      exitCode: 0,
      startedCommand: true,
      status: 0,
    })
    run = promptReached(run, { kind: "prompt", startedCommand: false, status: 0 })
    expect(statuses(run)).toEqual(["succeeded", "running", "pending"])

    run = promptReached(run, { kind: "prompt", exitCode: 0, startedCommand: true, status: 0 })
    expect(statuses(run)).toEqual(["succeeded", "succeeded", "running"])
  })

  it("ends a comment, and fails a line the shell can't parse, with nothing run", () => {
    let run = promptReached(startRun(["# note", "fi", "c"]), {
      kind: "prompt",
      startedCommand: false,
    })
    expect(statuses(run)).toEqual(["succeeded", "running", "pending"])

    run = promptReached(run, { kind: "prompt", startedCommand: false, status: 2 })
    expect(run.phase).toBe("failed")
    expect(run.lines[1].exitCode).toBe(2)
  })

  it("takes every prompt as a line's end in a shell that marks no output", () => {
    const run = promptReached(startRun(["a", "b"]), { kind: "prompt" })
    expect(statuses(run)).toEqual(["succeeded", "running"])
    expect(promptReached(run, { kind: "ended" }).phase).toBe("stopped")
  })

  it("stops with the running line left unfinished", () => {
    const run = stopRun(startRun(["a", "b"]))
    expect(run.phase).toBe("stopped")
    expect(statuses(run)).toEqual(["unfinished", "pending"])
    expect(stopRun(run)).toBe(run)
  })
})
