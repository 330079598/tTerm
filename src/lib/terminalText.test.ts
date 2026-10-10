import { describe, expect, it } from "vitest"
import type { IBuffer, IBufferLine } from "@xterm/xterm"

import { terminalBufferText, terminalTextFileName } from "@/lib/terminalText"

function buffer(rows: Array<{ text: string; wrapped?: boolean }>): IBuffer {
  return {
    length: rows.length,
    getLine: (index: number) => {
      const row = rows[index]
      if (!row) return undefined
      return {
        isWrapped: row.wrapped === true,
        translateToString: (trimRight?: boolean) => (trimRight ? row.text.trimEnd() : row.text),
      } as unknown as IBufferLine
    },
  } as unknown as IBuffer
}

describe("terminalBufferText", () => {
  it("joins wrapped rows and keeps blanks at the wrap", () => {
    const text = terminalBufferText(
      buffer([
        { text: "$ echo hello  " },
        { text: "abc " },
        { text: "def   ", wrapped: true },
        { text: "" },
        { text: "   " },
      ])
    )
    expect(text).toBe("$ echo hello\nabc def\n")
  })

  it("is empty for an empty buffer", () => {
    expect(terminalBufferText(buffer([{ text: "  " }]))).toBe("")
  })
})

describe("terminalTextFileName", () => {
  it("names the file after the tab and the time", () => {
    expect(terminalTextFileName("prod/api: 1", new Date(2026, 9, 10, 9, 5, 7))).toBe(
      "prod_api_ 1-20261010-090507.txt"
    )
    expect(terminalTextFileName("  ", new Date(2026, 0, 2, 3, 4, 5))).toBe(
      "terminal-20260102-030405.txt"
    )
  })
})
