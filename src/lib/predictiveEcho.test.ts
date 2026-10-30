import { beforeEach, describe, expect, it } from "vitest"

import { EchoPredictor, type PredictionScreen } from "@/lib/predictiveEcho"

/** One line of terminal, at row 5, that the tests type on and echo to. */
class FakeLine implements PredictionScreen {
  cursorRow = 5
  cols = 80
  constructor(
    public text = "$ ",
    public cursorCol = text.length
  ) {}

  charAt(row: number, col: number) {
    return (row === this.cursorRow && this.text[col]) || " "
  }

  /** The remote echoes `typed` the way a line editor does. */
  echo(typed: string) {
    for (const char of typed) {
      if (char === "\x7f") {
        this.cursorCol -= 1
        this.text = this.text.slice(0, this.cursorCol)
      } else {
        this.text = this.text.slice(0, this.cursorCol) + char + this.text.slice(this.cursorCol + 1)
        this.cursorCol += 1
      }
    }
  }
}

let clock = 0
let line: FakeLine
let predictor: EchoPredictor

const shown = () => {
  const view = predictor.view()
  return view && { text: view.cells.map((cell) => cell.char).join(""), cursor: view.cursorCol }
}

/** Types `keys` and lets the echo come back `rtt` later. */
function typeAndEcho(keys: string, rtt = 100) {
  predictor.type(keys, line)
  clock += rtt
  line.echo(keys)
  predictor.reconcile(line)
}

beforeEach(() => {
  clock = 0
  line = new FakeLine()
  predictor = new EchoPredictor(() => clock)
})

describe("EchoPredictor", () => {
  it("shows nothing on a line until the remote has echoed a key typed there", () => {
    predictor.type("l", line)
    expect(shown()).toBeNull()

    clock += 100
    line.echo("l")
    predictor.reconcile(line)
    predictor.type("s -", line)
    expect(shown()).toEqual({ text: "s -", cursor: 6 })
  })

  it("drops guesses as their echo arrives, even several at once", () => {
    typeAndEcho("l")
    predictor.type("s -la", line)
    line.echo("s -")
    predictor.reconcile(line)
    expect(shown()).toEqual({ text: "la", cursor: 8 })

    line.echo("la")
    predictor.reconcile(line)
    expect(shown()).toBeNull()
    expect(predictor.busy).toBe(false)
  })

  it("never shows what is typed at a prompt that does not echo", () => {
    line = new FakeLine("Password: ")
    predictor.type("secret", line)
    clock += 5000
    predictor.reconcile(line)
    predictor.expire(line)
    predictor.type("more", line)
    expect(shown()).toBeNull()
  })

  it("starts over on Enter, so the next line must echo again", () => {
    typeAndEcho("ls")
    predictor.type("\r", line)
    line.cursorRow += 1
    line.text = "$ "
    line.cursorCol = 2
    predictor.type("p", line)
    expect(shown()).toBeNull()
  })

  it("predicts backspace over typed characters but not into the prompt", () => {
    typeAndEcho("ab")
    predictor.type("\x7f", line)
    expect(shown()).toEqual({ text: " ", cursor: 3 })

    clock += 100
    line.echo("\x7f")
    predictor.reconcile(line)
    predictor.type("\x7f\x7f", line)
    // The second backspace would erase the prompt, so guessing stops.
    expect(shown()).toBeNull()
    expect(predictor.activeRow).toBeNull()
  })

  it("keeps a key and its backspace that echo as no change at all", () => {
    typeAndEcho("a")
    predictor.type("b\x7f", line)
    line.echo("b\x7f")
    predictor.reconcile(line)
    clock += 3000
    predictor.expire(line)
    expect(predictor.busy).toBe(false)

    predictor.type("c", line)
    expect(shown()).toEqual({ text: "c", cursor: 4 })
  })

  it("does not guess typing in the middle of text", () => {
    line = new FakeLine("$ echo hi", 4)
    typeAndEcho("x")
    expect(predictor.activeRow).toBeNull()
  })

  it("follows a grayed-out suggestion while it is typed", () => {
    line = new FakeLine("$ git status", 3)
    typeAndEcho("i")
    predictor.type("t", line)
    expect(shown()).toEqual({ text: "t", cursor: 5 })
  })

  it("stops at the last column, where the cursor waits to wrap", () => {
    line = new FakeLine("$ ".padEnd(18, "x"))
    line.cols = 20
    typeAndEcho("a")
    predictor.type("b", line)
    expect(shown()).toBeNull()
  })

  it("drops guesses once the line stops matching them", () => {
    typeAndEcho("a")
    predictor.type("b", line)
    line.text = "$ something else"
    line.cursorCol = 16
    predictor.reconcile(line)
    expect(shown()).toBeNull()
    expect(predictor.busy).toBe(true)

    clock += 200
    predictor.expire(line)
    expect(predictor.activeRow).toBeNull()
  })

  it("rides out a redraw caught halfway", () => {
    typeAndEcho("a")
    predictor.type("b", line)
    line.cursorCol = 0
    predictor.reconcile(line)
    line.cursorCol = 3
    line.echo("b")
    predictor.reconcile(line)
    clock += 200
    predictor.expire(line)
    expect(predictor.busy).toBe(false)
    expect(predictor.activeRow).toBe(5)
  })

  it("marks a guess whose echo is overdue", () => {
    typeAndEcho("a")
    predictor.type("b", line)
    clock += 600
    expect(predictor.view()?.cells[0].stalled).toBe(true)
  })

  it("measures the round trip and reports a slow link", () => {
    expect(predictor.slowerThan(30)).toBe(false)
    typeAndEcho("a", 10)
    expect(predictor.slowerThan(30)).toBe(false)
    for (let i = 0; i < 20; i += 1) typeAndEcho("b", 80)
    expect(predictor.roundTripMs).toBeGreaterThan(30)
    expect(predictor.slowerThan(30)).toBe(true)
    expect(predictor.slowerThan(200)).toBe(false)
  })

  it("keeps a link near the threshold slow until it is clearly faster", () => {
    typeAndEcho("a", 120)
    expect(predictor.slowerThan(100)).toBe(true)
    for (let i = 0; i < 5; i += 1) typeAndEcho("b", 75)
    expect(predictor.roundTripMs).toBeLessThan(100)
    expect(predictor.slowerThan(100)).toBe(true)
    for (let i = 0; i < 30; i += 1) typeAndEcho("c", 40)
    expect(predictor.slowerThan(100)).toBe(false)
  })

  it("guesses nothing for keys other than typing and backspace", () => {
    typeAndEcho("a")
    for (const key of ["\t", "\x1b[D", "\x03", "你"]) {
      predictor.type(key, line)
      expect(predictor.activeRow).toBeNull()
    }
  })
})
