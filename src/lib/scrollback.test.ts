import { describe, expect, it } from "vitest"

import {
  countLineFeeds,
  grownScrollbackCapacity,
  startingScrollbackLines,
  UNLIMITED_SCROLLBACK_BUFFER,
  UNLIMITED_SCROLLBACK_INITIAL,
} from "@/lib/scrollback"

describe("startingScrollbackLines", () => {
  it("starts unlimited scrollback small instead of at the full buffer", () => {
    expect(startingScrollbackLines(0)).toBe(UNLIMITED_SCROLLBACK_INITIAL)
  })

  it("passes explicit limits through", () => {
    expect(startingScrollbackLines(5000)).toBe(5000)
    expect(startingScrollbackLines(undefined)).toBe(10_000)
  })
})

describe("countLineFeeds", () => {
  it("counts LF, VT and FF but not CR", () => {
    expect(countLineFeeds("a\r\nb\nc\x0bd\x0ce\r")).toBe(4)
    expect(countLineFeeds("")).toBe(0)
  })
})

describe("grownScrollbackCapacity", () => {
  it("keeps the capacity while half of it is still free", () => {
    expect(grownScrollbackCapacity(10_000, 5000)).toBeNull()
  })

  it("doubles until the lines fit twice over", () => {
    expect(grownScrollbackCapacity(10_000, 5001)).toBe(20_000)
    expect(grownScrollbackCapacity(10_000, 30_000)).toBe(80_000)
  })

  it("never grows past the unlimited buffer", () => {
    expect(grownScrollbackCapacity(10_000, UNLIMITED_SCROLLBACK_BUFFER)).toBe(
      UNLIMITED_SCROLLBACK_BUFFER
    )
  })
})
