// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { SETTLE_DELAY_MS, watchMouseReporting } from "@/components/TerminalTab/mouseReportingGuard"

const MOUSE_ON = "\x1b[?1003h\x1b[?1006h"

describe("watchMouseReporting", () => {
  let term: Terminal | null = null
  let guard: { dispose(): void } | null = null

  afterEach(() => {
    guard?.dispose()
    term?.dispose()
    guard = null
    term = null
  })

  const create = (isShellInForeground: () => Promise<boolean>) => {
    term = new Terminal({ cols: 40, rows: 5, allowProposedApi: true })
    guard = watchMouseReporting(term, { isShellInForeground })
    return term
  }
  const write = (target: Terminal, data: string) =>
    new Promise<void>((resolve) => target.write(data, resolve))
  /** Waits past the settle delay, then for the guard's own write to be parsed. */
  const settle = async (target: Terminal) => {
    await new Promise((resolve) => setTimeout(resolve, SETTLE_DELAY_MS + 50))
    await write(target, "")
  }

  it("turns reporting off at a shell integration prompt", async () => {
    const target = create(() => Promise.resolve(false))
    await write(target, MOUSE_ON)
    expect(target.modes.mouseTrackingMode).toBe("any")

    await write(target, "\x1b]133;A\x07")
    await write(target, "")
    expect(target.modes.mouseTrackingMode).toBe("none")
  })

  it("turns reporting off once output settles with the shell in the foreground", async () => {
    const target = create(() => Promise.resolve(true))
    await write(target, `${MOUSE_ON}^C\r\n$ `)
    await settle(target)
    expect(target.modes.mouseTrackingMode).toBe("none")
  })

  it("leaves reporting on while a program runs", async () => {
    const isShellInForeground = vi.fn(() => Promise.resolve(false))
    const target = create(isShellInForeground)
    await write(target, `${MOUSE_ON}editor`)
    await settle(target)
    expect(isShellInForeground).toHaveBeenCalledTimes(1)
    expect(target.modes.mouseTrackingMode).toBe("any")
  })

  it("does not ask while reporting is off", async () => {
    const isShellInForeground = vi.fn(() => Promise.resolve(true))
    const target = create(isShellInForeground)
    await write(target, "plain output")
    await settle(target)
    expect(isShellInForeground).not.toHaveBeenCalled()
  })

  it("drops an answer overtaken by more output", async () => {
    let answer: (inForeground: boolean) => void = () => {}
    const target = create(
      () =>
        new Promise<boolean>((resolve) => {
          answer = resolve
        })
    )
    await write(target, MOUSE_ON)
    await new Promise((resolve) => setTimeout(resolve, SETTLE_DELAY_MS + 50))
    // A program starts and asks for reports while the check is out.
    await write(target, MOUSE_ON)
    answer(true)
    await write(target, "")
    expect(target.modes.mouseTrackingMode).toBe("any")
  })
})
