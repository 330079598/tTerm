// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest"
import { SerializeAddon } from "@xterm/addon-serialize"
import { Terminal } from "@xterm/xterm"

import {
  restoredContentText,
  serializeTerminal,
  SNAPSHOT_MAX_LINES,
} from "@/components/TerminalTab/terminalSnapshot"

let term: Terminal | null = null

afterEach(() => {
  term?.dispose()
  term = null
})

function setup(rows = 24) {
  term = new Terminal({ cols: 80, rows, scrollback: 10_000, allowProposedApi: true })
  const serializer = new SerializeAddon()
  term.loadAddon(serializer)
  const write = (data: string) => new Promise<void>((resolve) => term!.write(data, resolve))
  return { term, serializer, write }
}

/** Writes `text` into a fresh terminal and reads back what it shows. */
async function restoreInto(text: string): Promise<string[]> {
  const other = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  await new Promise<void>((resolve) => other.write(text, resolve))
  const buffer = other.buffer.active
  const lines: string[] = []
  for (let y = 0; y <= buffer.baseY + buffer.cursorY; y++) {
    lines.push(buffer.getLine(y)?.translateToString(true) ?? "")
  }
  other.dispose()
  return lines
}

describe("serializeTerminal", () => {
  it("keeps the output and its colors down to the cursor's line", async () => {
    const { term, serializer, write } = setup()
    await write("\x1b[31mred\x1b[0m\r\n$ ")
    const content = serializeTerminal(term, serializer)
    expect(content).toContain("red")
    expect(content).toContain("\x1b[31m")
    expect(await restoreInto(content)).toEqual(["red", "$ "])
  })

  it("leaves out what a program drew below the cursor", async () => {
    const { term, serializer, write } = setup()
    await write("$ claude\r\n\x1b[10;1Hstatus bar\x1b[2;1H")
    expect(serializeTerminal(term, serializer)).not.toContain("status bar")
  })

  it("leaves out the terminal's modes and a full-screen program's screen", async () => {
    const { term, serializer, write } = setup()
    await write("$ vim\r\n\x1b[?1049h\x1b[?1000h\x1b[?2004hin vim")
    const content = serializeTerminal(term, serializer)
    expect(content).toContain("$ vim")
    expect(content).not.toContain("in vim")
    expect(content).not.toContain("\x1b[?1049h")
    expect(content).not.toContain("\x1b[?1000h")
    expect(content).not.toContain("\x1b[?2004h")
  })

  it(`keeps at most ${SNAPSHOT_MAX_LINES} lines`, async () => {
    const { term, serializer, write } = setup()
    const lines = Array.from({ length: SNAPSHOT_MAX_LINES + 100 }, (_, index) => `line ${index}`)
    await write(lines.join("\r\n"))
    const restored = (await restoreInto(serializeTerminal(term, serializer))).filter(Boolean)
    expect(restored.length).toBeLessThanOrEqual(SNAPSHOT_MAX_LINES)
    expect(restored[restored.length - 1]).toBe(`line ${SNAPSHOT_MAX_LINES + 99}`)
    expect(restored).not.toContain("line 99")
  })
})

describe("restoredContentText", () => {
  it("closes the content with the banner on a line of its own", async () => {
    const lines = await restoreInto(restoredContentText("\x1b[1m$ ls", "Restored"))
    expect(lines).toEqual(["$ ls", "── Restored ──", ""])
  })

  it("is empty when the terminal had shown nothing", () => {
    expect(restoredContentText("  \r\n ", "Restored")).toBe("")
  })
})
