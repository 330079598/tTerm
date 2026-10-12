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

describe("Tooltip", () => {
  afterEach(cleanup)

  function renderTab() {
    render(
      <>
        <Tooltip>
          <TooltipTrigger asChild>
            <div>
              tab
              <button onMouseLeave={(event) => event.stopPropagation()}>close</button>
            </div>
          </TooltipTrigger>
          <TooltipContent>D:\code\tTerm</TooltipContent>
        </Tooltip>
        <div>elsewhere</div>
      </>
    )
    fireEvent.mouseEnter(screen.getByText("tab"))
    expect(screen.getByRole("tooltip")).toBeTruthy()
  }

  it("stays open while the pointer moves within its trigger", () => {
    renderTab()
    fireEvent.pointerOver(screen.getByText("close"))
    expect(screen.queryByRole("tooltip")).not.toBeNull()
  })

  it("closes when the pointer is over another element, even if mouseleave never reached the trigger", () => {
    renderTab()
    fireEvent.mouseLeave(screen.getByText("close"))
    fireEvent.pointerOver(screen.getByText("elsewhere"))
    expect(screen.queryByRole("tooltip")).toBeNull()
  })

  it("closes when the pointer leaves the window", () => {
    renderTab()
    fireEvent.mouseOut(screen.getByText("close"), { relatedTarget: null })
    expect(screen.queryByRole("tooltip")).toBeNull()
  })

  it("closes on a press, such as the start of a tab drag", () => {
    renderTab()
    fireEvent.pointerDown(screen.getByText("tab"))
    expect(screen.queryByRole("tooltip")).toBeNull()
  })

  it("closes when the window loses focus", () => {
    renderTab()
    fireEvent.blur(window)
    expect(screen.queryByRole("tooltip")).toBeNull()
  })
})
