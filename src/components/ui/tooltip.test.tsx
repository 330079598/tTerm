// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

function rect(top: number, height: number): DOMRect {
  return DOMRect.fromRect({ x: 100, y: top, width: 80, height })
}

// The trigger sits at `triggerTop`; every tooltip is 30px tall.
function showTooltip(triggerTop: number, side?: "top" | "bottom") {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement
  ) {
    return this.getAttribute("role") === "tooltip" ? rect(0, 30) : rect(triggerTop, 28)
  })
  render(
    <Tooltip>
      <TooltipTrigger asChild>
        <div>tab</div>
      </TooltipTrigger>
      <TooltipContent side={side}>D:\code\tTerm</TooltipContent>
    </Tooltip>
  )
  fireEvent.mouseEnter(screen.getByText("tab"))
  return screen.getByRole("tooltip")
}

describe("TooltipContent", () => {
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
  })

  it("goes above its trigger when there is room", () => {
    const tooltip = showTooltip(400)
    expect(tooltip.style.top).toBe("400px")
    expect(tooltip.className).toContain("-translate-y-[calc(100%+0.5rem)]")
  })

  it("goes below a trigger at the top of the window, such as a tab in the title bar", () => {
    const tooltip = showTooltip(4)
    expect(tooltip.style.top).toBe("32px")
    expect(tooltip.className).toContain("translate-y-2")
    expect(tooltip.className).not.toContain("-translate-y-")
  })

  it("goes above a trigger asking for below when there is no room under it", () => {
    const tooltip = showTooltip(window.innerHeight - 30, "bottom")
    expect(tooltip.style.top).toBe(`${window.innerHeight - 30}px`)
    expect(tooltip.className).toContain("-translate-y-[calc(100%+0.5rem)]")
  })
})
