import { describe, expect, it } from "vitest"

import { promptBarPlacement, promptInputRect } from "@/components/TerminalTab/passwordPromptLayout"

// A 600 x 400 surface; the bar is 300 x 50, so at the bottom it spans
// x 288..588 and y 338..388, and at the top y 12..62.
const surface = { surfaceWidth: 600, surfaceHeight: 400, barWidth: 300, barHeight: 50 }

describe("promptInputRect", () => {
  it("spans the row from its start through the key icon", () => {
    expect(
      promptInputRect({
        left: 256,
        top: 360,
        rowLeft: 6,
        cellWidth: 10,
        cellHeight: 20,
        cellsLeft: 30,
      })
    ).toEqual({ left: 6, top: 360, right: 276, bottom: 380 })
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
