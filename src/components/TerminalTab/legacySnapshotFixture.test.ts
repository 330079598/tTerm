// @vitest-environment jsdom
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { createRequire } from "node:module"
import { dirname, resolve } from "node:path"

import { SerializeAddon } from "@xterm/addon-serialize"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { Terminal } from "@xterm/xterm"
import { describe, expect, it } from "vitest"

import { serializeTerminal } from "@/components/TerminalTab/terminalSnapshot"

const require = createRequire(import.meta.url)
// The Tauri version wrote its snapshots with these versions; a change here
// needs a new fixture rather than an edit of the old one.
const PACKAGES = {
  "@xterm/xterm": "6.0.0",
  "@xterm/addon-serialize": "0.14.0",
  "@xterm/addon-unicode11": "0.9.0",
} as const

const COLS = 80
const ROWS = 24
const SCROLLBACK = 10_000
const SCROLLED_OFF = "scrolled-off-0"
const RED = "red"
const ALT_SCREEN_TEXT = "vim-only"
const ASSET_PATH = resolve(process.cwd(), "docs/gpui-migration/reports/assets/P2-19/legacy.json")
const REFRESH_COMMAND =
  "P2_19_REFRESH=1 pnpm exec vitest run src/components/TerminalTab/legacySnapshotFixture.test.ts"

interface LegacyFixture {
  id: string
  description: string
  generatedBy: { command: string; entry: string }
  versions: Record<string, string>
  terminal: { cols: number; rows: number; scrollback: number; unicode: string }
  scrolledOffLine: string
  scrollbackLines: number
  serialized: string
  input: string
  expectedVisible: string[]
  expectedColor: { text: string; foreground: string }
}

function createTerminal() {
  const term = new Terminal({
    cols: COLS,
    rows: ROWS,
    scrollback: SCROLLBACK,
    allowProposedApi: true,
  })
  term.loadAddon(new Unicode11Addon())
  term.unicode.activeVersion = "11"
  const serializer = new SerializeAddon()
  term.loadAddon(serializer)
  return { term, serializer }
}

function write(term: Terminal, data: string) {
  return new Promise<void>((resolveWrite) => term.write(data, resolveWrite))
}

/** Every line the replayed snapshot shows, scrollback included. */
function visibleLines(term: Terminal) {
  const buffer = term.buffer.active
  const lines: string[] = []
  for (let line = 0; line <= buffer.baseY + buffer.cursorY; line++) {
    lines.push(buffer.getLine(line)?.translateToString(true) ?? "")
  }
  return lines
}

/** The foreground of the first cell of `text` in the replayed snapshot. */
function foregroundOf(term: Terminal, text: string) {
  const buffer = term.buffer.active
  for (let line = 0; line <= buffer.baseY + buffer.cursorY; line++) {
    const row = buffer.getLine(line)
    if (!row) continue
    const column = row.translateToString(true).indexOf(text)
    if (column < 0) continue
    const cell = row.getCell(column)
    if (!cell) throw new Error(`Missing fixture cell at ${line}:${column}`)
    if (cell.isFgDefault()) return "default"
    if (cell.isFgRGB()) return `rgb:${cell.getFgColor().toString(16).padStart(6, "0")}`
    return `index:${cell.getFgColor()}`
  }
  return "missing"
}

async function buildFixture(): Promise<LegacyFixture> {
  for (const [name, version] of Object.entries(PACKAGES)) {
    expect(require(`${name}/package.json`).version, `${name} changed`).toBe(version)
  }

  const { term, serializer } = createTerminal()
  try {
    const history = Array.from({ length: 30 }, (_, index) =>
      index === 0 ? SCROLLED_OFF : `line-${index}`
    )
    const writes = [
      `${history.join("\r\n")}\r\n`,
      `\x1b[31m${RED}\x1b[0m 中文\r\n`,
      "$ cargo test\r\n",
      `\x1b[?1049h\x1b[?1000h\x1b[?2004h${ALT_SCREEN_TEXT}\x1b[?1049l`,
      "$ ",
    ]
    for (const data of writes) {
      await write(term, data)
    }

    const serialized = serializeTerminal(term, serializer)
    const replayed = createTerminal()
    let expectedVisible: string[]
    let foreground: string
    try {
      await write(replayed.term, serialized)
      expectedVisible = visibleLines(replayed.term)
      foreground = foregroundOf(replayed.term, RED)
    } finally {
      replayed.term.dispose()
    }

    expect(serialized, "the scrolled-off line is kept").toContain(SCROLLED_OFF)
    expect(serialized, "the SGR color is kept").toContain("\x1b[31m")
    expect(serialized, "the alternate screen is left out").not.toContain(ALT_SCREEN_TEXT)
    expect(serialized).not.toContain("\x1b[?1049h")
    expect(serialized).not.toContain("\x1b[?1000h")
    expect(serialized).not.toContain("\x1b[?2004h")
    expect(expectedVisible).toContain(SCROLLED_OFF)
    expect(expectedVisible).toContain("$ cargo test")
    expect(foreground, "red keeps its palette index").toBe("index:1")
    expect(term.buffer.active.baseY, "the first line scrolled off").toBeGreaterThan(0)

    return {
      id: "p2-19-legacy-snapshot",
      description:
        "Output from the Tauri version's serializeTerminal: scrollback down to the cursor line, SGR colors, and no alternate screen or terminal modes.",
      generatedBy: {
        command: REFRESH_COMMAND,
        entry: "src/components/TerminalTab/terminalSnapshot.ts#serializeTerminal",
      },
      versions: { ...PACKAGES },
      terminal: { cols: COLS, rows: ROWS, scrollback: SCROLLBACK, unicode: "11" },
      scrolledOffLine: SCROLLED_OFF,
      scrollbackLines: term.buffer.active.baseY,
      serialized,
      input: writes.join(""),
      expectedVisible,
      expectedColor: { text: RED, foreground },
    }
  } finally {
    term.dispose()
  }
}

describe("legacy snapshot fixture", () => {
  it("matches the checked-in P2-19 legacy snapshot", async () => {
    const fixture = await buildFixture()
    const text = `${JSON.stringify(fixture, null, 2)}\n`
    if (process.env.P2_19_REFRESH === "1") {
      await mkdir(dirname(ASSET_PATH), { recursive: true })
      await writeFile(ASSET_PATH, text)
      return
    }
    const checkedIn = await readFile(ASSET_PATH, "utf8").catch(() => "")
    expect(checkedIn, `Refresh the fixture with: ${REFRESH_COMMAND}`).toBe(text)
  })
})
