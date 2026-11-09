import { describe, expect, it } from "vitest"
import wrapped from "@fixtures/ui.context-menu.wrapped.json"
import fixture from "@fixtures/ui.context-menu.json"
import { contextMenuFixture } from "@/parity/screens/contextMenuFixture"

describe("native context menu fixture", () => {
  it("keeps menu actions and shortcut labels derived from production helpers and defaults", () => {
    expect(fixture).toEqual(contextMenuFixture())
    expect(wrapped).toEqual(contextMenuFixture(true))
  })
})
