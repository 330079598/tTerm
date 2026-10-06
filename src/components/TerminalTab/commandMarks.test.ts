// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import {
  CommandMarks,
  COMMAND_MARK_OSC_CODES,
  joinBufferLines,
  parseCommandMark,
} from "@/components/TerminalTab/commandMarks"

const PROMPT = "\x1b]133;A\x1b\\"
const OUTPUT = "\x1b]133;C\x1b\\"
const done = (code?: number) => `\x1b]133;D${code === undefined ? "" : `;${code}`}\x1b\\`

let term: Terminal | null = null

/**
 * The terminal as CommandMarks sees it, scrolling for real: jsdom never moves
 * xterm's viewport. Unscrolled, the view follows the output at the bottom.
 */
function withScrolling(real: Terminal): Terminal {
  let viewportY: number | null = null
  const view = Object.create(real) as Terminal
  const buffer = {
    get active() {
      return new Proxy(real.buffer.active, {
        get: (target, key) =>
          key === "viewportY" ? (viewportY ?? target.baseY) : Reflect.get(target, key),
      })
    },
  }
  Object.defineProperty(view, "buffer", { get: () => buffer })
  view.scrollToLine = (line) => {
    viewportY = Math.max(0, Math.min(line, real.buffer.active.baseY))
  }
  view.scrollToBottom = () => {
    viewportY = null
  }
  return view
}

function setup(rows = 24) {
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 40, rows, allowProposedApi: true })
  term.open(container)
  const view = withScrolling(term)
  const marks = new CommandMarks(view)
  for (const osc of COMMAND_MARK_OSC_CODES) {
    term.parser.registerOscHandler(osc, (data) => {
      marks.handleMark(data)
      return false
    })
  }
  const write = (data: string) => new Promise<void>((resolve) => term!.write(data, resolve))
  return { term: view, marks, write }
}

beforeAll(() => {
  window.matchMedia ||= () =>
    ({ matches: false, addListener() {}, removeListener() {} }) as unknown as MediaQueryList
  window.ResizeObserver ||= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterEach(() => {
  term?.dispose()
  term = null
  document.body.replaceChildren()
})

describe("parseCommandMark", () => {
  it("reads prompt, output and done marks", () => {
    expect(parseCommandMark("A")).toEqual({ kind: "prompt" })
    expect(parseCommandMark("A;cl=m;aid=1")).toEqual({ kind: "prompt" })
    expect(parseCommandMark("C")).toEqual({ kind: "output" })
    expect(parseCommandMark("D;0")).toEqual({ kind: "done", exitCode: 0 })
    expect(parseCommandMark("D;127")).toEqual({ kind: "done", exitCode: 127 })
    expect(parseCommandMark("D")).toEqual({ kind: "done", exitCode: undefined })
    expect(parseCommandMark("D;x")).toEqual({ kind: "done", exitCode: undefined })
  })

  it("ignores other marks", () => {
    expect(parseCommandMark("B")).toBeNull()
    expect(parseCommandMark("E;ls -la")).toBeNull()
    expect(parseCommandMark("")).toBeNull()
  })
})

describe("joinBufferLines", () => {
  it("runs wrapped rows on and drops trailing blank lines", () => {
    const line = (text: string, isWrapped = false) => ({
      isWrapped,
      translateToString: () => text,
    })
    expect(joinBufferLines([line("abc"), line("def", true), line("x"), line(""), line("")])).toBe(
      "abcdef\nx"
    )
  })
})

describe("CommandMarks", () => {
  it("copies the output between the C and D marks (bash, zsh, fish)", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}$ ls\r\n${OUTPUT}a.txt\r\nb.txt\r\n${done(0)}${PROMPT}$ `)
    expect(marks.lastOutputText()).toBe("a.txt\nb.txt")
  })

  it("keeps output that ends without a line break", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}$ printf hi\r\n${OUTPUT}hi${done(0)}\r\n${PROMPT}$ `)
    expect(marks.lastOutputText()).toBe("hi")
  })

  it("starts the output after the command line on Enter when the shell sends no C (PowerShell)", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}PS> dir`)
    marks.handleEnter()
    await write(`\r\nout 1\r\nout 2\r\n${done(1)}${PROMPT}PS> `)
    expect(marks.lastOutputText()).toBe("out 1\nout 2")
  })

  it("ignores Enter on an empty line", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}$ ls\r\n${OUTPUT}a.txt\r\n${done(0)}${PROMPT}$ `)
    marks.handleEnter()
    // bash: no C for an empty line, D repeats the last status.
    await write(`\r\n${done(0)}${PROMPT}$ `)
    expect(marks.lastOutputText()).toBe("a.txt")
  })

  it("knows no output for prompts without a D status (cmd)", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}C:\\> dir`)
    marks.handleEnter()
    await write(`\r\nfiles\r\n\r\n${PROMPT}C:\\> `)
    expect(marks.lastOutputRange()).toBeNull()
  })

  it("reports an empty output for a command that printed nothing", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}$ true\r\n${OUTPUT}${done(0)}${PROMPT}$ `)
    expect(marks.lastOutputText()).toBe("")
    expect(marks.selectLastOutput()).toBe(false)
  })

  it("knows when the shell waits at a prompt", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}$ cat big.bin\r\n${OUTPUT}`)
    expect(marks.isAtPrompt()).toBe(false)
    await write(`junk\r\n${done(0)}${PROMPT}$ `)
    expect(marks.isAtPrompt()).toBe(true)
    // Typing a command keeps it at the prompt until it runs.
    await write("vim")
    expect(marks.isAtPrompt()).toBe(true)
    await write(`\r\n${OUTPUT}\x1b[?1049h`)
    expect(marks.isAtPrompt()).toBe(false)
  })

  it("knows the prompt from Enter when the shell sends no C (PowerShell)", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}PS> dir\r\nout\r\n${done(0)}${PROMPT}PS> python`)
    expect(marks.isAtPrompt()).toBe(true)
    marks.handleEnter()
    await write("\r\n>>> ")
    expect(marks.isAtPrompt()).toBe(false)
  })

  it("does not claim a prompt for shells without D marks (cmd)", async () => {
    const { marks, write } = setup()
    await write(`${PROMPT}C:\\> dir`)
    marks.handleEnter()
    await write(`\r\nfiles\r\n\r\n${PROMPT}C:\\> `)
    expect(marks.isAtPrompt()).toBe(false)
  })

  it("does not claim a prompt before any mark or when turned off", async () => {
    const { marks, write } = setup()
    expect(marks.isAtPrompt()).toBe(false)
    await write(`${PROMPT}$ ls\r\n${OUTPUT}a\r\n${done(0)}${PROMPT}$ `)
    marks.setEnabled(false)
    expect(marks.isAtPrompt()).toBe(false)
  })

  it("forgets everything when turned off", async () => {
    const { marks, write } = setup()
    marks.setEnabled(false)
    await write(`${PROMPT}$ ls\r\n${OUTPUT}a\r\n${done(0)}${PROMPT}$ `)
    expect(marks.lastOutputRange()).toBeNull()
    expect(marks.scrollToPreviousPrompt()).toBe(false)
  })

  it("jumps between prompts and back to the bottom", async () => {
    const { term, marks, write } = setup(5)
    // Prompts on lines 0, 6 and 12; the current prompt on line 18.
    for (let command = 0; command < 3; command += 1) {
      await write(`${PROMPT}$ cmd${command}\r\n${OUTPUT}`)
      await write(Array.from({ length: 5 }, (_, line) => `out ${command}.${line}\r\n`).join(""))
      await write(done(0))
    }
    await write(`${PROMPT}$ `)
    const buffer = term.buffer.active
    expect(buffer.viewportY).toBe(buffer.baseY)

    expect(marks.scrollToPreviousPrompt()).toBe(true)
    expect(buffer.viewportY).toBe(12)
    marks.scrollToPreviousPrompt()
    expect(buffer.viewportY).toBe(6)
    marks.scrollToPreviousPrompt()
    expect(buffer.viewportY).toBe(0)
    marks.scrollToPreviousPrompt()
    expect(buffer.viewportY).toBe(0)

    marks.scrollToNextPrompt()
    expect(buffer.viewportY).toBe(6)
    marks.scrollToNextPrompt()
    expect(buffer.viewportY).toBe(12)
    marks.scrollToNextPrompt()
    expect(buffer.viewportY).toBe(buffer.baseY)
  })

  it("keeps going up past prompts that cannot scroll to the top", async () => {
    const { term, marks, write } = setup(10)
    // Prompts on lines 0, 3, 6 and 9 with 10 rows: only line 0 can reach the top.
    for (let command = 0; command < 3; command += 1) {
      await write(`${PROMPT}$ cmd${command}\r\n${OUTPUT}out\r\n\r\n${done(0)}`)
    }
    await write(`${PROMPT}$ \r\n\r\n\r\n`)
    const buffer = term.buffer.active
    expect(buffer.baseY).toBeGreaterThan(0)

    marks.scrollToPreviousPrompt()
    marks.scrollToPreviousPrompt()
    marks.scrollToPreviousPrompt()
    expect(buffer.viewportY).toBe(0)
  })
})
