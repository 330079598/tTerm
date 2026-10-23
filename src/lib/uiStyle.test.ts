// @vitest-environment jsdom

import { afterEach, describe, expect, it } from "vitest"

import { applyUiStyle, preloadUiStyle } from "@/lib/uiStyle"

const root = () => document.documentElement

afterEach(() => {
  root().classList.remove("ui-islands")
  localStorage.clear()
})

describe("ui style", () => {
  it("marks the page for islands and clears the mark for classic", () => {
    applyUiStyle("islands")
    expect(root().classList.contains("ui-islands")).toBe(true)
    applyUiStyle("classic")
    expect(root().classList.contains("ui-islands")).toBe(false)
  })

  it("starts the next launch in the style last applied", () => {
    applyUiStyle("islands")
    root().classList.remove("ui-islands")
    preloadUiStyle()
    expect(root().classList.contains("ui-islands")).toBe(true)
  })

  it("starts classic when nothing was remembered", () => {
    preloadUiStyle()
    expect(root().classList.contains("ui-islands")).toBe(false)
  })
})
