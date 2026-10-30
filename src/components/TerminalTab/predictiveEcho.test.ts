// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import {
  attachPredictiveEcho,
  type PredictiveEchoMode,
} from "@/components/TerminalTab/predictiveEcho"

let term: Terminal | null = null
let echo: ReturnType<typeof attachPredictiveEcho> | null = null

beforeAll(() => {
  window.matchMedia ||= () =>
    ({ matches: false, addListener() {}, removeListener() {} }) as unknown as MediaQueryList
  window.ResizeObserver ||= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

function teardown() {
  echo?.dispose()
  term?.dispose()
  echo = null
  term = null
}

afterEach(teardown)

async function setup(mode: PredictiveEchoMode = "always", canPredict = () => true) {
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 40, rows: 5, allowProposedApi: true })
  term.open(container)
  echo = attachPredictiveEcho(term, { modeRef: { current: mode }, canPredict })
  await write("$ ")
  return { term, echo }
}

function write(data: string) {
  return new Promise<void>((resolve) => term!.write(data, resolve))
}

/** The predicted characters drawn over the terminal, the cursor's included. */
function drawn() {
  const layer = term!.element!.querySelector(".terminal-predictive-echo")
  return [...(layer?.children ?? [])].map((node) => node.textContent).join("")
}

describe("attachPredictiveEcho", () => {
  it("draws typing once the line has echoed, and takes it away with the echo", async () => {
    const { echo } = await setup()
    echo.noteInput("l")
    expect(drawn()).toBe("")

    await write("l")
    echo.noteInput("s")
    expect(drawn()).toBe("s ")

    await write("s")
    expect(drawn()).toBe("")
  })

  it("leaves programs that hide the cursor alone", async () => {
    const { echo } = await setup()
    echo.noteInput("l")
    await write("l\x1b[?25l")
    echo.noteInput("s")
    expect(drawn()).toBe("")
  })

  it("leaves full-screen programs alone", async () => {
    const { echo } = await setup()
    await write("\x1b[?1049h$ ")
    echo.noteInput("l")
    await write("l")
    echo.noteInput("s")
    expect(drawn()).toBe("")
  })

  it("draws nothing when off, on a fast link in auto mode, or when typing may not echo", async () => {
    for (const [mode, canPredict] of [
      ["off", true],
      ["auto", true],
      ["always", false],
    ] as const) {
      const predictive = (await setup(mode, () => canPredict)).echo
      predictive.noteInput("l")
      await write("l")
      predictive.noteInput("s")
      expect(drawn(), mode).toBe("")
      teardown()
    }
  })
})
