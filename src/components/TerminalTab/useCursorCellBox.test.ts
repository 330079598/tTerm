import { describe, expect, it } from "vitest"

import { cursorCellBox } from "@/components/TerminalTab/useCursorCellBox"

const surface = { left: 100, top: 50, width: 900, height: 500 }
// 80 x 24 cells of 10 x 20 px, inset 6px into the surface.
const screen = { left: 106, top: 56, width: 800, height: 480 }

describe("cursorCellBox", () => {
  it("places the box on the cell after the cursor", () => {
    expect(
      cursorCellBox({ screen, surface, cols: 80, rows: 24, cursorCol: 10, cursorRow: 3 })
    ).toEqual({
      left: 116,
      top: 66,
      rowLeft: 6,
      cellWidth: 10,
      cellHeight: 20,
      cellsLeft: 69,
    })
  })

  it("keeps the box inside the row at its last cell", () => {
    expect(
      cursorCellBox({ screen, surface, cols: 80, rows: 24, cursorCol: 79, cursorRow: 0 })
    ).toMatchObject({ left: 806, cellsLeft: 0 })
  })

  it("has no box while the cursor is scrolled out of view", () => {
    expect(
      cursorCellBox({ screen, surface, cols: 80, rows: 24, cursorCol: 0, cursorRow: 24 })
    ).toBeNull()
    expect(
      cursorCellBox({ screen, surface, cols: 80, rows: 24, cursorCol: 0, cursorRow: -1 })
    ).toBeNull()
  })

  it("has no box before the terminal is measured", () => {
    expect(
      cursorCellBox({
        screen: { left: 0, top: 0, width: 0, height: 0 },
        surface,
        cols: 80,
        rows: 24,
        cursorCol: 0,
        cursorRow: 0,
      })
    ).toBeNull()
  })
})
