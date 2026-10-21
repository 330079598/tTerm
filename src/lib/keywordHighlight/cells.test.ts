// @vitest-environment jsdom
import { Terminal } from "@xterm/xterm"
import { describe, expect, it } from "vitest"

import { highlightSegments, readCellText } from "@/lib/keywordHighlight/cells"

const write = (term: Terminal, data: string) =>
  new Promise<void>((resolve) => term.write(data, resolve))

function read(term: Terminal, startRow: number, endRow: number, maxLength = 2000) {
  return readCellText(
    term.buffer.active,
    startRow,
    endRow,
    maxLength,
    term.buffer.active.getNullCell()
  )
}

describe("readCellText", () => {
  it("reads a wrapped line as one text and drops trailing blanks", async () => {
    const term = new Terminal({ cols: 10, rows: 5, allowProposedApi: true })
    await write(term, "connect to 10.0.0.1")
    const cells = read(term, 0, 2)
    expect(cells.text).toBe("connect to 10.0.0.1")
    expect(cells.rows.slice(9, 12)).toEqual([0, 1, 1])
    expect(cells.columns.slice(9, 12)).toEqual([9, 0, 1])
  })

  it("maps wide characters to their first cell", async () => {
    const term = new Terminal({ cols: 20, rows: 5, allowProposedApi: true })
    await write(term, "错误 error")
    const cells = read(term, 0, 1)
    expect(cells.text).toBe("错误 error")
    expect(cells.columns).toEqual([0, 2, 4, 5, 6, 7, 8, 9])
    expect(cells.widths.slice(0, 3)).toEqual([2, 2, 1])
  })

  it("marks the cells the program colored", async () => {
    const term = new Terminal({ cols: 20, rows: 5, allowProposedApi: true })
    await write(term, "a\x1b[31mb\x1b[0m\x1b[7mc\x1b[0m\x1b[1md")
    expect(read(term, 0, 1).plain).toEqual([true, false, false, true])
  })

  it("stops at the length it is given", async () => {
    const term = new Terminal({ cols: 10, rows: 5, allowProposedApi: true })
    await write(term, "x".repeat(25))
    expect(read(term, 0, 3, 12).text).toHaveLength(12)
  })
})

describe("highlightSegments", () => {
  it("splits a highlight over the rows it wraps across", async () => {
    const term = new Terminal({ cols: 10, rows: 5, allowProposedApi: true })
    await write(term, "connect to 10.0.0.1")
    const cells = read(term, 0, 2)
    expect(highlightSegments(cells, [{ start: 11, end: 19, foreground: "#00cccc" }])).toEqual([
      { row: 1, x: 1, width: 8, foreground: "#00cccc", background: undefined },
    ])
    expect(highlightSegments(cells, [{ start: 8, end: 13, foreground: "#00cccc" }])).toEqual([
      { row: 0, x: 8, width: 2, foreground: "#00cccc", background: undefined },
      { row: 1, x: 0, width: 3, foreground: "#00cccc", background: undefined },
    ])
  })

  it("covers both cells of wide characters", async () => {
    const term = new Terminal({ cols: 20, rows: 5, allowProposedApi: true })
    await write(term, "发生错误")
    const cells = read(term, 0, 1)
    expect(highlightSegments(cells, [{ start: 2, end: 4, foreground: "#cc0000" }])).toEqual([
      { row: 0, x: 4, width: 4, foreground: "#cc0000", background: undefined },
    ])
  })

  it("leaves out the cells the program colored", async () => {
    const term = new Terminal({ cols: 20, rows: 5, allowProposedApi: true })
    await write(term, "err\x1b[31mor\x1b[0m!")
    const cells = read(term, 0, 1)
    expect(highlightSegments(cells, [{ start: 0, end: 6, foreground: "#cc0000" }])).toEqual([
      { row: 0, x: 0, width: 3, foreground: "#cc0000", background: undefined },
      { row: 0, x: 5, width: 1, foreground: "#cc0000", background: undefined },
    ])
  })
})
