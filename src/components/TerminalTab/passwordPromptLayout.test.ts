import { describe, expect, it } from "vitest"

import {
  feedbackLayout,
  promptBarPlacement,
  promptInputRect,
} from "@/components/TerminalTab/passwordPromptLayout"

// An 80-cell row of 10 x 20 px cells starting 6px into the surface, so it
// ends at x 806.
function boxAt(cursorCol: number, top = 360) {
  const col = cursorCol + 1
  return { left: 6 + col * 10, top, rowLeft: 6, cellWidth: 10, cellHeight: 20, cellsLeft: 80 - col }
}

// A 600 x 400 surface; the bar is 300 x 50, so at the bottom it spans
// x 288..588 and y 338..388, and at the top y 12..62.
const surface = { surfaceWidth: 600, surfaceHeight: 400, barWidth: 300, barHeight: 50 }

describe("feedbackLayout", () => {
  it("draws a star per character right after the cursor", () => {
    expect(feedbackLayout(boxAt(24), 3)).toEqual({
      floating: false,
      left: 256,
      top: 360,
      cells: 55,
      marks: "***",
    })
  })

  it("fills the rest of the row behind an ellipsis once the stars reach its end", () => {
    expect(feedbackLayout(boxAt(68), 20).marks).toBe("…********")
  })

  it("moves onto a chip on the row above near the end of the row", () => {
    // Cursor in cell 75; four cells are left, too few for the stars.
    expect(feedbackLayout(boxAt(75), 3)).toEqual({
      floating: true,
      left: 746,
      top: 340,
      cells: 6,
      marks: "***",
    })
  })

  it("pulls the chip left so it ends within the row", () => {
    // At the last cell, a 6-cell chip must start 6 cells before the row's end.
    expect(feedbackLayout(boxAt(79), 3)).toMatchObject({ floating: true, left: 746 })
  })

  it("puts the chip below when the cursor is on the first row", () => {
    expect(feedbackLayout(boxAt(79, 6), 0)).toMatchObject({ floating: true, top: 26 })
  })

  it("keeps the lone key inline when the stars are off", () => {
    expect(feedbackLayout(boxAt(75), null)).toMatchObject({ floating: false, marks: "" })
  })
})

describe("promptInputRect", () => {
  it("spans the row from its start through the key icon", () => {
    const box = boxAt(24)
    expect(promptInputRect(box, feedbackLayout(box, 0))).toEqual({
      left: 6,
      top: 360,
      right: 276,
      bottom: 380,
    })
  })

  it("takes in a floating chip", () => {
    const box = boxAt(79)
    expect(promptInputRect(box, feedbackLayout(box, 3))).toEqual({
      left: 6,
      top: 340,
      right: 826,
      bottom: 380,
    })
  })
})

describe("promptBarPlacement", () => {
  it("stays at the bottom when the prompt line is clear of it", () => {
    const input = { left: 6, top: 360, right: 276, bottom: 380 }
    expect(promptBarPlacement({ ...surface, input })).toBe("bottom")
  })

  it("moves to the top when it would cover the prompt line", () => {
    const input = { left: 6, top: 360, right: 400, bottom: 380 }
    expect(promptBarPlacement({ ...surface, input })).toBe("top")
  })

  it("stays at the bottom when the top would cover the line too", () => {
    const input = { left: 6, top: 0, right: 400, bottom: 400 }
    expect(promptBarPlacement({ ...surface, input })).toBe("bottom")
  })

  it("stays at the bottom while the cursor is off screen", () => {
    expect(promptBarPlacement({ ...surface, input: null })).toBe("bottom")
  })
})
