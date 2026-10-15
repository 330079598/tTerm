// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { nudgeTab, nudgeTabBar } from "@/lib/tabNudge"

function animationEnd(target: Element) {
  target.dispatchEvent(new Event("animationend", { bubbles: true }))
}

describe("tab nudges", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div class="tab-list-viewport">
        <div class="tab-item" data-tab-id="t1"><span class="tab-agent"></span></div>
        <div class="tab-item" data-tab-id="t2"></div>
      </div>`
    Element.prototype.scrollIntoView = vi.fn()
  })

  afterEach(() => {
    document.body.innerHTML = ""
  })

  const tab = (id: string) => document.querySelector(`[data-tab-id="${id}"]`)!

  it("flashes only the named tab and scrolls it into view", () => {
    nudgeTab("t2")

    expect(tab("t2").classList.contains("tab-nudge-flash")).toBe(true)
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(false)
    expect(Element.prototype.scrollIntoView).toHaveBeenCalledTimes(1)
  })

  it("clears the flash when its own animation ends, not a child's", () => {
    nudgeTab("t1")

    animationEnd(tab("t1").querySelector(".tab-agent")!)
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(true)

    animationEnd(tab("t1"))
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(false)
  })

  it("replays a nudge that is still running", () => {
    nudgeTab("t1")
    nudgeTab("t1")
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(true)

    animationEnd(tab("t1"))
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(false)

    nudgeTab("t1")
    expect(tab("t1").classList.contains("tab-nudge-flash")).toBe(true)
  })

  it("ignores a tab that is not in the bar", () => {
    expect(() => nudgeTab("missing")).not.toThrow()
    expect(document.querySelector(".tab-nudge-flash")).toBeNull()
  })

  it("shakes the tab bar", () => {
    const bar = document.querySelector(".tab-list-viewport")!

    nudgeTabBar()
    expect(bar.classList.contains("tab-nudge-shake")).toBe(true)

    animationEnd(bar)
    expect(bar.classList.contains("tab-nudge-shake")).toBe(false)
  })
})
