import { describe, expect, it } from "vitest"

import {
  PASTE_PREVIEW_MAX_LINE_LENGTH,
  PASTE_PREVIEW_MAX_LINES,
  pasteNeedsConfirmation,
  summarizePaste,
} from "@/lib/pasteGuard"

describe("pasteNeedsConfirmation", () => {
  const unguarded = { enabled: true, bracketedPasteMode: false }

  it("asks for text with any line break", () => {
    expect(pasteNeedsConfirmation("ls\npwd", unguarded)).toBe(true)
    expect(pasteNeedsConfirmation("rm -rf build\r\n", unguarded)).toBe(true)
    expect(pasteNeedsConfirmation("echo hi\r", unguarded)).toBe(true)
  })

  it("lets a single line through", () => {
    expect(pasteNeedsConfirmation("git status", unguarded)).toBe(false)
  })

  it("trusts bracketed paste mode and the setting", () => {
    expect(pasteNeedsConfirmation("ls\npwd", { enabled: true, bracketedPasteMode: true })).toBe(
      false
    )
    expect(pasteNeedsConfirmation("ls\npwd", { enabled: false, bracketedPasteMode: false })).toBe(
      false
    )
  })
})

describe("summarizePaste", () => {
  it("counts lines across line ending styles and ignores a trailing break", () => {
    expect(summarizePaste("a\r\nb\rc\n")).toEqual({ lineCount: 3, preview: "a\nb\nc" })
    expect(summarizePaste("one\n")).toEqual({ lineCount: 1, preview: "one" })
  })

  it("shortens long pastes and long lines", () => {
    const lines = Array.from({ length: PASTE_PREVIEW_MAX_LINES + 4 }, (_, index) => `line ${index}`)
    const summary = summarizePaste(lines.join("\n"))
    expect(summary.lineCount).toBe(PASTE_PREVIEW_MAX_LINES + 4)
    expect(summary.preview.split("\n")).toHaveLength(PASTE_PREVIEW_MAX_LINES + 1)
    expect(summary.preview.endsWith("\n…")).toBe(true)

    const long = "x".repeat(PASTE_PREVIEW_MAX_LINE_LENGTH + 10)
    expect(summarizePaste(`${long}\nok`).preview).toBe(
      `${"x".repeat(PASTE_PREVIEW_MAX_LINE_LENGTH)}…\nok`
    )
  })
})
